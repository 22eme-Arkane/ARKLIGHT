//! AIODOS, l'IA locale d'ARKANE : un moteur de l'assistant, à côté d'Ollama et des API en ligne
//! (portage de `server/ai/aiodos.js` et de la partie du client `aiodos-client` 0.2.0 qu'il utilise :
//! accès, preuve d'identité, état, modèles, conversation).
//!
//! Le client d'origine, `aiodos-client` (Copyright 2026 22eme Arkane, auteur d'AIODOS), est sous
//! licence Apache-2.0 : voir les fichiers LICENSE et NOTICE de `vendor/aiodos-client-0.2.0.tgz`.
//!
//! ARKLIGHT n'a accès à AIODOS qu'après l'accord de l'utilisateur : bouton « Se connecter à
//! AIODOS », code à 6 chiffres à comparer dans AIODOS (relève de la clé à la manière de la
//! RFC 8628, « device flow ») ; la clé d'ARKLIGHT est alors rangée avec les autres clés
//! ([`KeyStore`], « aiodos »). Rien n'est demandé au lancement, rien n'est téléchargé.
//!
//! Sécurité (comme le client JavaScript) : seulement 127.0.0.1, localhost ou [::1] ; aucune
//! redirection suivie ; la clé n'est envoyée qu'après la preuve d'identité d'AIODOS
//! (HMAC-SHA-256 d'un défi, clé = SHA-256 de la clé d'accès) ; jamais dans un message d'erreur.

use crate::error::AiError;
use crate::http::{Cancel, HttpError, Request, Response, Url};
use crate::keys::KeyStore;
use crate::providers::{AnsweredBy, ChatOptions, ChatReply, Message, ModelInfo, ToolCall};
use crate::util::{base64url_decode_43, base64url_encode, js_string, truthy};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Adresse par défaut : 127.0.0.1 et non localhost, la passerelle n'écoute pas en IPv6.
pub const DEFAULT_URL: &str = "http://127.0.0.1:11435";
/// Version de l'API des applications d'AIODOS parlée ici.
pub const API_VERSION: f64 = 1.0;
/// Droits demandés : la conversation (outils et photos compris).
pub const AIODOS_SCOPES: [&str; 1] = ["chat"];
/// Où calculer : ce PC (les outils et les images ne se calculent qu'ici de toute façon).
pub const AIODOS_WHERE: [&str; 1] = ["here"];
const CLIENT_HEADER: &str = concat!("rust/arklight-ai/", env!("CARGO_PKG_VERSION"));
const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
const PROOF_LABEL: &str = "aiodos-proof/1\n";
const PROOF_TTL: Duration = Duration::from_secs(60);
const LOADED_TTL: Duration = Duration::from_secs(30);
const INFO_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_TIMEOUT: Duration = Duration::from_secs(10);
const REVOKE_TIMEOUT: Duration = Duration::from_secs(10);
const MODELS_TIMEOUT: Duration = Duration::from_secs(15);
const PROBE_TIMEOUT: Duration = Duration::from_secs(4);
const HELPER_TIMEOUT: Duration = Duration::from_secs(90);
const CIRCLES: [&str; 4] = ["here", "mine", "friends", "public"];
const RESERVED_NAMES: [&str; 4] = ["aiodos", "autres outils", "windows", "microsoft"];
/// Identifiant d'Inno Setup de l'installeur d'AIODOS.
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{8E1BB2BE-7BA8-4AF2-ACC7-463BA169FCF3}_is1";

fn purpose(lang: &str) -> &'static str {
    if lang == "en" {
        "Lighting assistant: understand requests, read drawn plots and act on the lighting plot"
    } else {
        "Assistant lumière : comprendre les demandes, lire les plans dessinés et agir sur le plan de feu"
    }
}

// ------------------------------------------------------------------ textes
/// Message d'un état de [`Aiodos::status`].
pub fn state_text(state: &str, lang: &str, code: &str) -> String {
    let en = lang == "en";
    let t = match state {
        "not_installed" => if en { "AIODOS is not installed on this PC." } else { "AIODOS n'est pas installé sur ce PC." },
        "not_running" => if en { "AIODOS is not running." } else { "AIODOS n'est pas lancé." },
        "gateway_closed" => if en { "AIODOS is running, but is not open to applications." } else { "AIODOS tourne, mais n'est pas ouvert aux applications." },
        "gateway_error" => if en { "AIODOS's gateway reports an error." } else { "La passerelle d'AIODOS est en erreur." },
        "apps_closed" => if en { "AIODOS is only open to other tools." } else { "AIODOS n'est ouvert qu'aux autres outils." },
        "not_aiodos" => if en { "The port is taken by another program." } else { "Le port est pris par un autre programme." },
        "too_old" => if en { "Update AIODOS." } else { "Mets AIODOS à jour." },
        "untrusted" => if en { "The program that answers does not know the key: it was not sent." } else { "Le programme qui répond ne connaît pas la clé : elle n'a pas été envoyée." },
        "headless" => if en { "AIODOS is running without a window: open it to authorize the app." } else { "AIODOS tourne sans fenêtre : ouvre-le pour autoriser l'appli." },
        "requests_closed" => if en { "AIODOS is not accepting new applications right now." } else { "AIODOS n'accepte pas de nouvelle application en ce moment." },
        "needs_access" => if en { "The app does not have access to AIODOS yet." } else { "L'appli n'a pas encore accès à AIODOS." },
        "pending" => return if en { format!("Authorize the app in the AIODOS window. Code: {code}") } else { format!("Autorise l'appli dans la fenêtre d'AIODOS. Code : {code}") },
        "denied" => if en { "Access was denied in AIODOS." } else { "L'accès a été refusé dans AIODOS." },
        "revoked" => if en { "Access was removed (or has expired) in AIODOS." } else { "L'accès a été retiré (ou a expiré) dans AIODOS." },
        "other_account" => if en { "AIODOS is running under another Windows account." } else { "AIODOS tourne sous un autre compte Windows." },
        "ready" => if en { "Connected to AIODOS." } else { "Connecté à AIODOS." },
        other => other,
    };
    t.to_owned()
}

/// Message d'un code d'erreur du client (et messages de secours de ceux du serveur).
fn code_text(code: &str, lang: &str, var: &str) -> Option<String> {
    let en = lang == "en";
    let t = match code {
        "unreachable" => return Some(if en { format!("Connection refused ({var}): AIODOS is not running, or is not open to applications.") } else { format!("Connexion refusée ({var}) : AIODOS n'est pas lancé, ou n'est pas ouvert aux applications.") }),
        "connection_lost" => return Some(if en { format!("Connection refused or lost ({var}): AIODOS no longer answers.") } else { format!("Connexion refusée ou coupée ({var}) : AIODOS ne répond plus.") }),
        "not_aiodos" => return Some(if en { format!("Port {var} is taken by a program other than AIODOS.") } else { format!("Le port {var} est pris par un autre programme qu'AIODOS.") }),
        "untrusted" => return Some(if en { format!("The program that answers on {var} does not know the app's key: it was not sent. Check that AIODOS is running.") } else { format!("Le programme qui répond sur {var} ne connaît pas la clé de l'appli : elle n'a pas été envoyée. Vérifie qu'AIODOS est bien lancé.") }),
        "non_local_url" => return Some(if en { format!("Address refused ({var}): AIODOS is only called on this PC (127.0.0.1, localhost or [::1]).") } else { format!("Adresse refusée ({var}) : AIODOS ne s'appelle que sur ce PC (127.0.0.1, localhost ou [::1]).") }),
        "redirect" => return Some(if en { format!("Response refused: AIODOS never redirects, another program answers on {var}.") } else { format!("Réponse refusée : AIODOS ne redirige jamais, un autre programme répond sur {var}.") }),
        "not_supported" => return Some(if en { format!("This version of AIODOS does not serve “{var}” yet: update AIODOS.") } else { format!("Cette version d'AIODOS ne sert pas encore « {var} » : mets AIODOS à jour.") }),
        "program_missing" => return Some(if en { format!("AIODOS program not found: {var}") } else { format!("Programme d'AIODOS introuvable : {var}") }),
        "dev_copy" => return Some(if en { format!("Development copy of AIODOS ({var}) without an interpreter: set AIODOS_PYTHON.") } else { format!("Copie de développement d'AIODOS ({var}) sans interpréteur : donne-le par AIODOS_PYTHON.") }),
        "http" => return Some(if en { format!("Unexpected response from AIODOS (HTTP {var}).") } else { format!("Réponse inattendue d'AIODOS (HTTP {var}).") }),
        "gateway_closed" => if en { "AIODOS is running, but is not open to applications." } else { "AIODOS tourne, mais n'est pas ouvert aux applications." },
        "too_old" => if en { "This version of AIODOS is too old for applications: update AIODOS." } else { "Cette version d'AIODOS est trop ancienne pour les applications : mets AIODOS à jour." },
        "timeout" => if en { "Timed out: AIODOS does not answer." } else { "Délai dépassé : AIODOS ne répond pas." },
        "cancelled" => if en { "Stopped" } else { "Arrêté" },
        "protocol" => if en { "Response from AIODOS not understood." } else { "Réponse incomprise d'AIODOS." },
        "cut" => if en { "AIODOS's response stopped before the end." } else { "La réponse d'AIODOS s'est arrêtée avant la fin." },
        "not_installed" => if en { "AIODOS is not installed on this PC." } else { "AIODOS n'est pas installé sur ce PC." },
        "not_running" => if en { "AIODOS is not running." } else { "AIODOS n'est pas lancé." },
        "needs_access" => if en { "The app does not have access to AIODOS yet: connect it first." } else { "L'appli n'a pas encore accès à AIODOS : connecte-la d'abord." },
        "bad_key" => if en { "The stored key does not look like an AIODOS application key: it was not sent." } else { "La clé rangée n'a pas la forme d'une clé d'application d'AIODOS : elle n'a pas été envoyée." },
        "access_denied" => if en { "Access was denied in AIODOS." } else { "L'accès a été refusé dans AIODOS." },
        "expired_token" => if en { "The access request has expired: start again." } else { "La demande d'accès a expiré : recommence." },
        "invalid_grant" => if en { "Unknown access request, or key already collected: start again." } else { "Demande d'accès inconnue, ou clé déjà relevée : recommence." },
        "invalid_request" => if en { "Invalid request." } else { "Demande invalide." },
        "invalid_api_key" => if en { "AIODOS access key missing or invalid." } else { "Clé d'accès d'AIODOS manquante ou invalide." },
        "key_revoked" => if en { "The app's access was removed in AIODOS." } else { "L'accès de l'appli a été retiré dans AIODOS." },
        "key_expired" => if en { "The app's access has expired in AIODOS." } else { "L'accès de l'appli a expiré dans AIODOS." },
        "scope_denied" => if en { "The app does not have this right in AIODOS." } else { "L'appli n'a pas ce droit dans AIODOS." },
        "apps_closed" => if en { "AIODOS is only open to other tools." } else { "AIODOS n'est ouvert qu'aux autres outils." },
        "requests_closed" => if en { "AIODOS is not accepting new applications right now." } else { "AIODOS n'accepte pas de nouvelle application en ce moment." },
        "other_account" => if en { "AIODOS is running under another Windows account." } else { "AIODOS tourne sous un autre compte Windows." },
        "model_not_found" => if en { "Unknown model." } else { "Modèle inconnu." },
        "no_machine" => if en { "No allowed machine has this model right now." } else { "Aucune machine permise n'a ce modèle en ce moment." },
        "context_length_exceeded" => if en { "The request exceeds the model's context." } else { "La demande dépasse le contexte du modèle." },
        "tools_unavailable" => if en { "No allowed machine takes tools for this model." } else { "Aucune machine permise ne reçoit les outils pour ce modèle." },
        "vision_unsupported" => if en { "This model does not see images." } else { "Ce modèle ne voit pas les images." },
        "local_only" => if en { "This request is only computed on this PC, which does not have the model." } else { "Cette demande ne se calcule que sur ce PC, qui n'a pas le modèle." },
        "file_too_large" => if en { "Request too large." } else { "Demande trop grosse." },
        "rate_limited" => if en { "Too many requests: try again in a moment." } else { "Trop de demandes : réessaie dans un moment." },
        "busy" => if en { "AIODOS is busy: try again in a moment." } else { "AIODOS est occupé : réessaie dans un moment." },
        "engine_failed" => if en { "The engine could not answer." } else { "Le moteur n'a pas pu répondre." },
        "engine_unavailable" => if en { "The engine is not ready: down, starting, or on no allowed machine." } else { "Le moteur n'est pas prêt : en panne, en démarrage, ou sur aucune machine permise." },
        "approval_unavailable" => if en { "AIODOS is running without a window: open it to authorize the app." } else { "AIODOS tourne sans fenêtre : ouvre-le pour autoriser l'appli." },
        _ => return None,
    };
    Some(t.to_owned())
}

