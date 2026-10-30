//! Faux serveur HTTP local pour les essais (127.0.0.1, port choisi par le système) : AIODOS,
//! Ollama, API compatibles OpenAI ou Anthropic, sans aucun accès à Internet.

#![allow(dead_code)]

use arklight_core::Project;
use arklight_core::document::Document;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Une requête reçue.
#[derive(Debug, Clone)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Req {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    /// Chemin sans la requête (« /api/chat »).
    pub fn route(&self) -> &str {
        self.path.split('?').next().unwrap_or("")
    }

    /// Valeur d'un paramètre de la requête.
    pub fn query(&self, key: &str) -> Option<String> {
        let q = self.path.split_once('?')?.1;
        q.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v.to_owned()))
    }
}

/// Une réponse du faux serveur.
pub enum Reply {
    /// statut, corps JSON, en-têtes en plus
    Json(u16, Value, Vec<(&'static str, String)>),
    /// flux SSE découpé (« chunked ») : chaque ligne donnée est envoyée suivie d'un saut de ligne
    Sse(Vec<String>, Vec<(&'static str, String)>),
    /// attendre avant de répondre (essais d'arrêt)
    Slow(Duration, Box<Reply>),
}

pub fn json(status: u16, v: Value) -> Reply {
    Reply::Json(status, v, Vec::new())
}

/// Réponse JSON d'AIODOS (en-tête X-Aiodos-Api).
pub fn aiodos_json(status: u16, v: Value) -> Reply {
    Reply::Json(status, v, vec![("X-Aiodos-Api", "1".into())])
}

type Handler = dyn Fn(&Req) -> Reply + Send + Sync;

/// Le faux serveur ; arrêté quand il est lâché.
pub struct FakeServer {
    pub url: String,
    pub port: u16,
    log: Arc<Mutex<Vec<Req>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeServer {
    pub fn start(handler: impl Fn(&Req) -> Reply + Send + Sync + 'static) -> FakeServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("port local");
        let port = listener.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handler: Arc<Handler> = Arc::new(handler);
        let (l, s) = (log.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            for conn in listener.incoming() {
                if s.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                let (h, l) = (handler.clone(), l.clone());
                std::thread::spawn(move || serve(conn, &*h, &l));
            }
        });
        FakeServer { url: format!("http://127.0.0.1:{port}"), port, log, stop, thread: Some(thread) }
    }

    pub fn requests(&self) -> Vec<Req> {
        self.log.lock().unwrap().clone()
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn serve(conn: TcpStream, handler: &Handler, log: &Mutex<Vec<Req>>) {
    let _ = conn.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(conn.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_owned(), parts.next().unwrap_or("").to_owned());
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 {
            break;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_owned(), v.trim().to_owned()));
        }
    }
    let len = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(0);
    let mut body = vec![0u8; len];
    let _ = reader.read_exact(&mut body);
    let req = Req { method, path, headers, body };
    log.lock().unwrap().push(req.clone());
    let reply = handler(&req);
    write_reply(conn, reply);
}

fn write_reply(mut conn: TcpStream, reply: Reply) {
    match reply {
        Reply::Slow(d, inner) => {
            std::thread::sleep(d);
            return write_reply(conn, *inner);
        }
        Reply::Json(status, v, extra) => {
            let body = v.to_string();
            let mut head = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
            for (k, v) in extra {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = conn.write_all(head.as_bytes());
            let _ = conn.write_all(body.as_bytes());
        }
        Reply::Sse(lines, extra) => {
            let mut head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n".to_owned();
            for (k, v) in extra {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = conn.write_all(head.as_bytes());
            for l in lines {
                let data = format!("{l}\n");
                let _ = conn.write_all(format!("{:x}\r\n{data}\r\n", data.len()).as_bytes());
                let _ = conn.flush();
            }
            let _ = conn.write_all(b"0\r\n\r\n");
        }
    }
    let _ = conn.flush();
}

/// Un port où rien n'écoute.
pub fn closed_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Le projet d'exemple « demo-night » (données d'essai d'arklight-core).
pub fn demo() -> Document {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("arklight-core").join("tests").join("data").join("demo-night.json");
    let text = std::fs::read_to_string(&p).expect("demo-night.json");
    Document::new(Project::from_json(&text).expect("projet"), None)
}

/// Un dossier temporaire propre à l'essai, effacé à la fin.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!("arklight-ai-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
