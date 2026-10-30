//! Petit client HTTP/1.1 bloquant (bibliothèque standard ; HTTPS par rustls et les autorités de
//! certification de webpki-roots) : corps JSON, réponses découpées (« chunked »), flux lus ligne à
//! ligne (Ollama, SSE). Ni proxy ni redirection (sauf demandée : téléchargements), une connexion
//! par requête (`Connection: close`).
//!
//! Délai : d'inactivité (aucun octet reçu pendant `timeout`). Arrêt : un [`Cancel`] arrête la
//! requête en cours depuis un autre fil (bouton « Arrêter ») : la connexion est non bloquante et
//! surveillée toutes les 5 ms (sous Windows, fermer une copie de la connexion ne débloque pas une
//! lecture en attente).

use serde_json::Value;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// au-delà, une réponse lue entière est refusée
const MAX_BODY: usize = 256 * 1024 * 1024;
/// attente entre deux essais de lecture ou d'écriture
const POLL: Duration = Duration::from_millis(5);

/// Échec d'une requête (une réponse HTTP d'erreur n'en est pas un : voir [`Response::status`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// connexion refusée : rien n'écoute à cette adresse (hôte:port)
    Refused(String),
    /// délai dépassé
    Timeout,
    /// arrêté par l'utilisateur
    Cancelled,
    /// adresse invalide
    BadUrl(String),
    /// connexion perdue, TLS, nom inconnu…
    Io(String),
    /// réponse incomprise
    Protocol(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(host) => write!(f, "Connexion refusée ({host})"),
            Self::Timeout => write!(f, "Délai dépassé"),
            Self::Cancelled => write!(f, "Arrêté"),
            Self::BadUrl(u) => write!(f, "Adresse invalide : {u}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Protocol(e) => write!(f, "Réponse incomprise : {e}"),
        }
    }
}

impl std::error::Error for HttpError {}

// ------------------------------------------------------------------ arrêt
/// Arrête une requête en cours (ou une attente) depuis un autre fil.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl std::fmt::Debug for Cancel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cancel({})", self.is_cancelled())
    }
}

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }

    /// Arrête : la requête en cours s'interrompt (« Arrêté ») dans les 5 ms, comme les attentes et
    /// les requêtes suivantes, jusqu'à [`Cancel::reset`].
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// Repart pour une nouvelle demande.
    pub fn reset(&self) {
        self.0.store(false, Ordering::SeqCst);
    }

    /// Erreur « Arrêté » si l'arrêt est demandé.
    pub fn check(&self) -> Result<(), HttpError> {
        if self.is_cancelled() { Err(HttpError::Cancelled) } else { Ok(()) }
    }

    /// Attend `d`, ou s'arrête dès que l'arrêt est demandé.
    pub fn sleep(&self, d: Duration) -> Result<(), HttpError> {
        let end = Instant::now() + d;
        loop {
            self.check()?;
            let now = Instant::now();
            if now >= end {
                return Ok(());
            }
            std::thread::sleep((end - now).min(Duration::from_millis(50)));
        }
    }
}

/// Erreur « arrêté » d'une lecture ou d'une écriture (pas `Interrupted` : la bibliothèque standard
/// réessaierait sans fin).
fn stopped_error() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionAborted, "arrêté")
}

// ------------------------------------------------------------------ adresses
/// Une adresse `http://` ou `https://`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub https: bool,
    /// hôte (IPv6 entre crochets)
    pub host: String,
    pub port: u16,
    /// chemin et requête (« /api/chat?x=1 »)
    pub path: String,
}