/// Conseil donné avec une erreur propre au client.
fn hint_text(code: &str, lang: &str) -> Option<String> {
    let en = lang == "en";
    let t = match code {
        "unreachable" => if en { "Start AIODOS, or connect the app: it will open AIODOS to applications." } else { "Lance AIODOS, ou connecte l'appli : elle l'ouvrira aux applications." },
        "connection_lost" => if en { "Check that AIODOS is still running, then try again." } else { "Vérifie qu'AIODOS tourne encore, puis réessaie." },
        "not_aiodos" => if en { "Free the port, or change AIODOS's port in its settings." } else { "Libère le port, ou change celui d'AIODOS dans ses réglages." },
        "too_old" | "not_supported" => if en { "Update AIODOS." } else { "Mets AIODOS à jour." },
        "untrusted" => if en { "Check that AIODOS is running, then reconnect the app if needed." } else { "Vérifie qu'AIODOS est bien lancé, puis reconnecte l'appli si besoin." },
        "timeout" => if en { "Try again, or pick a lighter model." } else { "Réessaie, ou choisis un modèle plus léger." },
        "not_installed" => if en { "Install AIODOS from its project page." } else { "Installe AIODOS depuis sa page de projet." },
        "needs_access" => if en { "Connect the app to AIODOS, after a click from the user." } else { "Connecte l'appli à AIODOS, après un clic de l'utilisateur." },
        "bad_key" => if en { "Reconnect the app to AIODOS." } else { "Reconnecte l'appli à AIODOS." },
        "access_denied" | "expired_token" | "invalid_grant" => if en { "Ask for access again, after a click from the user." } else { "Redemande l'accès, après un clic de l'utilisateur." },
        _ => return None,
    };
    Some(t.to_owned())
}

const RETRYABLE: [&str; 10] = ["unreachable", "timeout", "rate_limited", "busy", "peer_failed", "engine_failed", "engine_unavailable", "no_machine", "model_unavailable", "quota_exceeded"];

fn make_error(code: &str, message: String) -> AiError {
    AiError { retryable: RETRYABLE.contains(&code), ..AiError::with_code(code, redact(&message)) }
}

/// Erreur propre au client ; `text_key` choisit un autre texte que celui du code.
fn client_error(code: &str, lang: &str, text_key: Option<&str>, var: &str) -> AiError {
    let key = text_key.unwrap_or(code);
    let mut e = make_error(code, code_text(key, lang, var).unwrap_or_else(|| code.to_owned()));
    e.hint = hint_text(key, lang);
    e
}

/// Masque toute clé dans un texte (l'identifiant public d'une clé d'appli reste lisible).
pub fn redact(text: &str) -> String {
    let app = regex::Regex::new(r"aiodos_app_([a-z0-9]{8})_[A-Za-z0-9_-]{20,}").expect("motif");
    let general = regex::Regex::new(r"aiodos-[A-Za-z0-9_-]{40,}").expect("motif");
    let bearer = regex::Regex::new(r"(?i)(Bearer\s+)[A-Za-z0-9._~+/=-]{8,}").expect("motif");
    let t = app.replace_all(text, "aiodos_app_${1}_…");
    let t = general.replace_all(&t, "aiodos-…");
    bearer.replace_all(&t, "${1}…").into_owned()
}

fn code_for_status(status: Option<u16>) -> &'static str {
    match status {
        Some(400) => "invalid_request",
        Some(401) => "invalid_api_key",
        Some(404) => "not_supported",
        Some(413) => "file_too_large",
        Some(415) => "unsupported_media_type",
        Some(429) => "rate_limited",
        Some(502) => "engine_failed",
        Some(503) => "engine_unavailable",
        _ => "protocol",
    }
}

const OAUTH_ERRORS: [&str; 8] = ["authorization_pending", "slow_down", "access_denied", "expired_token", "invalid_grant", "invalid_request", "invalid_client", "unsupported_grant_type"];

/// Lit une réponse d'erreur, quelle qu'en soit la forme (enveloppe d'OpenAI, d'Ollama, d'OAuth).
fn error_from_response(status: Option<u16>, json: &Value, retry_after: Option<&str>, lang: &str) -> AiError {
    let mut code: Option<String> = None;
    let mut message: Option<String> = None;
    let mut hint: Option<String> = None;
    let mut retryable: Option<bool> = None;
    let s = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
    match json.get("error") {
        Some(e @ Value::Object(_)) => {
            code = s(e.get("code"));
            message = s(e.get("message"));
            if let Some(a) = e.get("aiodos").filter(|a| a.is_object()) {
                hint = s(a.get("hint"));
                retryable = a.get("retryable").and_then(Value::as_bool);
            }
        }
        Some(Value::String(e)) => {
            if json.get("error_description").is_some_and(Value::is_string) || (OAUTH_ERRORS.contains(&e.as_str()) && !truthy(json.get("code"))) {
                code = Some(e.clone());
                message = s(json.get("error_description"));
            } else {
                code = s(json.get("code"));
                message = Some(e.clone());
                hint = s(json.get("hint"));
            }
        }
        _ => {}
    }
    let derived = code.is_none();
    let code = code.unwrap_or_else(|| code_for_status(status).to_owned());
    let st = status.map_or_else(|| "?".to_owned(), |s| s.to_string());
    let message = message.unwrap_or_else(|| if derived && code == "protocol" { code_text("http", lang, &st) } else { code_text(&code, lang, &st) }.or_else(|| code_text("http", lang, &st)).unwrap_or_default());
    let mut e = make_error(&code, message);
    e.status = status;
    e.hint = hint.map(|h| redact(&h));
    if let Some(r) = retryable {
        e.retryable = r;
    }
    e.retry_after = retry_after.and_then(|v| v.trim().parse::<f64>().ok());
    e
}

// ------------------------------------------------------------------ preuve d'identité
/// Forme d'une clé d'appli : aiodos_app_<id base32, 8>_<secret base64url, 43>.
pub fn is_app_key(key: &str) -> bool {
    let Some(rest) = key.strip_prefix("aiodos_app_") else { return false };
    let b = rest.as_bytes();
    b.len() == 8 + 1 + 43 && b[..8].iter().all(|c| matches!(c, b'a'..=b'z' | b'2'..=b'7')) && b[8] == b'_' && b[9..].iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
}

fn key_id_of(key: &str) -> Option<&str> {
    if is_app_key(key) { key.get(11..19) } else { None }
}

/// La preuve attendue : base64url(HMAC-SHA256(clé = SHA-256(clé d'accès), « aiodos-proof/1\n » + défi)).
pub fn compute_proof(key: &str, challenge: &str) -> String {
    let secret = ring::digest::digest(&ring::digest::SHA256, key.as_bytes());
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_ref());
    base64url_encode(ring::hmac::sign(&k, format!("{PROOF_LABEL}{challenge}").as_bytes()).as_ref())
}

/// Compare la preuve reçue à celle attendue, en temps constant.
fn proof_matches(key: &str, challenge: &str, proof: Option<&str>) -> bool {
    let Some(given) = proof.and_then(base64url_decode_43) else { return false };
    let secret = ring::digest::digest(&ring::digest::SHA256, key.as_bytes());
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_ref());
    ring::hmac::verify(&k, format!("{PROOF_LABEL}{challenge}").as_bytes(), &given).is_ok()
}

fn new_challenge() -> Result<String, AiError> {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 32];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| AiError::with_code("protocol", "Tirage aléatoire impossible"))?;
    Ok(base64url_encode(&b))
}

/// Empreinte d'une clé (savoir si elle a changé sans la garder).
fn key_fingerprint(key: &str) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA256, format!("aiodos-client/fp\n{key}").as_bytes()).as_ref().to_vec()
}

