//! Commande vocale de l'assistant par ORAKLE, l'application de dictée locale d'ARKANE : ARKLIGHT
//! lui demande de commencer puis d'arrêter une dictée (API locale d'ORAKLE : 127.0.0.1, port et
//! jeton dans %APPDATA%\orakle\app-api.json) et reçoit le texte. Rien ne sort de l'ordinateur ;
//! ORAKLE garde son micro, sa langue, son dictionnaire et sa traduction.

use crate::tr;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn api_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("orakle").join("app-api.json"))
}

/// Port et jeton de l'ORAKLE lancé.
fn endpoint() -> Option<(u16, String)> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(api_file()?).ok()?).ok()?;
    Some((v["port"].as_u64()? as u16, v["token"].as_str()?.to_owned()))
}

/// Une requête à ORAKLE ; `wait` : délai de réponse (une transcription peut prendre du temps).
fn request(cmd: &str, wait: Duration) -> Result<Value, String> {
    let not_running = || tr("ORAKLE n’est pas lancé.", "ORAKLE is not running.").to_owned();
    let (port, token) = endpoint().ok_or_else(not_running)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|_| not_running())?;
    s.set_read_timeout(Some(wait)).ok();
    let line = json!({ "token": token, "cmd": cmd, "app": "ARKLIGHT" }).to_string() + "\n";
    s.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(&s).read_line(&mut reply).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&reply).map_err(|_| tr("Réponse d’ORAKLE illisible.", "Unreadable answer from ORAKLE.").to_owned())?;
    if v["ok"].as_bool() == Some(true) {
        return Ok(v);
    }
    Err(match v["error"].as_str() {
        Some("busy") => tr("ORAKLE est déjà en train de dicter ou de transcrire.", "ORAKLE is already dictating or transcribing.").to_owned(),
        Some("micro") => tr("ORAKLE n’a pas accès au micro.", "ORAKLE cannot open the microphone.").to_owned(),
        Some("token") => tr("ORAKLE a refusé ARKLIGHT (relance ORAKLE).", "ORAKLE refused ARKLIGHT (restart ORAKLE).").to_owned(),
        _ => v["message"].as_str().unwrap_or("ORAKLE").to_owned(),
    })
}

/// ORAKLE installé (installeur : Program Files ; ou pour l'utilisateur).
fn installed() -> Option<PathBuf> {
    let mut list = Vec::new();
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(d) = std::env::var_os(var) {
            list.push(PathBuf::from(d).join("ORAKLE").join("ORAKLE.exe"));
        }
    }
    if let Some(d) = std::env::var_os("LOCALAPPDATA") {
        list.push(PathBuf::from(d).join("Programs").join("ORAKLE").join("ORAKLE.exe"));
    }
    list.into_iter().find(|p| p.is_file())
}

/// ORAKLE prêt à dicter : lancé au besoin (jusqu'à 20 s d'attente). Appeler hors du fil de l'interface.
pub fn ensure_running() -> Result<(), String> {
    if request("status", Duration::from_secs(3)).is_ok() {
        return Ok(());
    }
    let exe = installed().ok_or_else(|| tr("ORAKLE n’est pas installé : installe-le pour parler à l’assistant (dictée locale d’ARKANE).", "ORAKLE is not installed: install it to talk to the assistant (ARKANE’s local dictation).").to_owned())?;
    std::process::Command::new(&exe).spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(400));
        if request("status", Duration::from_secs(3)).is_ok() {
            return Ok(());
        }
    }
    Err(tr(
        "ORAKLE est lancé mais ne répond pas : mets-le à jour (version avec l’API pour les applications) ou vérifie « app_api » dans ses réglages.",
        "ORAKLE is running but does not answer: update it (version with the app API) or check “app_api” in its settings.",
    )
    .to_owned())
}

/// Commence la dictée.
pub fn start() -> Result<(), String> {
    ensure_running()?;
    request("start", Duration::from_secs(5)).map(|_| ())
}

/// Arrête la dictée et rend le texte transcrit (vide : rien entendu).
pub fn stop() -> Result<String, String> {
    let v = request("stop", Duration::from_secs(180))?;
    Ok(v["text"].as_str().unwrap_or("").trim().to_owned())
}

/// Jette la dictée en cours.
pub fn cancel() {
    let _ = request("cancel", Duration::from_secs(3));
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// Faux ORAKLE : vérifie le jeton, rend un texte à « stop ».
    #[test]
    fn dictee_par_orakle() {
        let dir = std::env::temp_dir().join(format!("arklight-voice-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("orakle")).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::fs::write(dir.join("orakle").join("app-api.json"), format!(r#"{{"port":{port},"token":"t0k"}}"#)).unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                let reply = match (v["token"].as_str(), v["cmd"].as_str()) {
                    (Some("t0k"), Some("status")) => r#"{"ok":true,"version":"0.1.4","state":"idle"}"#,
                    (Some("t0k"), Some("start")) => r#"{"ok":true}"#,
                    (Some("t0k"), Some("stop")) => r#"{"ok":true,"text":" ajoute deux Titan en contre ","lang":"fr"}"#,
                    _ => r#"{"ok":false,"error":"token"}"#,
                };
                (&stream).write_all(format!("{reply}
").as_bytes()).unwrap();
            }
        });
        // SAFETY : seul essai de ce programme qui lit APPDATA
        unsafe { std::env::set_var("APPDATA", &dir) };
        super::start().unwrap();
        assert_eq!(super::stop().unwrap(), "ajoute deux Titan en contre");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