impl Url {
    pub fn parse(s: &str) -> Result<Url, HttpError> {
        let raw = s.trim();
        let bad = || HttpError::BadUrl(raw.chars().take(120).collect());
        let lower = raw.to_ascii_lowercase();
        let (https, rest) = if lower.starts_with("https://") {
            (true, &raw[8..])
        } else if lower.starts_with("http://") {
            (false, &raw[7..])
        } else {
            return Err(bad());
        };
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (authority, tail) = rest.split_at(end);
        if authority.is_empty() || authority.contains('@') {
            return Err(bad());
        }
        let default_port = if https { 443 } else { 80 };
        let (host, port) = if authority.starts_with('[') {
            let close = authority.find(']').ok_or_else(bad)?;
            let host = &authority[..=close];
            let port = match &authority[close + 1..] {
                "" => default_port,
                p => p.strip_prefix(':').and_then(|p| p.parse().ok()).ok_or_else(bad)?,
            };
            (host.to_owned(), port)
        } else {
            match authority.rsplit_once(':') {
                Some((h, p)) => (h.to_owned(), if p.is_empty() { default_port } else { p.parse().map_err(|_| bad())? }),
                None => (authority.to_owned(), default_port),
            }
        };
        if host.is_empty() {
            return Err(bad());
        }
        let tail = tail.split('#').next().unwrap_or("");
        let path = if tail.is_empty() {
            "/".to_owned()
        } else if tail.starts_with('?') {
            format!("/{tail}")
        } else {
            tail.to_owned()
        };
        Ok(Url { https, host: host.to_ascii_lowercase(), port, path })
    }

    fn default_port(&self) -> bool {
        self.port == if self.https { 443 } else { 80 }
    }

    /// « hôte:port » (port omis s'il est celui par défaut).
    pub fn authority(&self) -> String {
        if self.default_port() { self.host.clone() } else { format!("{}:{}", self.host, self.port) }
    }

    /// « http://127.0.0.1:11435 »
    pub fn origin(&self) -> String {
        format!("{}://{}", if self.https { "https" } else { "http" }, self.authority())
    }

    /// Hôte sans crochets (pour la résolution de nom et TLS).
    fn bare_host(&self) -> &str {
        self.host.trim_start_matches('[').trim_end_matches(']')
    }

    /// Hôte de cet ordinateur (127.0.0.1, localhost, [::1]).
    pub fn is_local(&self) -> bool {
        matches!(self.host.as_str(), "127.0.0.1" | "localhost" | "[::1]")
    }
}

// ------------------------------------------------------------------ connexion
/// La connexion TCP, non bloquante : chaque lecture ou écriture attend par petits pas, en
/// surveillant l'arrêt et le délai d'inactivité.
struct Sock {
    tcp: TcpStream,
    cancel: Option<Cancel>,
    /// délai d'inactivité (ms), modifiable pendant la réponse
    idle: Arc<AtomicU64>,
}

impl Sock {
    fn wait<T>(&self, mut op: impl FnMut(&TcpStream) -> io::Result<T>) -> io::Result<T> {
        let start = Instant::now();
        loop {
            if self.cancel.as_ref().is_some_and(Cancel::is_cancelled) {
                return Err(stopped_error());
            }
            match op(&self.tcp) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if start.elapsed() >= Duration::from_millis(self.idle.load(Ordering::Relaxed)) {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    std::thread::sleep(POLL);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                r => return r,
            }
        }
    }
}

impl Read for Sock {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.wait(|mut t| t.read(buf))
    }
}

impl Write for Sock {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.wait(|mut t| t.write(buf))
    }
    fn flush(&mut self) -> io::Result<()> {
        self.wait(|mut t| t.flush())
    }
}

enum Conn {
    Plain(Sock),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, Sock>>),
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Conn::Plain(s) => s.read(buf),
            Conn::Tls(s) => match s.read(buf) {
                // serveur qui ferme sans « close_notify » : fin du flux
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(0),
                r => r,
            },
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Conn::Plain(s) => s.write(buf),
            Conn::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Conn::Plain(s) => s.flush(),
            Conn::Tls(s) => s.flush(),
        }
    }
}

fn tls_config() -> Result<Arc<rustls::ClientConfig>, HttpError> {
    static CFG: OnceLock<Result<Arc<rustls::ClientConfig>, String>> = OnceLock::new();
    CFG.get_or_init(|| {
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map(|b| Arc::new(b.with_root_certificates(roots).with_no_client_auth()))
            .map_err(|e| e.to_string())
    })
    .clone()
    .map_err(HttpError::Io)
}

fn io_error(e: io::Error, cancel: Option<&Cancel>, host: &str) -> HttpError {
    if cancel.is_some_and(Cancel::is_cancelled) {
        return HttpError::Cancelled;
    }
    match e.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => HttpError::Timeout,
        io::ErrorKind::ConnectionRefused => HttpError::Refused(host.to_owned()),
        _ => HttpError::Io(format!("{host} : {e}")),
    }
}