// ------------------------------------------------------------------ adresses et découverte
/// L'origine (« http://127.0.0.1:11435 ») d'une adresse locale ; toute autre adresse est refusée
/// sans rien envoyer.
pub fn local_origin(raw: &str, lang: &str) -> Result<String, AiError> {
    match Url::parse(raw) {
        Ok(u) if u.is_local() => Ok(u.origin()),
        Ok(u) => Err(client_error("non_local_url", lang, None, &u.origin())),
        Err(_) => Err(client_error("non_local_url", lang, None, "?")),
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Le chemin du fichier de découverte (Windows : %LOCALAPPDATA%\AIODOS\gateway.json).
fn discovery_path() -> Option<PathBuf> {
    match env("AIODOS_DISCOVERY_FILE") {
        Some(f) if f == "-" => None,
        Some(f) => Some(PathBuf::from(f)),
        None => {
            if cfg!(windows) {
                Some(PathBuf::from(env("LOCALAPPDATA")?).join("AIODOS").join("gateway.json"))
            } else {
                let state = env("XDG_STATE_HOME").map(PathBuf::from).or_else(|| env("HOME").map(|h| Path::new(&h).join(".local").join("state")))?;
                Some(state.join("aiodos").join("gateway.json"))
            }
        }
    }
}

/// Le contenu du fichier de découverte s'il vient d'AIODOS (lu, jamais exécuté), sinon None.
fn read_discovery() -> Option<Value> {
    let file = discovery_path()?;
    let meta = std::fs::metadata(&file).ok()?;
    if !meta.is_file() || meta.len() > 64 * 1024 {
        return None;
    }
    let data: Value = serde_json::from_str(&std::fs::read_to_string(&file).ok()?).ok()?;
    if data.get("product").and_then(Value::as_str) != Some("aiodos") {
        return None;
    }
    if data.get("format").and_then(Value::as_f64).is_some_and(|f| f < 1.0) {
        return None;
    }
    if data.get("gateway").is_some_and(|g| !g.is_null() && !g.is_object()) {
        return None;
    }
    Some(data)
}

fn discovery_url(disc: &Value) -> Option<String> {
    let url = disc.get("gateway")?.get("url")?.as_str()?;
    local_origin(url, "fr").ok()
}

/// Le programme `pid` vit-il ? (None : on ne sait pas)
fn pid_alive(pid: Option<&Value>) -> Option<bool> {
    let pid = pid?.as_u64().filter(|p| *p > 0 && *p <= u64::from(u32::MAX))? as u32;
    process_alive(pid)
}

#[cfg(windows)]
fn process_alive(pid: u32) -> Option<bool> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY : appels Win32 sans pointeur partagé ; la poignée ouverte est refermée.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return Some(GetLastError() == ERROR_ACCESS_DENIED); // il existe, sous un autre compte
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        if ok == 0 { None } else { Some(code == 259) } // STILL_ACTIVE
    }
}

#[cfg(not(windows))]
fn process_alive(_: u32) -> Option<bool> {
    None
}

/// L'installation d'AIODOS trouvée sur ce PC.
#[derive(Debug, Clone)]
struct Install {
    root: PathBuf,
    dev: bool,
    cwd: PathBuf,
    python: Option<PathBuf>,
    pythonw: Option<PathBuf>,
}

#[cfg(windows)]
fn hide_window(cmd: &mut std::process::Command, flags: u32) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(flags);
}

#[cfg(not(windows))]
fn hide_window(_: &mut std::process::Command, _: u32) {}

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED: u32 = 0x0000_0008 | 0x0000_0200; // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP

/// Une valeur texte du registre (REG_SZ, ou REG_EXPAND_SZ développée), lue par l'API de Windows
/// (RegGetValueW) : en UTF-16, donc juste quels que soient les accents du chemin. La sortie de
/// « reg query », lue auparavant, est dans la page de codes OEM de la console, pas en UTF-8.
#[cfg(windows)]
fn registry_string(hive: windows_sys::Win32::System::Registry::HKEY, subkey: &str, name: &str) -> Option<std::ffi::OsString> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{REG_EXPAND_SZ, REG_SZ, RRF_RT_ANY, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (subkey, name) = (wide(subkey), wide(name));
    let mut ty = 0u32;
    let mut size = 0u32;
    // SAFETY : chaînes terminées par un zéro ; sans tampon, Windows ne donne que la taille.
    let rc = unsafe { RegGetValueW(hive, subkey.as_ptr(), name.as_ptr(), RRF_RT_ANY, &mut ty, std::ptr::null_mut(), &mut size) };
    if rc != ERROR_SUCCESS || (ty != REG_SZ && ty != REG_EXPAND_SZ) {
        return None;
    }
    // la valeur peut grandir entre deux appels (ou au développement des variables) : quelques essais
    for _ in 0..4 {
        let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
        let mut bytes = u32::try_from(buf.len() * 2).ok()?;
        // SAFETY : `bytes` est la taille réelle du tampon, en octets.
        let rc = unsafe { RegGetValueW(hive, subkey.as_ptr(), name.as_ptr(), RRF_RT_ANY, &mut ty, buf.as_mut_ptr().cast(), &mut bytes) };
        if rc == ERROR_MORE_DATA {
            size = bytes.max(size.saturating_mul(2));
            continue;
        }
        if rc != ERROR_SUCCESS || (ty != REG_SZ && ty != REG_EXPAND_SZ) {
            return None;
        }
        buf.truncate((bytes as usize / 2).min(buf.len()));
        // jusqu'au premier zéro, sans les espaces autour
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let text = &buf[..end];
        let start = text.iter().position(|&c| c != u16::from(b' ')).unwrap_or(text.len());
        let stop = text.iter().rposition(|&c| c != u16::from(b' ')).map_or(start, |i| i + 1);
        return (start < stop).then(|| std::ffi::OsString::from_wide(&text[start..stop]));
    }
    None
}

/// Les racines indiquées par le registre (HKCU puis HKLM).
#[cfg(windows)]
fn registry_roots() -> Vec<PathBuf> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE].into_iter().filter_map(|hive| registry_string(hive, UNINSTALL_KEY, "InstallLocation")).map(PathBuf::from).collect()
}

#[cfg(not(windows))]
fn registry_roots() -> Vec<PathBuf> {
    Vec::new()
}

/// L'installation : AIODOS_HOME (seul, s'il est donné) ; sinon le registre puis
/// %LOCALAPPDATA%\Programs\AIODOS.
fn find_install() -> Option<Install> {
    let mut roots = Vec::new();
    if let Some(h) = env("AIODOS_HOME") {
        roots.push(PathBuf::from(h));
    } else if cfg!(windows) {
        roots.extend(registry_roots());
        if let Some(local) = env("LOCALAPPDATA") {
            roots.push(Path::new(&local).join("Programs").join("AIODOS"));
        }
    }
    for root in roots {
        if root.join("aiodos").join("__init__.py").exists() {
            // copie de développement : l'interpréteur vient d'AIODOS_PYTHON, sinon rien n'est lancé
            let py = env("AIODOS_PYTHON").map(PathBuf::from);
            return Some(Install { cwd: root.clone(), root, dev: true, python: py.clone(), pythonw: py });
        }
        let runtime = root.join("runtime");
        let (python, pythonw) = (runtime.join("python.exe"), runtime.join("pythonw.exe"));
        if python.exists() || pythonw.exists() {
            return Some(Install { cwd: root.join("app"), root, dev: false, python: Some(python), pythonw: Some(pythonw) });
        }
    }
    None
}

/// Le programme à lancer : pythonw.exe pour AIODOS, python.exe (sans fenêtre) pour le programme d'aide.
fn program_for(gui: bool, lang: &str) -> Result<(Install, PathBuf), AiError> {
    let inst = find_install().ok_or_else(|| client_error("not_installed", lang, None, ""))?;
    let exe = if gui { inst.pythonw.clone() } else { inst.python.clone() };
    let Some(exe) = exe else { return Err(client_error("not_installed", lang, Some("dev_copy"), &inst.root.display().to_string())) };
    if inst.dev || exe.exists() {
        return Ok((inst, exe));
    }
    let other = if gui { inst.python.clone() } else { inst.pythonw.clone() };
    match other.filter(|o| o.exists()) {
        Some(o) => Ok((inst, o)),
        None => Err(client_error("not_installed", lang, Some("program_missing"), &exe.display().to_string())),
    }
}

/// Lancement d'AIODOS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub launched: bool,
    pub already_running: bool,
}

/// La demande d'accès en cours.
#[derive(Debug, Clone)]
struct Pending {
    user_code: Option<String>,
    expires_at: Option<Instant>,
    cancel: Cancel,
}

/// Une demande d'accès acceptée par AIODOS (code à montrer, relève à faire).
#[derive(Debug, Clone)]
struct AccessRequest {
    origin: String,
    device_code: String,
    user_code: String,
    interval: f64,
    expires_at: Instant,
    upgrade: bool,
}

/// Les droits accordés.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grant {
    /// droits en plus pour la même clé
    pub upgraded: bool,
    pub scopes: Vec<String>,
    pub where_: Vec<String>,
}

/// L'état d'AIODOS pour les réglages de l'assistant (jamais la clé).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiodosStatus {
    /// not_installed, not_running, gateway_closed, gateway_error, apps_closed, not_aiodos,
    /// too_old, untrusted, headless, requests_closed, needs_access, pending, denied, revoked,
    /// other_account, ready
    pub state: String,
    pub message: String,
    /// code de l'erreur qui a donné cet état
    pub code: Option<String>,
    pub url: Option<String>,
    pub version: Option<String>,
    pub installed: Option<bool>,
    pub running: Option<bool>,
    pub features: Vec<String>,
    pub scopes: Vec<String>,
    pub where_: Vec<String>,
    /// une demande d'accès attend l'utilisateur
    pub pending: bool,
    /// le code à 6 chiffres de cette demande (à ne montrer que sur cet ordinateur)
    pub code_to_show: Option<String>,
}

impl AiodosStatus {
    /// Pour l'interface (JSON).
    pub fn to_json(&self) -> Value {
        json!({
            "state": self.state, "message": self.message, "version": self.version, "installed": self.installed,
            "running": self.running, "scopes": self.scopes, "where": self.where_, "pending": self.pending,
        })
    }
}

/// Réglages du client AIODOS.
#[derive(Debug, Clone)]
pub struct AiodosOptions {
    /// nom montré dans AIODOS
    pub app: String,
    /// identifiant stable ([a-z0-9.-], 2 à 40)
    pub app_id: String,
    pub version: String,
    /// adresse imposée (sinon AIODOS_URL, le fichier de découverte, puis 127.0.0.1:11435)
    pub url: Option<String>,
    /// lire l'environnement (AIODOS_URL, AIODOS_HOME…), le fichier de découverte et le registre ;
    /// faux dans les essais
    pub system: bool,
    /// délai d'inactivité d'une réponse
    pub timeout: Duration,
    /// délai de chargement d'un modèle
    pub load_timeout: Duration,
}

impl AiodosOptions {
    /// Ceux d'ARKLIGHT.
    pub fn arklight(version: &str) -> AiodosOptions {
        AiodosOptions { app: "ARKLIGHT".into(), app_id: "arkane.arklight".into(), version: version.into(), url: None, system: true, timeout: crate::providers::TIMEOUT, load_timeout: crate::providers::LOAD_TIMEOUT }
    }
}

