//! Une seule fenêtre d'ARKLIGHT (comme `requestSingleInstanceLock` de la version Electron) : un
//! second lancement passe son projet à la fenêtre déjà ouverte, puis s'arrête. Échange local
//! (127.0.0.1), une ligne « ARKLIGHT-OPEN<tab>chemin », réponse « ARKLIGHT-OK ».

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

const PORT: u16 = 47_331;
const HELLO: &str = "ARKLIGHT-OPEN";
const OK: &str = "ARKLIGHT-OK";

pub enum Claim {
    /// première fenêtre : elle écoute les lancements suivants
    Primary(TcpListener),
    /// une fenêtre d'ARKLIGHT était déjà ouverte : elle a reçu le projet
    Forwarded,
    /// port pris par un autre programme : fenêtre indépendante
    Alone,
}

/// Au lancement : devient la fenêtre d'ARKLIGHT, ou passe `project` à celle déjà ouverte.
pub fn claim(project: Option<&Path>) -> Claim {
    claim_at(PORT, project)
}

fn claim_at(port: u16, project: Option<&Path>) -> Claim {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match TcpListener::bind(addr) {
        Ok(l) => Claim::Primary(l),
        Err(_) => {
            let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else { return Claim::Alone };
            let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
            let path = project.map(|p| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()).display().to_string()).unwrap_or_default();
            if s.write_all(format!("{HELLO}\t{path}\n").as_bytes()).is_err() {
                return Claim::Alone;
            }
            let mut reply = String::new();
            let _ = BufReader::new(&s).read_line(&mut reply);
            if reply.trim() == OK { Claim::Forwarded } else { Claim::Alone }
        }
    }
}

/// Fenêtre d'ARKLIGHT : `on_open` reçoit le projet de chaque lancement suivant (vide : seulement
/// montrer la fenêtre). Appelé depuis un fil à part.
pub fn listen(listener: TcpListener, on_open: impl Fn(String) + Send + 'static) {
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                continue;
            }
            let Some(rest) = line.trim_end_matches(['\r', '\n']).strip_prefix(HELLO) else { continue };
            let _ = (&stream).write_all(format!("{OK}\n").as_bytes());
            on_open(rest.trim_start_matches('\t').to_owned());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_lancement_passe_son_projet() {
        // port libre pris au hasard (pas celui d'une fenêtre d'ARKLIGHT ouverte)
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let Claim::Primary(l) = claim_at(port, None) else { panic!("première fenêtre attendue") };
        let (tx, rx) = std::sync::mpsc::channel();
        listen(l, move |p| tx.send(p).unwrap());
        assert!(matches!(claim_at(port, Some(Path::new(r"C:\projets\essai.json"))), Claim::Forwarded));
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), r"C:\projets\essai.json");
        assert!(matches!(claim_at(port, None), Claim::Forwarded));
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), "");
    }

    #[test]
    fn port_pris_par_un_autre_programme() {
        // un serveur qui ne répond pas comme ARKLIGHT : fenêtre indépendante
        let other = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = other.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in other.incoming().flatten() {
                let _ = (&s).write_all(b"HELLO\n");
            }
        });
        assert!(matches!(claim_at(port, None), Claim::Alone));
    }
}