fn connect(url: &Url, idle: &Arc<AtomicU64>, cancel: Option<&Cancel>) -> Result<Conn, HttpError> {
    let host = url.authority();
    let addrs: Vec<_> = (url.bare_host(), url.port).to_socket_addrs().map_err(|e| HttpError::Io(format!("{host} : {e}")))?.collect();
    let mut last = HttpError::Refused(host.clone());
    let mut stream = None;
    let limit = CONNECT_TIMEOUT.min(Duration::from_millis(idle.load(Ordering::Relaxed)).max(Duration::from_millis(100)));
    for a in addrs {
        if let Some(c) = cancel {
            c.check()?;
        }
        match TcpStream::connect_timeout(&a, limit) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => last = io_error(e, cancel, &host),
        }
    }
    let tcp = stream.ok_or(last)?;
    tcp.set_nonblocking(true).map_err(|e| io_error(e, cancel, &host))?;
    let _ = tcp.set_nodelay(true);
    let sock = Sock { tcp, cancel: cancel.cloned(), idle: idle.clone() };
    if !url.https {
        return Ok(Conn::Plain(sock));
    }
    let name = rustls_pki_types::ServerName::try_from(url.bare_host().to_owned()).map_err(|_| HttpError::BadUrl(url.host.clone()))?;
    let conn = rustls::ClientConnection::new(tls_config()?, name).map_err(|e| HttpError::Io(format!("{host} : {e}")))?;
    Ok(Conn::Tls(Box::new(rustls::StreamOwned::new(conn, sock))))
}

// ------------------------------------------------------------------ requête
/// Une requête.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// délai d'inactivité (aucun octet reçu)
    pub timeout: Duration,
    /// suivre les redirections (téléchargements seulement)
    pub follow_redirects: bool,
}

impl Request {
    pub fn new(method: &str, url: &str) -> Request {
        Request { method: method.to_owned(), url: url.to_owned(), headers: Vec::new(), body: None, timeout: Duration::from_secs(240), follow_redirects: false }
    }

    pub fn get(url: &str) -> Request {
        Request::new("GET", url)
    }

    /// POST d'un corps JSON.
    pub fn post_json(url: &str, body: &Value) -> Request {
        Request::new("POST", url).json(body)
    }

    pub fn json(mut self, body: &Value) -> Request {
        self.body = Some(serde_json::to_vec(body).unwrap_or_default());
        self.header("Content-Type", "application/json")
    }

    pub fn header(mut self, name: &str, value: &str) -> Request {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn timeout(mut self, d: Duration) -> Request {
        self.timeout = d;
        self
    }

    pub fn follow_redirects(mut self) -> Request {
        self.follow_redirects = true;
        self
    }

    /// Envoie la requête ; la réponse est rendue dès ses en-têtes lus (corps à lire ensuite).
    pub fn send(&self, cancel: Option<&Cancel>) -> Result<Response, HttpError> {
        let mut url = self.url.clone();
        for _ in 0..6 {
            let res = self.send_to(&url, cancel)?;
            if self.follow_redirects && matches!(res.status, 301 | 302 | 303 | 307 | 308)
                && let Some(loc) = res.header("location") {
                    url = resolve_location(&url, loc)?;
                    continue;
                }
            return Ok(res);
        }
        Err(HttpError::Protocol("trop de redirections".into()))
    }

    fn send_to(&self, raw_url: &str, cancel: Option<&Cancel>) -> Result<Response, HttpError> {
        if let Some(c) = cancel {
            c.check()?;
        }
        let url = Url::parse(raw_url)?;
        let host = url.authority();
        let idle = Arc::new(AtomicU64::new(self.timeout.as_millis().clamp(1, u128::from(u64::MAX)) as u64));
        let mut conn = connect(&url, &idle, cancel)?;
        let mut head = format!("{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: ARKLIGHT/{}\r\n", self.method, url.path, url.authority(), env!("CARGO_PKG_VERSION"));
        for (k, v) in &self.headers {
            if k.contains(['\r', '\n']) || v.contains(['\r', '\n']) {
                return Err(HttpError::Protocol("en-tête invalide".into()));
            }
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some(b) = &self.body {
            head.push_str(&format!("Content-Length: {}\r\n", b.len()));
        } else if matches!(self.method.as_str(), "POST" | "PUT" | "PATCH") {
            head.push_str("Content-Length: 0\r\n");
        }
        head.push_str("\r\n");
        let io = |e| io_error(e, cancel, &host);
        conn.write_all(head.as_bytes()).map_err(io)?;
        if let Some(b) = &self.body {
            conn.write_all(b).map_err(io)?;
        }
        conn.flush().map_err(io)?;
        let mut reader = BufReader::new(conn);
        let (status, headers) = loop {
            let (status, headers) = read_head(&mut reader).map_err(io)?;
            if status != 100 {
                break (status, headers);
            }
        };
        let get = |n: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.as_str());
        let mode = if self.method == "HEAD" || status == 204 || status == 304 || (100..200).contains(&status) {
            BodyMode::Length(0)
        } else if get("transfer-encoding").is_some_and(|t| t.to_ascii_lowercase().contains("chunked")) {
            BodyMode::Chunked { left: 0, started: false, done: false }
        } else if let Some(n) = get("content-length").and_then(|v| v.trim().parse::<u64>().ok()) {
            BodyMode::Length(n)
        } else {
            BodyMode::Eof
        };
        let body = BodyReader { inner: reader, mode, cancel: cancel.cloned() };
        Ok(Response { status, headers, reader: BufReader::new(body), idle, host })
    }
}

fn resolve_location(base: &str, loc: &str) -> Result<String, HttpError> {
    if loc.starts_with("http://") || loc.starts_with("https://") {
        return Ok(loc.to_owned());
    }
    let u = Url::parse(base)?;
    if loc.starts_with('/') {
        Ok(format!("{}{loc}", u.origin()))
    } else {
        let dir = u.path.rsplit_once('/').map_or("", |(d, _)| d);
        Ok(format!("{}{dir}/{loc}", u.origin()))
    }
}

fn read_line_raw(r: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    let n = r.read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    while buf.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        buf.pop();
    }
    Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
}