#[derive(Default)]
struct Session {
    /// (origine, empreinte de la clé, instance, moment)
    proof: Option<(String, Vec<u8>, Option<String>, Instant)>,
    /// dernière info sans clé : (origine, données, moment)
    info: Option<(String, Value, Instant)>,
}

/// Le client AIODOS d'ARKLIGHT. Partagé entre fils (`Arc<Aiodos>`) : la connexion (bloquante,
/// sur un fil de travail) s'annule par [`Aiodos::cancel`] depuis l'interface.
pub struct Aiodos {
    opts: AiodosOptions,
    keys: Arc<KeyStore>,
    lang: Mutex<String>,
    session: Mutex<Session>,
    pending: Mutex<Option<Pending>>,
    denied: Mutex<bool>,
    loaded: Mutex<HashMap<String, (bool, Instant)>>,
}

impl std::fmt::Debug for Aiodos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[aiodos {} · {}]", self.opts.app, self.opts.app_id)
    }
}

impl Aiodos {
    pub fn new(opts: AiodosOptions, keys: Arc<KeyStore>) -> Aiodos {
        Aiodos { opts, keys, lang: Mutex::new("fr".into()), session: Mutex::default(), pending: Mutex::default(), denied: Mutex::new(false), loaded: Mutex::default() }
    }

    /// Langue des messages (« fr » ou « en »).
    pub fn set_lang(&self, lang: &str) {
        if let Ok(mut l) = self.lang.lock() {
            *l = if lang.trim().to_ascii_lowercase().starts_with("en") { "en".into() } else { "fr".into() };
        }
    }

    fn lang(&self) -> String {
        self.lang.lock().map(|l| l.clone()).unwrap_or_else(|_| "fr".into())
    }

    fn forget(&self) {
        if let Ok(mut s) = self.session.lock() {
            *s = Session::default();
        }
    }

    /// L'adresse : celle des réglages, AIODOS_URL, le fichier de découverte, le défaut.
    fn resolve_origin(&self) -> Result<String, AiError> {
        let lang = self.lang();
        if let Some(u) = &self.opts.url {
            return local_origin(u, &lang);
        }
        if self.opts.system {
            if let Some(u) = env("AIODOS_URL") {
                return local_origin(&u, &lang);
            }
            if let Some(u) = read_discovery().as_ref().and_then(discovery_url) {
                return Ok(u);
            }
        }
        Ok(DEFAULT_URL.into())
    }

    /// La clé rangée (relue à chaque appel), ou None.
    fn read_key(&self) -> Option<String> {
        let k = self.keys.get("aiodos");
        let k = k.trim();
        if k.is_empty() { None } else { Some(k.to_owned()) }
    }

    fn require_key(&self) -> Result<String, AiError> {
        let lang = self.lang();
        let key = self.read_key().ok_or_else(|| client_error("needs_access", &lang, None, ""))?;
        if !is_app_key(&key) {
            return Err(client_error("needs_access", &lang, Some("bad_key"), ""));
        }
        Ok(key)
    }

    // -------------------------------------------------------------- transport
    /// Une requête à la passerelle (hôte vérifié avant tout envoi, aucune redirection).
    #[allow(clippy::too_many_arguments)]
    fn send(&self, origin: &str, method: &str, path: &str, body: Option<&Value>, key: Option<&str>, headers: &[(&str, String)], timeout: Duration, stream: bool, cancel: Option<&Cancel>) -> Result<Response, AiError> {
        let lang = self.lang();
        let origin = local_origin(origin, &lang)?;
        let host = Url::parse(&origin).map(|u| u.authority()).unwrap_or_default();
        let mut req = Request::new(method, &format!("{origin}{path}")).timeout(timeout);
        req = req.header("Accept", if stream { "text/event-stream" } else { "application/json" }).header("Accept-Language", &lang).header("X-Aiodos-Client", CLIENT_HEADER);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        if let Some(k) = key {
            req = req.header("Authorization", &format!("Bearer {k}"));
        }
        let res = req.send(cancel).map_err(|e| {
            let err = match e {
                HttpError::Cancelled => client_error("cancelled", &lang, None, ""),
                HttpError::Timeout => client_error("timeout", &lang, None, ""),
                HttpError::Refused(_) => client_error("unreachable", &lang, None, &host),
                HttpError::Io(_) => client_error("unreachable", &lang, Some("connection_lost"), &host),
                HttpError::BadUrl(u) => client_error("non_local_url", &lang, None, &u),
                HttpError::Protocol(_) => client_error("protocol", &lang, None, ""),
            };
            if matches!(err.code(), "unreachable" | "timeout") {
                self.forget(); // la preuve sera refaite
            }
            err
        })?;
        if (300..400).contains(&res.status) {
            let mut e = client_error("protocol", &lang, Some("redirect"), &host);
            e.status = Some(res.status);
            return Err(e);
        }
        Ok(res)
    }

    /// Lit une réponse entière : (statut, en-tête X-Aiodos-Api présent, Retry-After, JSON).
    fn read(&self, res: Response) -> Result<(u16, bool, Option<String>, Value), AiError> {
        let status = res.status;
        let from_aiodos = res.header("x-aiodos-api").is_some();
        let retry = res.header("retry-after").map(str::to_owned);
        let lang = self.lang();
        let (_, json) = res.text_and_json().map_err(|e| match e {
            HttpError::Cancelled => client_error("cancelled", &lang, None, ""),
            HttpError::Timeout => client_error("timeout", &lang, None, ""),
            _ => client_error("protocol", &lang, None, ""),
        })?;
        Ok((status, from_aiodos, retry, json))
    }

    fn expect_ok(&self, (status, _, retry, json): (u16, bool, Option<String>, Value)) -> Result<Value, AiError> {
        if (200..300).contains(&status) { Ok(json) } else { Err(error_from_response(Some(status), &json, retry.as_deref(), &self.lang())) }
    }

    // -------------------------------------------------------------- session
    /// GET /aiodos/v1/info, avec ou sans clé, avec ou sans défi ; reconnaît AIODOS.
    fn fetch_info(&self, origin: &str, query: &str, key: Option<&str>, cancel: Option<&Cancel>) -> Result<Value, AiError> {
        let lang = self.lang();
        let host = Url::parse(origin).map(|u| u.authority()).unwrap_or_default();
        let res = self.send(origin, "GET", &format!("/aiodos/v1/info{query}"), None, key, &[], INFO_TIMEOUT, false, cancel)?;
        let r = self.read(res)?;
        let (status, from_aiodos, _, json) = &r;
        if (200..300).contains(status) {
            if json.get("product").and_then(Value::as_str) != Some("aiodos") {
                let mut e = client_error("not_aiodos", &lang, None, &host);
                e.status = Some(*status);
                return Err(e);
            }
            if json.get("api").and_then(Value::as_f64).is_none_or(|a| a < API_VERSION) {
                return Err(client_error("too_old", &lang, None, ""));
            }
            return Ok(json.clone());
        }
        if !from_aiodos {
            if *status == 401 && json.get("error").is_some_and(Value::is_string) {
                return Err(client_error("too_old", &lang, None, ""));
            }
            let mut e = client_error("not_aiodos", &lang, None, &host);
            e.status = Some(*status);
            return Err(e);
        }
        self.expect_ok(r)
    }

    /// L'info sans clé, gardée 60 s au plus.
    fn get_info(&self, origin: &str, fresh: bool, cancel: Option<&Cancel>) -> Result<Value, AiError> {
        if !fresh
            && let Ok(s) = self.session.lock()
                && let Some((o, d, at)) = &s.info
                    && o == origin && at.elapsed() < PROOF_TTL {
                        return Ok(d.clone());
                    }
        let data = self.fetch_info(origin, "", None, cancel)?;
        if let Ok(mut s) = self.session.lock() {
            let instance = data.get("instance").and_then(Value::as_str).map(str::to_owned);
            if s.proof.as_ref().is_some_and(|(o, _, i, _)| o == origin && *i != instance) {
                s.proof = None;
            }
            s.info = Some((origin.to_owned(), data.clone(), Instant::now()));
        }
        Ok(data)
    }

    /// L'instance du fichier de découverte a-t-elle changé depuis la preuve ?
    fn instance_moved(&self, origin: &str, instance: Option<&str>) -> bool {
        if !self.opts.system {
            return false;
        }
        let Some(disc) = read_discovery() else { return false };
        if disc.get("running") == Some(&Value::Bool(false)) {
            return false;
        }
        let Some(i) = disc.get("instance").and_then(Value::as_str) else { return false };
        discovery_url(&disc).as_deref() == Some(origin) && Some(i) != instance
    }

    /// Fait (ou reprend) la preuve d'identité pour cette clé ; « untrusted » si elle est fausse.
    fn ensure_proof(&self, origin: &str, key: &str, cancel: Option<&Cancel>) -> Result<(), AiError> {
        let fp = key_fingerprint(key);
        let cached = self.session.lock().ok().and_then(|s| s.proof.clone());
        if let Some((o, f, inst, at)) = cached
            && o == origin && f == fp && at.elapsed() < PROOF_TTL && !self.instance_moved(origin, inst.as_deref()) {
                return Ok(());
            }
        if let Ok(mut s) = self.session.lock() {
            s.proof = None;
        }
        let challenge = new_challenge()?;
        let key_id = key_id_of(key).unwrap_or("");
        let data = self.fetch_info(origin, &format!("?key_id={key_id}&challenge={challenge}"), None, cancel)?;
        if !proof_matches(key, &challenge, data.get("proof").and_then(Value::as_str)) {
            let host = Url::parse(origin).map(|u| u.authority()).unwrap_or_default();
            return Err(client_error("untrusted", &self.lang(), None, &host));
        }
        let mut rest = data.clone();
        if let Value::Object(m) = &mut rest {
            m.remove("proof");
        }
        if let Ok(mut s) = self.session.lock() {
            let instance = data.get("instance").and_then(Value::as_str).map(str::to_owned);
            s.info = Some((origin.to_owned(), rest, Instant::now()));
            s.proof = Some((origin.to_owned(), fp, instance, Instant::now()));
        }
        Ok(())
    }

    fn require_feature_in(&self, info: &Value, feature: &str) -> Result<(), AiError> {
        let ok = info.get("features").and_then(Value::as_array).is_some_and(|l| l.iter().any(|f| f.as_str() == Some(feature)));
        if ok { Ok(()) } else { Err(client_error("not_supported", &self.lang(), None, feature)) }
    }