fn read_head(r: &mut impl BufRead) -> io::Result<(u16, Vec<(String, String)>)> {
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_owned());
    let line = read_line_raw(r)?.ok_or_else(|| bad("connexion fermée avant la réponse"))?;
    let mut parts = line.splitn(3, ' ');
    let version = parts.next().unwrap_or("");
    if !version.starts_with("HTTP/") {
        return Err(bad("ce n'est pas une réponse HTTP"));
    }
    let status = parts.next().and_then(|s| s.parse::<u16>().ok()).ok_or_else(|| bad("statut illisible"))?;
    let mut headers = Vec::new();
    loop {
        let l = read_line_raw(r)?.ok_or_else(|| bad("en-têtes coupés"))?;
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            headers.push((k.trim().to_owned(), v.trim().to_owned()));
        }
        if headers.len() > 200 {
            return Err(bad("trop d'en-têtes"));
        }
    }
    Ok((status, headers))
}

enum BodyMode {
    Length(u64),
    Chunked { left: u64, started: bool, done: bool },
    Eof,
}

/// Le corps d'une réponse (longueur connue, découpé, ou jusqu'à la fermeture).
struct BodyReader {
    inner: BufReader<Conn>,
    mode: BodyMode,
    cancel: Option<Cancel>,
}

impl BodyReader {
    fn stopped(&self) -> bool {
        self.cancel.as_ref().is_some_and(Cancel::is_cancelled)
    }
}

impl Read for BodyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.stopped() {
            return Err(stopped_error());
        }
        if buf.is_empty() {
            return Ok(0);
        }
        let n = match &mut self.mode {
            BodyMode::Length(0) => return Ok(0),
            BodyMode::Length(left) => {
                let max = (*left).min(buf.len() as u64) as usize;
                let n = self.inner.read(&mut buf[..max])?;
                if n == 0 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "réponse coupée"));
                }
                *left -= n as u64;
                n
            }
            BodyMode::Chunked { left, started, done } => {
                if *done {
                    return Ok(0);
                }
                if *left == 0 {
                    if *started {
                        // fin de ligne après les données du morceau
                        read_line_raw(&mut self.inner)?;
                    }
                    *started = true;
                    let line = read_line_raw(&mut self.inner)?.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "réponse coupée"))?;
                    let size = line.split(';').next().unwrap_or("").trim();
                    let size = u64::from_str_radix(size, 16).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "morceau illisible"))?;
                    if size == 0 {
                        // en-têtes de fin, puis ligne vide
                        while let Some(l) = read_line_raw(&mut self.inner)? {
                            if l.is_empty() {
                                break;
                            }
                        }
                        *done = true;
                        return Ok(0);
                    }
                    *left = size;
                }
                let max = (*left).min(buf.len() as u64) as usize;
                let n = self.inner.read(&mut buf[..max])?;
                if n == 0 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "réponse coupée"));
                }
                *left -= n as u64;
                n
            }
            BodyMode::Eof => self.inner.read(buf)?,
        };
        if n == 0 && self.stopped() {
            return Err(stopped_error());
        }
        Ok(n)
    }
}