    /// Une requête avec la clé, après une preuve juste (et la fonction annoncée par l'info).
    #[allow(clippy::too_many_arguments)]
    fn keyed(&self, method: &str, path: &str, body: Option<&Value>, headers: &[(&str, String)], timeout: Duration, stream: bool, feature: Option<&str>, cancel: Option<&Cancel>) -> Result<Response, AiError> {
        let key = self.require_key()?;
        let origin = self.resolve_origin()?;
        self.ensure_proof(&origin, &key, cancel)?;
        if let Some(f) = feature {
            let info = self.get_info(&origin, false, cancel)?;
            self.require_feature_in(&info, f)?;
        }
        let r = self.send(&origin, method, path, body, Some(&key), headers, timeout, stream, cancel);
        if r.as_ref().is_err_and(|e| !e.is_cancelled()) {
            self.forget();
        }
        r
    }

    // -------------------------------------------------------------- état
    /// L'état d'AIODOS pour cette appli. Ne renvoie jamais d'erreur.
    pub fn status(&self) -> AiodosStatus {
        let lang = self.lang();
        // une demande d'accès attend l'utilisateur (dès le clic, code connu ou pas encore)
        if let Some(p) = self.pending.lock().ok().and_then(|p| p.clone())
            && p.expires_at.is_none_or(|e| e > Instant::now()) {
                let url = self.resolve_origin().ok();
                return AiodosStatus {
                    state: "pending".into(),
                    message: state_text("pending", &lang, p.user_code.as_deref().unwrap_or("")),
                    url,
                    pending: true,
                    code_to_show: p.user_code,
                    ..AiodosStatus::default()
                };
            }
        self.compute_status(&lang)
    }

    fn compute_status(&self, lang: &str) -> AiodosStatus {
        let finish = |state: &str, mut st: AiodosStatus, message: Option<String>| {
            st.message = message.filter(|m| !m.is_empty()).unwrap_or_else(|| state_text(state, lang, ""));
            st.state = state.to_owned();
            st
        };
        let mut out = AiodosStatus::default();
        let disc = if self.opts.system { read_discovery() } else { None };
        let gateway = disc.as_ref().and_then(|d| d.get("gateway")).filter(|g| g.is_object()).cloned();
        let origin = match self.resolve_origin() {
            Ok(o) => o,
            Err(e) => {
                out.code = e.code.clone();
                return finish("not_aiodos", out, Some(e.message));
            }
        };
        out.url = Some(origin.clone());
        let disc_running = disc.as_ref().is_some_and(|d| d.get("running") != Some(&Value::Bool(false)) && pid_alive(d.get("pid")) != Some(false));
        let from_error = |err: AiError, mut out: AiodosStatus| -> AiodosStatus {
            let code = err.code().to_owned();
            out.code = Some(code.clone());
            match code.as_str() {
                "unreachable" | "cancelled" => {
                    let gw_state = gateway.as_ref().and_then(|g| g.get("state")).and_then(Value::as_str);
                    if disc_running && gw_state == Some("error") {
                        out.installed = Some(true);
                        out.running = Some(true);
                        let msg = gateway.as_ref().and_then(|g| g.get("error")).and_then(Value::as_str).map(str::to_owned);
                        return finish("gateway_error", out, msg);
                    }
                    if disc_running {
                        out.installed = Some(true);
                        out.running = Some(true);
                        return finish("gateway_closed", out, None);
                    }
                    let installed = disc.is_some() || (self.opts.system && find_install().is_some());
                    out.installed = Some(installed);
                    out.running = Some(false);
                    finish(if installed { "not_running" } else { "not_installed" }, out, None)
                }
                "too_old" | "other_account" | "untrusted" => {
                    out.installed = Some(true);
                    out.running = Some(true);
                    finish(&code, out, None)
                }
                "non_local_url" => finish("not_aiodos", out, Some(err.message)),
                _ => finish("not_aiodos", out, None),
            }
        };
        let info = match self.get_info(&origin, true, None) {
            Ok(i) => i,
            Err(e) => return from_error(e, out),
        };
        let list = |v: Option<&Value>| v.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>()).unwrap_or_default();
        out.installed = Some(true);
        out.running = Some(true);
        out.version = info.get("version").and_then(Value::as_str).map(str::to_owned);
        out.features = list(info.get("features"));
        let access = info.get("access").filter(|a| a.is_object());
        let access_is = |k: &str| access.and_then(|a| a.get(k)) == Some(&Value::Bool(false));
        if access_is("apps") {
            return finish("apps_closed", out, None);
        }
        let Some(key) = self.read_key() else {
            if self.denied.lock().map(|d| *d).unwrap_or(false) {
                out.code = Some("access_denied".into());
                return finish("denied", out, None);
            }
            if access_is("requests") {
                return finish("requests_closed", out, None);
            }
            if access_is("approval_ui") {
                return finish("headless", out, None);
            }
            return finish("needs_access", out, None);
        };
        if !is_app_key(&key) {
            out.code = Some("bad_key".into());
            return finish("needs_access", out, None);
        }
        let keyed = self.ensure_proof(&origin, &key, None).and_then(|()| self.fetch_info(&origin, "", Some(&key), None));
        let keyed = match keyed {
            Ok(k) => k,
            Err(e) => return from_error(e, out),
        };
        match keyed.get("auth").and_then(Value::as_str) {
            Some("valid") => {
                let app = keyed.get("app").cloned().unwrap_or(Value::Null);
                let f = list(keyed.get("features"));
                if !f.is_empty() {
                    out.features = f;
                }
                out.scopes = list(app.get("scopes"));
                out.where_ = list(app.get("where"));
                finish("ready", out, None)
            }
            Some(a @ ("revoked" | "expired")) => {
                out.code = Some(if a == "revoked" { "key_revoked" } else { "key_expired" }.into());
                finish("revoked", out, None)
            }
            _ => {
                out.code = Some("invalid_api_key".into());
                finish("needs_access", out, None)
            }
        }
    }

    /// Attend qu'AIODOS réponde (autre état que « pas lancé »), `timeout` au plus.
    pub fn wait_ready(&self, timeout: Duration, cancel: Option<&Cancel>) -> Result<AiodosStatus, AiError> {
        let deadline = Instant::now() + timeout;
        let lang = self.lang();
        loop {
            if cancel.is_some_and(Cancel::is_cancelled) {
                return Err(client_error("cancelled", &lang, None, ""));
            }
            let st = self.status();
            if st.state != "not_running" && st.state != "not_installed" {
                return Ok(st);
            }
            if st.state == "not_installed" {
                return Ok(st);
            }
            if Instant::now() + Duration::from_secs(1) > deadline {
                return Err(client_error("timeout", &lang, None, ""));
            }
            match cancel {
                Some(c) => c.sleep(Duration::from_secs(1)).map_err(|_| client_error("cancelled", &lang, None, ""))?,
                None => std::thread::sleep(Duration::from_secs(1)),
            }
        }
    }

    /// Lance AIODOS en arrière-plan (installé, arrêté), sur un clic de l'utilisateur ; rien n'est
    /// jamais installé.
    pub fn launch(&self) -> Result<Launch, AiError> {
        let lang = self.lang();
        if let Some(d) = read_discovery()
            && d.get("running") != Some(&Value::Bool(false)) && pid_alive(d.get("pid")) == Some(true) {
                return Ok(Launch { launched: false, already_running: true });
            }
        let (inst, exe) = program_for(true, &lang)?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["-m", "aiodos", "--background"]).current_dir(&inst.cwd).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        hide_window(&mut cmd, DETACHED);
        cmd.spawn().map_err(|_| client_error("not_installed", &lang, Some("program_missing"), &exe.display().to_string()))?;
        Ok(Launch { launched: true, already_running: false })
    }

    // -------------------------------------------------------------- accès
    /// Le corps de la demande d'accès, vérifié avant tout envoi.
    fn access_body(&self, lang: &str) -> Result<Value, AiError> {
        let invalid = |_param: &str, fr: &str, en: &str| make_error("invalid_request", if lang == "en" { en } else { fr }.to_owned());
        let name: String = self.opts.app.chars().filter(|c| !c.is_control() && !matches!(*c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')).collect::<String>().trim().to_owned();
        if name.is_empty() || name.chars().count() > 40 || RESERVED_NAMES.contains(&name.to_lowercase().as_str()) {
            return Err(invalid("app", "Nom d'appli invalide (1 à 40 caractères, ni « AIODOS », « Autres outils », « Windows », « Microsoft »).", "Invalid app name (1 to 40 characters, not “AIODOS”, “Autres outils”, “Windows”, “Microsoft”)."));
        }
        let id = &self.opts.app_id;
        if !(2..=40).contains(&id.len()) || !id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-') {
            return Err(invalid("appId", "Identifiant d'appli invalide ([a-z0-9.-], 2 à 40 caractères).", "Invalid app id ([a-z0-9.-], 2 to 40 characters)."));
        }
        if self.opts.version.chars().count() > 40 {
            return Err(invalid("version", "Version trop longue (40 caractères au plus).", "Version too long (40 characters at most)."));
        }
        let why = purpose(lang);
        let mut body = Map::new();
        body.insert("client_name".into(), name.into());
        body.insert("client_id".into(), id.as_str().into());
        if !self.opts.version.is_empty() {
            body.insert("client_version".into(), self.opts.version.as_str().into());
        }
        body.insert("scope".into(), AIODOS_SCOPES.join(" ").into());
        debug_assert!(AIODOS_WHERE.iter().all(|w| CIRCLES.contains(w)));
        body.insert("where".into(), json!(AIODOS_WHERE));
        body.insert("purpose".into(), why.into());
        Ok(Value::Object(body))
    }

    /// La demande d'accès par la passerelle ouverte (POST /aiodos/v1/access/requests). Avec une
    /// clé valide (après la preuve), c'est une demande de droits en plus pour la même clé.
    fn request_access(&self, origin: &str, body: &Value, cancel: &Cancel) -> Result<AccessRequest, AiError> {
        let lang = self.lang();
        let info = self.get_info(origin, false, Some(cancel))?;
        self.require_feature_in(&info, "access")?;
        let post = |key: Option<&str>| self.send(origin, "POST", "/aiodos/v1/access/requests", Some(body), key, &[], REQUEST_TIMEOUT, false, Some(cancel)).and_then(|r| self.read(r));
        let mut res = None;
        let mut upgrade = false;
        if let Some(key) = self.read_key().filter(|k| is_app_key(k)) {
            self.ensure_proof(origin, &key, Some(cancel))?; // preuve fausse : rejet, clé non envoyée
            let r = post(Some(&key))?;
            upgrade = (200..300).contains(&r.0);
            if r.0 != 401 {
                res = Some(r); // 401 : clé retirée ou inconnue, nouvelle demande sans elle
            }
        }
        let r = match res {
            Some(r) => r,
            None => post(None)?,
        };
        let d = self.expect_ok(r)?;
        let (Some(device), Some(user)) = (d.get("device_code").and_then(Value::as_str), d.get("user_code").and_then(Value::as_str)) else {
            return Err(client_error("protocol", &lang, None, ""));
        };
        Ok(AccessRequest {
            origin: origin.to_owned(),
            device_code: device.to_owned(),
            user_code: user.to_owned(),
            interval: interval_seconds(d.get("interval")),
            expires_at: Instant::now() + Duration::from_secs_f64(d.get("expires_in").and_then(Value::as_f64).filter(|n| *n > 0.0).unwrap_or(600.0)),
            upgrade,
        })
    }

    /// Le programme d'aide (python -m aiodos --app-request) quand la passerelle est fermée ou
    /// AIODOS arrêté : la demande en une ligne JSON sur l'entrée standard, la réponse sur la sortie.
    fn run_app_request(&self, body: &Value, origin: &str, cancel: &Cancel) -> Result<AccessRequest, AiError> {
        use std::io::{Read, Write};
        let lang = self.lang();
        if !self.opts.system {
            return Err(client_error("not_installed", &lang, None, ""));
        }
        let (inst, exe) = program_for(false, &lang)?;
        cancel.check().map_err(|_| client_error("cancelled", &lang, None, ""))?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["-m", "aiodos", "--app-request"]).current_dir(&inst.cwd).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        hide_window(&mut cmd, CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|_| client_error("not_installed", &lang, Some("program_missing"), &exe.display().to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(format!("{body}\n").as_bytes());
        }
        let mut stdout = child.stdout.take();
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            if let Some(s) = stdout.as_mut() {
                let _ = s.take(64 * 1024).read_to_end(&mut out);
            }
            out
        });
        let start = Instant::now();
        let status = loop {
            if let Ok(Some(st)) = child.try_wait() {
                break st;
            }
            if cancel.is_cancelled() || start.elapsed() > HELPER_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(client_error(if cancel.is_cancelled() { "cancelled" } else { "timeout" }, &lang, None, ""));
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let out = String::from_utf8_lossy(&reader.join().unwrap_or_default()).into_owned();
        let data: Value = out.lines().map(str::trim).find(|l| l.starts_with('{')).and_then(|l| serde_json::from_str(l).ok()).unwrap_or(Value::Null);
        if !status.success() {
            if let Some(e) = data.get("error").filter(|e| e.is_object())
                && let Some(code) = e.get("code").and_then(Value::as_str) {
                    return Err(make_error(code, e.get("message").and_then(Value::as_str).unwrap_or(code).to_owned()));
                }
            return Err(client_error("protocol", &lang, None, ""));
        }
        let (Some(device), Some(user)) = (data.get("device_code").and_then(Value::as_str), data.get("user_code").and_then(Value::as_str)) else {
            return Err(client_error("protocol", &lang, None, ""));
        };
        let url = data.get("url").and_then(Value::as_str).and_then(|u| local_origin(u, &lang).ok()).unwrap_or_else(|| origin.to_owned());
        Ok(AccessRequest {
            origin: url,
            device_code: device.to_owned(),
            user_code: user.to_owned(),
            interval: interval_seconds(Some(&Value::from(data.get("interval").and_then(Value::as_f64).filter(|n| *n != 0.0).unwrap_or(2.0)))),
            expires_at: Instant::now() + Duration::from_secs_f64(data.get("expires_in").and_then(Value::as_f64).filter(|n| *n > 0.0).unwrap_or(600.0)),
            upgrade: false,
        })
    }

    /// Relève la clé (POST /aiodos/v1/access/token) jusqu'à la réponse de l'utilisateur.
    fn poll(&self, r: &AccessRequest, cancel: &Cancel) -> Result<Grant, AiError> {
        let lang = self.lang();
        let mut interval = Duration::from_secs_f64(r.interval);
        let body = json!({ "grant_type": GRANT_TYPE, "device_code": r.device_code });
        loop {
            if Instant::now() + interval > r.expires_at {
                return Err(client_error("expired_token", &lang, None, ""));
            }
            cancel.sleep(interval).map_err(|_| client_error("cancelled", &lang, None, ""))?;
            let res = match self.send(&r.origin, "POST", "/aiodos/v1/access/token", Some(&body), None, &[], POLL_TIMEOUT, false, Some(cancel)).and_then(|res| self.read(res)) {
                Ok(res) => res,
                // la passerelle n'écoute pas encore (« Autoriser » l'ouvrira) : on continue
                Err(e) if matches!(e.code(), "unreachable" | "timeout") => continue,
                Err(e) => return Err(e),
            };
            let (status, _, retry, d) = &res;
            if (200..300).contains(status) {
                let grant = |upgraded| {
                    let a = d.get("aiodos");
                    let words = |v: Option<&Value>| match v {
                        Some(Value::Array(a)) => a.iter().map(js_string).filter(|s| !s.is_empty()).collect(),
                        Some(Value::String(s)) => s.split([' ', ',']).filter(|s| !s.is_empty()).map(str::to_owned).collect(),
                        _ => Vec::new(),
                    };
                    Grant { upgraded, scopes: words(d.get("scope")), where_: words(a.and_then(|a| a.get("where"))) }
                };
                if let Some(token) = d.get("access_token").and_then(Value::as_str) {
                    if !is_app_key(token) {
                        return Err(client_error("protocol", &lang, None, ""));
                    }
                    self.keys.set("aiodos", token).map_err(|e| AiError::with_code("key_store", e.to_string()))?;
                    self.forget(); // nouvelle clé : nouvelle preuve
                    if let Ok(mut dn) = self.denied.lock() {
                        *dn = false;
                    }
                    return Ok(grant(false));
                }
                if d.get("aiodos").and_then(|a| a.get("upgraded")) == Some(&Value::Bool(true)) {
                    return Ok(grant(true));
                }
                return Err(client_error("protocol", &lang, None, ""));
            }
            let e = d.get("error").and_then(Value::as_str);
            if e == Some("authorization_pending") {
                continue;
            }
            if e == Some("slow_down") {
                interval += Duration::from_secs(5);
                continue;
            }
            let err = error_from_response(Some(*status), d, retry.as_deref(), &lang);
            if e == Some("access_denied") {
                if !r.upgrade
                    && let Ok(mut dn) = self.denied.lock() {
                        *dn = true;
                    }
                return Err(err);
            }
            if (*status == 429 || *status == 503) && err.retryable {
                interval = interval.max(Duration::from_secs_f64(err.retry_after.unwrap_or(0.0)));
                continue;
            }
            return Err(err);
        }
    }

    /// Se connecter à AIODOS (sur un clic seulement, bloquant) : `on_code(code)` reçoit le code à
    /// 6 chiffres à afficher en grand ; rend les droits accordés quand AIODOS a dit oui. La clé est
    /// rangée dans le coffre (« aiodos »). [`Aiodos::cancel`] abandonne.
    pub fn connect(&self, on_code: &mut dyn FnMut(&str)) -> Result<Grant, AiError> {
        let lang = self.lang();
        let cancel = Cancel::new();
        {
            let mut p = self.pending.lock().map_err(|_| AiError::new("AIODOS indisponible"))?;
            if p.is_some() {
                return Err(AiError::with_code("busy", if lang == "en" { "An access request is already waiting." } else { "Une demande d'accès attend déjà." }));
            }
            *p = Some(Pending { user_code: None, expires_at: None, cancel: cancel.clone() });
        }
        let r = self.connect_inner(&lang, &cancel, on_code);
        if let Ok(mut p) = self.pending.lock() {
            *p = None;
        }
        r
    }

    fn connect_inner(&self, lang: &str, cancel: &Cancel, on_code: &mut dyn FnMut(&str)) -> Result<Grant, AiError> {
        let body = self.access_body(lang)?; // vérifié avant tout
        let origin = self.resolve_origin()?;
        let info = match self.get_info(&origin, true, Some(cancel)) {
            Ok(i) => Some(i),
            Err(e) if e.code() == "unreachable" => None,
            Err(e) => return Err(e),
        };
        let mut req = None;
        if let Some(i) = &info
            && i.get("access").and_then(|a| a.get("apps")) != Some(&Value::Bool(false)) {
                match self.request_access(&origin, &body, cancel) {
                    Ok(r) => req = Some(r),
                    // fermée aux applis entre-temps, ou ouverte à la seule clé générale : le programme d'aide
                    Err(e) if matches!(e.code(), "apps_closed" | "unreachable") => {}
                    Err(e) => return Err(e),
                }
            }
        let req = match req {
            Some(r) => r,
            None => self.run_app_request(&body, &origin, cancel)?,
        };
        if let Ok(mut p) = self.pending.lock()
            && let Some(p) = p.as_mut() {
                p.user_code = Some(req.user_code.clone());
                p.expires_at = Some(req.expires_at);
            }
        on_code(&req.user_code);
        self.poll(&req, cancel)
    }

    /// Abandonne la demande d'accès en cours.
    pub fn cancel(&self) {
        if let Ok(p) = self.pending.lock()
            && let Some(p) = p.as_ref() {
                p.cancel.cancel();
            }
    }

    /// Retire l'accès dans AIODOS et efface la clé (même si AIODOS ne répond pas) ; rend vrai si
    /// AIODOS a confirmé le retrait.
    pub fn disconnect(&self) -> Result<bool, AiError> {
        let mut revoked = false;
        if self.read_key().is_some_and(|k| is_app_key(&k)) {
            match self.keyed("POST", "/aiodos/v1/access/revoke", Some(&json!({})), &[], REVOKE_TIMEOUT, false, None, None).and_then(|r| self.read(r)) {
                Ok((status, _, _, d)) => revoked = (200..300).contains(&status) && d.get("revoked") == Some(&Value::Bool(true)),
                Err(e) if e.is_cancelled() => return Err(e),
                Err(_) => {} // retrait impossible : la clé est effacée quand même
            }
        }
        self.keys.set("aiodos", "").map_err(|e| AiError::with_code("key_store", e.to_string()))?;
        self.forget();
        if let Ok(mut d) = self.denied.lock() {
            *d = false;
        }
        Ok(revoked)
    }

    // -------------------------------------------------------------- modèles
    fn models_with(&self, timeout: Duration, cancel: Option<&Cancel>) -> Result<Vec<ModelInfo>, AiError> {
        let res = self.keyed("GET", "/aiodos/v1/models?kind=chat", None, &[], timeout, false, Some("models"), cancel)?;
        let r = self.read(res)?;
        let d = self.expect_ok(r)?;
        let list = d.get("models").and_then(Value::as_array).ok_or_else(|| client_error("protocol", &self.lang(), None, ""))?;
        let out: Vec<(ModelInfo, Vec<(String, bool)>)> = list.iter().filter(|m| m.get("id").is_some_and(Value::is_string)).map(to_model).collect();
        if let Ok(mut l) = self.loaded.lock() {
            let now = Instant::now();
            for (m, wh) in &out {
                l.insert(m.id.clone(), (m.loaded, now));
                for (id, loaded) in wh {
                    l.insert(id.clone(), (*loaded, now));
                }
            }
        }
        Ok(out.into_iter().map(|(m, _)| m).collect())
    }

    /// Modèles de conversation permis à ARKLIGHT, au format de la liste des réglages.
    pub fn models(&self) -> Result<Vec<ModelInfo>, AiError> {
        self.models_with(MODELS_TIMEOUT, None)
    }

    /// Le modèle conseillé : de ce PC, qui sait utiliser les outils, et qui voit les photos si
    /// possible ('' : aucun).
    pub fn pick_model(&self) -> Result<String, AiError> {
        Ok(pick_model(&self.models()?, &["tools"], true).map(|m| m.id.clone()).unwrap_or_default())
    }

    /// Le modèle est-il chargé ? (None : on ne sait pas)
    fn known_loaded(&self, model: &str, cancel: Option<&Cancel>) -> Result<Option<bool>, AiError> {
        let hit = self.loaded.lock().ok().and_then(|l| l.get(model).copied());
        if let Some((loaded, at)) = hit
            && at.elapsed() < LOADED_TTL {
                return Ok(Some(loaded));
            }
        match self.models_with(PROBE_TIMEOUT, cancel) {
            Ok(_) => Ok(self.loaded.lock().ok().and_then(|l| l.get(model).map(|(b, _)| *b))),
            Err(e) if e.is_cancelled() => Err(e),
            Err(_) => Ok(None), // la conversation dira ce qui ne va pas
        }
    }

    // -------------------------------------------------------------- conversation
    /// Un tour de conversation (POST /v1/chat/completions, en flux SSE), calculé sur ce PC.
    pub fn chat(&self, model: &str, o: &mut ChatOptions) -> Result<ChatReply, AiError> {
        let lang = self.lang();
        if model.trim().is_empty() {
            return Err(make_error("invalid_request", if lang == "en" { "Choose a model." } else { "Choisis un modèle." }.to_owned()));
        }
        let mut body = json!({
            "model": model,
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": to_openai_messages(o.system, o.messages)?,
            "reasoning_effort": "none",
            "temperature": 0.2,
        });
        if !o.tools.is_empty() {
            body["tools"] = Value::Array(o.tools.iter().map(|t| json!({ "type": "function", "function": t })).collect());
        }
        let headers = [("X-Aiodos-Where", AIODOS_WHERE.join(","))];
        let loaded = self.known_loaded(model, o.cancel)?;
        if loaded == Some(false) {
            (o.on_loading)(); // prévenu avant d'attendre
        }
        let first = if loaded == Some(false) { self.opts.load_timeout.max(self.opts.timeout) } else { self.opts.timeout };
        let mut res = self.keyed("POST", "/v1/chat/completions", Some(&body), &headers, first, true, Some("chat"), o.cancel)?;
        if !res.ok() {
            let r = self.read(res)?;
            return Err(self.expect_ok(r).err().unwrap_or_else(|| client_error("protocol", &lang, None, "")));
        }
        let header = |n: &str| res.header(n).map(percent_decode);
        let (h_model, h_machine, h_place) = (header("x-aiodos-model"), header("x-aiodos-machine"), res.header("x-aiodos-place").map(str::to_owned));
        let mut text = String::new();
        let mut calls: Vec<(i64, PartialCall)> = Vec::new();
        let mut meta = Value::Null;
        let mut chunk_model: Option<String> = None;
        let mut finish: Option<String> = None;
        let mut complete = false;
        let map_http = |e: HttpError| match e {
            HttpError::Cancelled => client_error("cancelled", &lang, None, ""),
            HttpError::Timeout => client_error("timeout", &lang, None, ""),
            _ => client_error("unreachable", &lang, Some("connection_lost"), "127.0.0.1"),
        };
        if res.header("content-type").is_some_and(|c| c.to_ascii_lowercase().contains("application/json")) {
            let (_, d) = res.text_and_json().map_err(map_http)?;
            if !d.is_object() {
                return Err(client_error("protocol", &lang, None, ""));
            }
            if d.get("error").is_some() {
                return Err(error_from_response(None, &d, None, &lang));
            }
            let choice = d.get("choices").and_then(|c| c.get(0));
            let msg = choice.and_then(|c| c.get("message"));
            text = msg.and_then(|m| m.get("content")).and_then(Value::as_str).unwrap_or("").to_owned();
            for (i, tc) in msg.and_then(|m| m.get("tool_calls")).and_then(Value::as_array).into_iter().flatten().enumerate() {
                let mut tc = tc.clone();
                tc["index"] = json!(i);
                add_tool_delta(&mut calls, &tc);
            }
            finish = choice.and_then(|c| c.get("finish_reason")).and_then(Value::as_str).map(str::to_owned);
            meta = d.get("aiodos").cloned().unwrap_or(Value::Null);
            chunk_model = d.get("model").and_then(Value::as_str).map(str::to_owned);
            complete = true;
        } else {
            let mut data: Vec<String> = Vec::new();
            let mut timeout_reset = false;
            loop {
                let line = res.next_line().map_err(map_http)?;
                if !timeout_reset {
                    res.set_timeout(self.opts.timeout); // signe de vie
                    timeout_reset = true;
                }
                let Some(line) = line else { break };
                if let Some(comment) = line.strip_prefix(':') {
                    if loading_seconds(comment.strip_prefix(' ').unwrap_or(comment)).is_some() {
                        (o.on_loading)();
                    }
                    continue;
                }
                if !line.is_empty() {
                    let (field, value) = line.split_once(':').map_or((line.as_str(), ""), |(f, v)| (f, v.strip_prefix(' ').unwrap_or(v)));
                    if field == "data" {
                        data.push(value.to_owned());
                    }
                    continue;
                }
                if data.is_empty() {
                    continue;
                }
                let payload = data.join("\n");
                data.clear();
                if payload == "[DONE]" {
                    complete = true;
                    break;
                }
                let d: Value = serde_json::from_str(&payload).map_err(|_| client_error("protocol", &lang, None, ""))?;
                if !d.is_object() {
                    return Err(client_error("protocol", &lang, None, ""));
                }
                if d.get("error").is_some() {
                    return Err(error_from_response(None, &d, None, &lang));
                }
                if let Some(m) = d.get("model").and_then(Value::as_str) {
                    chunk_model = Some(m.to_owned());
                }
                if d.get("aiodos").is_some_and(Value::is_object) {
                    meta = d["aiodos"].clone();
                }
                let Some(choice) = d.get("choices").and_then(|c| c.get(0)) else { continue };
                if let Some(delta) = choice.get("delta") {
                    if let Some(c) = delta.get("content").and_then(Value::as_str) {
                        text.push_str(c);
                    }
                    for tc in delta.get("tool_calls").and_then(Value::as_array).into_iter().flatten().filter(|t| t.is_object()) {
                        add_tool_delta(&mut calls, tc);
                    }
                }
                if let Some(f) = choice.get("finish_reason").and_then(Value::as_str) {
                    finish = Some(f.to_owned());
                }
            }
            // un événement sans ligne vide finale
            if !complete && !data.is_empty() && data.join("\n") == "[DONE]" {
                complete = true;
            }
        }
        if !complete && finish.is_none() {
            return Err(client_error("protocol", &lang, Some("cut"), ""));
        }
        calls.sort_by_key(|(i, _)| *i);
        let tool_calls = calls
            .into_iter()
            .filter(|(_, c)| !c.name.is_empty())
            .enumerate()
            .map(|(i, (_, c))| ToolCall { id: c.id.filter(|s| !s.is_empty()).unwrap_or_else(|| format!("t{}", i + 1)), name: c.name, args: tool_args(&c.args) })
            .collect();
        let m = |k: &str| meta.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned);
        let by = AnsweredBy { model: m("model").or(h_model).or(chunk_model).unwrap_or_else(|| model.to_owned()), machine: m("machine").or(h_machine).unwrap_or_default(), place: m("place").or(h_place).unwrap_or_default() };
        Ok(ChatReply { text, tool_calls, by: Some(by) })
    }
}