// ------------------------------------------------------------------ réponse
/// Une réponse : statut et en-têtes lus, corps à lire (entier ou ligne à ligne).
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    reader: BufReader<BodyReader>,
    /// délai d'inactivité (ms) de la connexion
    idle: Arc<AtomicU64>,
    host: String,
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Response({} {})", self.status, self.host)
    }
}

impl Response {
    /// Succès (200 à 299).
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Un en-tête (nom sans casse).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    fn err(&self, e: io::Error) -> HttpError {
        io_error(e, self.reader.get_ref().cancel.as_ref(), &self.host)
    }

    /// Change le délai d'inactivité (le premier morceau d'un gros modèle peut être long).
    pub fn set_timeout(&self, d: Duration) {
        self.idle.store(d.as_millis().clamp(1, u128::from(u64::MAX)) as u64, Ordering::Relaxed);
    }

    /// Le corps entier.
    pub fn bytes(mut self) -> Result<Vec<u8>, HttpError> {
        let mut out = Vec::new();
        let mut buf = [0u8; 16 * 1024];
        loop {
            match self.reader.read(&mut buf) {
                Ok(0) => return Ok(out),
                Ok(n) => {
                    out.extend_from_slice(&buf[..n]);
                    if out.len() > MAX_BODY {
                        return Err(HttpError::Protocol("réponse trop grosse".into()));
                    }
                }
                Err(e) => return Err(self.err(e)),
            }
        }
    }

    /// Le corps en texte (UTF-8, caractères invalides remplacés).
    pub fn text(self) -> Result<String, HttpError> {
        self.bytes().map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    /// Le texte et son JSON (null s'il n'en est pas).
    pub fn text_and_json(self) -> Result<(String, Value), HttpError> {
        let text = self.text()?;
        let json = if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap_or(Value::Null) };
        Ok((text, json))
    }

    /// La ligne suivante du corps (sans fin de ligne), None à la fin du flux.
    pub fn next_line(&mut self) -> Result<Option<String>, HttpError> {
        match read_line_raw(&mut self.reader) {
            Ok(l) => Ok(l),
            Err(e) => Err(self.err(e)),
        }
    }

    /// Copie le corps dans `out` ; `progress(octets reçus)` à chaque morceau.
    pub fn copy_to(mut self, out: &mut impl Write, mut progress: impl FnMut(u64)) -> Result<u64, HttpError> {
        let mut total = 0u64;
        let mut buf = [0u8; 64 * 1024];
        loop {
            match self.reader.read(&mut buf) {
                Ok(0) => return Ok(total),
                Ok(n) => {
                    out.write_all(&buf[..n]).map_err(|e| HttpError::Io(e.to_string()))?;
                    total += n as u64;
                    progress(total);
                }
                Err(e) => return Err(self.err(e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses() {
        let u = Url::parse("http://127.0.0.1:11435").unwrap();
        assert_eq!((u.https, u.host.as_str(), u.port, u.path.as_str()), (false, "127.0.0.1", 11435, "/"));
        assert!(u.is_local());
        let u = Url::parse("https://api.anthropic.com/v1/messages?x=1#f").unwrap();
        assert_eq!((u.https, u.port, u.path.as_str(), u.origin()), (true, 443, "/v1/messages?x=1", "https://api.anthropic.com".to_owned()));
        let u = Url::parse("http://[::1]:8080?a").unwrap();
        assert_eq!((u.host.as_str(), u.port, u.path.as_str()), ("[::1]", 8080, "/?a"));
        assert!(Url::parse("ftp://x").is_err());
        assert!(Url::parse("http://user:pw@127.0.0.1/").is_err());
        assert_eq!(resolve_location("https://a.com/x/y", "/z").unwrap(), "https://a.com/z");
    }
}