/// Un appel d'outil reçu en morceaux.
#[derive(Debug, Default)]
struct PartialCall {
    id: Option<String>,
    name: String,
    args: String,
}

/// Rassemble les morceaux de `tool_calls` par index (sans index : un nouvel identifiant ouvre un
/// nouvel appel, sinon on complète le dernier).
fn add_tool_delta(calls: &mut Vec<(i64, PartialCall)>, tc: &Value) {
    let index = match tc.get("index").and_then(Value::as_i64) {
        Some(i) => i,
        None => {
            let last = calls.iter().map(|(i, _)| *i).max();
            match last {
                None => 0,
                Some(l) => {
                    let id = tc.get("id").and_then(Value::as_str).filter(|s| !s.is_empty());
                    let last_id = calls.iter().find(|(i, _)| *i == l).and_then(|(_, c)| c.id.clone());
                    if id.is_some() && last_id.is_some() && id != last_id.as_deref() { l + 1 } else { l }
                }
            }
        }
    };
    let pos = match calls.iter().position(|(i, _)| *i == index) {
        Some(p) => p,
        None => {
            calls.push((index, PartialCall::default()));
            calls.len() - 1
        }
    };
    let c = &mut calls[pos].1;
    if let Some(id) = tc.get("id").and_then(Value::as_str).filter(|s| !s.is_empty())
        && c.id.is_none() {
            c.id = Some(id.to_owned());
        }
    let f = tc.get("function");
    if let Some(n) = f.and_then(|f| f.get("name")).and_then(Value::as_str).filter(|s| !s.is_empty())
        && c.name.is_empty() {
            c.name = n.to_owned();
        }
    match f.and_then(|f| f.get("arguments")) {
        Some(Value::String(s)) => c.args.push_str(s),
        Some(v @ Value::Object(_)) => c.args.push_str(&v.to_string()),
        _ => {}
    }
}

/// Arguments d'un appel : objet JSON (encodé deux fois chez certains modèles) ; illisibles : {}.
fn tool_args(raw: &str) -> Value {
    if raw.trim().is_empty() {
        return json!({});
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::String(s)) => serde_json::from_str::<Value>(&s).ok().filter(Value::is_object).unwrap_or_else(|| json!({})),
        Ok(v @ Value::Object(_)) => v,
        _ => json!({}),
    }
}

/// Les secondes d'un commentaire « aiodos loading N ».
fn loading_seconds(comment: &str) -> Option<f64> {
    let rest = comment.trim().strip_prefix("aiodos loading")?;
    let n = rest.trim();
    if n.is_empty() {
        return Some(0.0);
    }
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    n.parse().ok()
}

/// `decodeURIComponent` (en-têtes X-Aiodos-Model, X-Aiodos-Machine) ; texte mal codé : tel quel.
fn percent_decode(v: &str) -> String {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| v.to_owned())
}

fn interval_seconds(v: Option<&Value>) -> f64 {
    match v.and_then(Value::as_f64) {
        Some(n) if n.is_finite() && n > 0.0 => n.clamp(0.1, 60.0),
        _ => 5.0, // valeur de la RFC 8628 quand il manque
    }
}

/// Une fiche du catalogue d'AIODOS → modèle d'ARKLIGHT, et (identifiant, chargé) de chaque machine.
fn to_model(m: &Value) -> (ModelInfo, Vec<(String, bool)>) {
    let s = |k: &str| m.get(k).and_then(Value::as_str).map(str::to_owned);
    let b = |k: &str| m.get(k).and_then(Value::as_bool).unwrap_or(false);
    let id = s("id").unwrap_or_default();
    let where_: Vec<&Value> = m.get("where").and_then(Value::as_array).into_iter().flatten().filter(|w| w.get("id").is_some_and(Value::is_string)).collect();
    let main = where_.iter().find(|w| w.get("place").and_then(Value::as_str) == Some("here")).or(where_.first());
    let mut caps = Vec::new();
    if s("kind").as_deref() == Some("embed") {
        caps.push("embedding".to_owned());
    } else {
        caps.push("completion".to_owned());
        for (k, c) in [("tools", "tools"), ("vision", "vision"), ("thinking", "thinking")] {
            if b(k) {
                caps.push(c.to_owned());
            }
        }
    }
    let info = ModelInfo {
        size: m.get("size_bytes").and_then(Value::as_f64).unwrap_or(0.0),
        caps,
        params: params_of(&id),
        machine: main.and_then(|w| w.get("machine")).and_then(Value::as_str).unwrap_or("").to_owned(),
        here: b("here"),
        loaded: b("loaded"),
        default: m.get("default") == Some(&Value::Bool(true)),
        context: m.get("context").and_then(Value::as_f64),
        installed: m.get("installed") != Some(&Value::Bool(false)),
        id,
    };
    let wh = where_.iter().filter_map(|w| Some((w.get("id")?.as_str()?.to_owned(), w.get("loaded").and_then(Value::as_bool).unwrap_or(false)))).collect();
    (info, wh)
}

/// « 9B » pour « qwen3.5:9b », « 35B » pour « qwen3.6:35b-a3b », sinon ''.
pub fn params_of(id: &str) -> String {
    let s = id.to_lowercase();
    let tag = s.rfind(':').map_or("", |i| &s[i + 1..]);
    let a = regex::Regex::new(r"^(\d+(?:\.\d+)?)([bm])").expect("motif");
    if let Some(c) = a.captures(tag) {
        let next = tag[c.get(0).map_or(0, |m| m.end())..].chars().next();
        if !next.is_some_and(|ch| ch.is_ascii_lowercase()) {
            return format!("{}{}", &c[1], c[2].to_uppercase());
        }
    }
    let b = regex::Regex::new(r"(?:^|[-_:])(\d+(?:\.\d+)?)([bm])(?:$|[-_:])").expect("motif");
    b.captures(&s).map_or_else(String::new, |c| format!("{}{}", &c[1], c[2].to_uppercase()))
}

/// Le meilleur modèle pour un usage (client `pickModel`) : capacités exigées, ceux qui voient les
/// images d'abord si `vision`, puis ceux de ce PC, celui d'AIODOS, un modèle chargé, le plus léger.
pub fn pick_model<'a>(list: &'a [ModelInfo], needs: &[&str], vision: bool) -> Option<&'a ModelInfo> {
    let wants_embedding = needs.contains(&"embedding");
    let mut items: Vec<&ModelInfo> = list
        .iter()
        .filter(|m| m.installed)
        .filter(|m| wants_embedding || !(m.has("embedding") && !m.has("completion")))
        .filter(|m| needs.iter().all(|c| m.has(c)))
        .collect();
    let score = |m: &ModelInfo| (u8::from(!(vision && m.has("vision"))), u8::from(!m.here), u8::from(!m.default), u8::from(!m.loaded), if m.size > 0.0 { m.size } else { f64::MAX });
    items.sort_by(|a, b| {
        let (sa, sb) = (score(a), score(b));
        (sa.0, sa.1, sa.2, sa.3).cmp(&(sb.0, sb.1, sb.2, sb.3)).then(sa.4.total_cmp(&sb.4)).then_with(|| a.id.to_lowercase().cmp(&b.id.to_lowercase()))
    });
    items.first().copied()
}

/// Messages neutres → messages d'OpenAI (images en data URL seulement).
fn to_openai_messages(system: &str, messages: &[Message]) -> Result<Value, AiError> {
    let mut out = Vec::new();
    if !system.is_empty() {
        out.push(json!({ "role": "system", "content": system }));
    }
    for m in messages {
        out.push(match m {
            Message::User { content, images } => {
                if images.is_empty() {
                    json!({ "role": "user", "content": content })
                } else {
                    let mut parts = Vec::new();
                    if !content.is_empty() {
                        parts.push(json!({ "type": "text", "text": content }));
                    }
                    for u in images {
                        let ok = u.len() > 11 && u[..11].eq_ignore_ascii_case("data:image/") && u.contains(";base64,");
                        if !ok {
                            return Err(make_error("invalid_request", "Les images s'envoient en data-URL (data:image/…;base64,…) ou en octets, jamais par une adresse ou un chemin.".into()));
                        }
                        parts.push(json!({ "type": "image_url", "image_url": { "url": u } }));
                    }
                    json!({ "role": "user", "content": parts })
                }
            }
            Message::Assistant { content, tool_calls } => {
                let calls: Vec<&ToolCall> = tool_calls.iter().filter(|c| !c.name.is_empty()).collect();
                let c = if !content.is_empty() {
                    Value::from(content.as_str())
                } else if calls.is_empty() {
                    Value::from("")
                } else {
                    Value::Null
                };
                let mut v = json!({ "role": "assistant", "content": c });
                if !calls.is_empty() {
                    v["tool_calls"] = Value::Array(
                        calls
                            .iter()
                            .enumerate()
                            .map(|(i, c)| {
                                let id = if c.id.is_empty() { format!("call_{}", i + 1) } else { c.id.clone() };
                                let args = if c.args.is_object() { c.args.to_string() } else { "{}".into() };
                                json!({ "id": id, "type": "function", "function": { "name": c.name, "arguments": args } })
                            })
                            .collect(),
                    );
                }
                v
            }
            Message::Tool { tool_call_id, content, .. } => {
                if tool_call_id.is_empty() {
                    return Err(make_error("invalid_request", "Un résultat d'outil doit porter toolCallId.".into()));
                }
                json!({ "role": "tool", "tool_call_id": tool_call_id, "content": content })
            }
        });
    }
    Ok(Value::Array(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preuve_du_contrat() {
        // vecteur d'essai de aiodos-client (contract/proof.json)
        let key = "aiodos_app_k3j7m2qa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let challenge = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";
        assert!(is_app_key(key));
        assert_eq!(key_id_of(key), Some("k3j7m2qa"));
        assert_eq!(compute_proof(key, challenge), "kzujpClJtfCeF66dvjiKZvhVjt_Gf-u5t_oCAAOeIks");
        assert!(proof_matches(key, challenge, Some("kzujpClJtfCeF66dvjiKZvhVjt_Gf-u5t_oCAAOeIks")));
        assert!(!proof_matches(key, challenge, Some("lzujpClJtfCeF66dvjiKZvhVjt_Gf-u5t_oCAAOeIks")));
        assert!(!proof_matches(key, challenge, Some("court")));
        assert!(!is_app_key("aiodos_app_K3J7M2QA_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"));
    }

    #[test]
    fn petits_outils() {
        assert_eq!(params_of("qwen3.5:9b"), "9B");
        assert_eq!(params_of("qwen3.6:35b-a3b"), "35B");
        assert_eq!(params_of("mistral"), "");
        assert_eq!(loading_seconds("aiodos loading 5"), Some(5.0));
        assert_eq!(loading_seconds("aiodos loading"), Some(0.0));
        assert_eq!(loading_seconds("autre"), None);
        assert_eq!(percent_decode("PC%20BUREAU"), "PC BUREAU");
        // même résultat que le client JavaScript
        assert_eq!(redact("Bearer aiodos_app_k3j7m2qa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"), "Bearer ……");
        assert_eq!(redact("clé aiodos_app_k3j7m2qa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"), "clé aiodos_app_k3j7m2qa_…");
        let e = error_from_response(Some(400), &json!({ "error": "authorization_pending", "error_description": "attends" }), None, "fr");
        assert_eq!((e.code(), e.message.as_str()), ("authorization_pending", "attends"));
        let e = error_from_response(Some(503), &Value::Null, Some("3"), "fr");
        assert_eq!((e.code(), e.retryable, e.retry_after), ("engine_unavailable", true, Some(3.0)));
        assert!(local_origin("http://192.168.1.2:11435", "fr").is_err());
        assert_eq!(local_origin("http://localhost:11435/x", "fr").unwrap(), "http://localhost:11435");
    }

    #[cfg(windows)]
    #[test]
    fn registre_lu_par_l_api_de_windows() {
        use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        // REG_SZ : le dossier de Windows, comme la variable d'environnement
        let root = registry_string(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "SystemRoot").expect("SystemRoot");
        assert_eq!(root.to_string_lossy().to_lowercase(), env("SystemRoot").expect("SystemRoot").to_lowercase());
        // REG_EXPAND_SZ : rendue développée (plus de « %USERPROFILE% »)
        if let Some(temp) = registry_string(HKEY_CURRENT_USER, "Environment", "TEMP") {
            assert!(!temp.to_string_lossy().contains('%'), "{temp:?}");
        }
        // clé ou valeur absente : rien
        assert!(registry_string(HKEY_CURRENT_USER, r"Software\ARKLIGHT-essai-cle-absente", "InstallLocation").is_none());
        assert!(registry_string(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "ValeurAbsenteArklight").is_none());
    }
}
