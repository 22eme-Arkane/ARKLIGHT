//! Ollama (IA locale et gratuite) : état, démarrage, installation et téléchargement des modèles,
//! pour les utilisateurs qui ne l'ont pas encore (portage de `server/ai/ollama.js`). Tout est
//! déclenché par l'utilisateur depuis l'assistant (bouton) : rien n'est téléchargé tout seul.

use crate::error::AiError;
use crate::http::{Cancel, Request};
use crate::providers::{ModelInfo, ollama_model};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const OLLAMA_SETUP_URL: &str = "https://ollama.com/download/OllamaSetup.exe";

/// Un modèle proposé au téléchargement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SuggestedModel {
    pub id: &'static str,
    /// taille approximative (Go)
    pub size: f64,
    pub note: &'static str,
}

/// Modèles proposés au téléchargement.
pub const SUGGESTED_MODELS: [SuggestedModel; 4] = [
    SuggestedModel { id: "qwen3.5:9b", size: 6.6, note: "Conseillé : comprend les outils, rapide avec une carte graphique de 8 Go" },
    SuggestedModel { id: "qwen3:8b", size: 5.2, note: "Comprend les outils" },
    SuggestedModel { id: "qwen3:4b", size: 2.6, note: "Petit modèle pour un ordinateur modeste" },
    SuggestedModel { id: "qwen2.5vl:7b", size: 6.0, note: "Voit les images (photo d’un plan de feu dessiné)" },
];

/// Programme Ollama installé (Windows : dossier de l'utilisateur), ou None.
pub fn ollama_exe() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty())?;
    let dir = Path::new(&local).join("Programs").join("Ollama");
    ["ollama app.exe", "ollama.exe"].iter().map(|n| dir.join(n)).find(|p| p.exists())
}

/// État d'Ollama : installé ? en marche ? version, modèles (avec leurs capacités).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OllamaStatus {
    pub installed: bool,
    pub running: bool,
    pub version: String,
    pub models: Vec<ModelInfo>,
}

impl OllamaStatus {
    pub fn to_json(&self) -> Value {
        json!({ "installed": self.installed, "running": self.running, "version": self.version, "models": crate::providers::models_json(&self.models) })
    }
}

fn get_json(url: &str, timeout: Duration) -> Option<Value> {
    let res = Request::get(url).timeout(timeout).send(None).ok()?;
    if !res.ok() {
        return None;
    }
    let (_, v) = res.text_and_json().ok()?;
    Some(v)
}

/// État d'Ollama à cette adresse.
pub fn ollama_status(url: &str) -> OllamaStatus {
    let mut st = OllamaStatus { installed: ollama_exe().is_some(), ..OllamaStatus::default() };
    let Some(v) = get_json(&format!("{url}/api/version"), Duration::from_secs(4)) else { return st }; // pas lancé
    st.version = v.get("version").and_then(Value::as_str).unwrap_or("").to_owned();
    st.running = true;
    st.installed = true;
    if let Some(tags) = get_json(&format!("{url}/api/tags"), Duration::from_secs(8)) {
        st.models = tags.get("models").and_then(Value::as_array).into_iter().flatten().map(ollama_model).collect();
    }
    st
}

/// Familles préférées, dans l'ordre (sans les motifs « (?!…) » de JavaScript, écrits à la main).
fn family(i: usize, id: &str) -> bool {
    // « pat » trouvé et suivi d'autre chose que l'un des `not`
    let followed = |pat: &str, not: &[&str]| id.match_indices(pat).any(|(p, _)| !not.iter().any(|n| id[p + pat.len()..].starts_with(n)));
    match i {
        0 => id.contains("qwen3.5"),
        1 => followed("qwen3", &["."]),
        2 => id.contains("qwen3.6"),
        3 => id.contains("gemma4"),
        4 => ["llama3.1", "llama3.2", "llama3.3"].iter().any(|p| id.contains(p)),
        5 => id.contains("mistral-small"),
        6 => followed("qwen2.5", &["-coder", "vl"]),
        7 => id.contains("mistral"),
        _ => false,
    }
}

/// Meilleur modèle installé pour l'assistant (qui comprend les outils) : les familles récentes
/// d'abord (les petits modèles anciens se trompent souvent dans les placements), le plus léger de
/// la famille. Les modèles qui tiennent dans une carte graphique de portable (8 Go) d'abord :
/// qwen3.6 (36B) est meilleur mais met plusieurs minutes à se charger sur ce genre de machine.
pub fn pick_model(models: &[ModelInfo]) -> String {
    let with_tools: Vec<&ModelInfo> = models.iter().filter(|m| m.caps.is_empty() || m.has("tools")).collect();
    for i in 0..8 {
        let mut fam: Vec<&&ModelInfo> = with_tools.iter().filter(|m| family(i, &m.id)).collect();
        fam.sort_by(|a, b| a.size.total_cmp(&b.size));
        if let Some(m) = fam.first() {
            return m.id.clone();
        }
    }
    with_tools.first().map(|m| m.id.clone()).unwrap_or_default()
}

/// Lance Ollama (installé mais arrêté).
pub fn start_ollama() -> Result<(), AiError> {
    let exe = ollama_exe().ok_or_else(|| AiError::new("Ollama n’est pas installé"))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0000_0008 | 0x0000_0200 | 0x0800_0000);
    }
    cmd.spawn().map(|_| ()).map_err(|e| AiError::new(format!("Ollama : {e}")))
}

/// Progression d'un téléchargement.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// pourcentage (None : inconnu)
    pub pct: Option<f64>,
    pub text: String,
}

/// « 1,2 Go »
fn gb(n: f64) -> String {
    let s = if n < 1e9 { format!("{:.2}", n / 1e9) } else { format!("{:.1}", n / 1e9) };
    format!("{} Go", s.replace('.', ","))
}

/// Télécharge un modèle (POST /api/pull, progression en continu).
pub fn pull_model(url: &str, model: &str, on_progress: &mut dyn FnMut(Progress), cancel: Option<&Cancel>) -> Result<(), AiError> {
    let req = Request::post_json(&format!("{url}/api/pull"), &json!({ "model": model, "stream": true })).timeout(crate::providers::LOAD_TIMEOUT);
    let mut res = req.send(cancel).map_err(AiError::from_provider)?;
    if !res.ok() {
        let status = res.status;
        let text = res.text().unwrap_or_default();
        return Err(AiError::new(format!("Ollama : {status} {text}").trim().to_owned()));
    }
    let mut last: Option<Instant> = None;
    while let Some(line) = res.next_line().map_err(AiError::from_provider)? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(j) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(e) = j.get("error").filter(|e| crate::util::truthy(Some(e))) {
            return Err(AiError::new(crate::util::js_string(e)));
        }
        let total = j.get("total").and_then(Value::as_f64).filter(|t| *t > 0.0);
        let done = j.get("completed").and_then(Value::as_f64).unwrap_or(0.0);
        let status = j.get("status").and_then(Value::as_str).unwrap_or("");
        let success = status == "success";
        if success || last.is_none_or(|t| t.elapsed() > Duration::from_millis(400)) {
            last = Some(Instant::now());
            let pct = if success { Some(100.0) } else { total.map(|t| arklight_core::engine::js_round(100.0 * done / t)) };
            let text = match total {
                Some(t) => format!("{model} — {} / {}", gb(done), gb(t)),
                None => format!("{model} — {status}"),
            };
            on_progress(Progress { pct, text });
        }
    }
    Ok(())
}

/// Télécharge l'installeur officiel d'Ollama dans `dir` puis le lance (l'utilisateur suit son
/// assistant) ; rend le chemin de l'installeur.
pub fn install_ollama(dir: &Path, on_progress: &mut dyn FnMut(Progress), cancel: Option<&Cancel>) -> Result<PathBuf, AiError> {
    std::fs::create_dir_all(dir).map_err(|e| AiError::new(e.to_string()))?;
    let dest = dir.join("OllamaSetup.exe");
    let res = Request::get(OLLAMA_SETUP_URL).follow_redirects().timeout(Duration::from_secs(60)).send(cancel).map_err(AiError::from_provider)?;
    if !res.ok() {
        return Err(AiError::new(format!("Téléchargement impossible ({})", res.status)));
    }
    let total = res.header("content-length").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let mut file = std::fs::File::create(&dest).map_err(|e| AiError::new(e.to_string()))?;
    let mut last = Instant::now();
    res.copy_to(&mut file, |done| {
        if last.elapsed() > Duration::from_millis(400) {
            last = Instant::now();
            let done = done as f64;
            let pct = if total > 0.0 { Some(arklight_core::engine::js_round(100.0 * done / total)) } else { None };
            let text = if total > 0.0 { format!("OllamaSetup.exe — {} / {}", gb(done), gb(total)) } else { format!("OllamaSetup.exe — {}", gb(done)) };
            on_progress(Progress { pct, text });
        }
    })
    .map_err(AiError::from_provider)?;
    drop(file);
    on_progress(Progress { pct: Some(100.0), text: "Installeur d’Ollama lancé : suis ses étapes, puis reviens ici.".into() });
    std::process::Command::new(&dest).spawn().map_err(|e| AiError::new(format!("Installeur d’Ollama : {e}")))?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, size: f64, caps: &[&str]) -> ModelInfo {
        ModelInfo { size, caps: caps.iter().map(|c| (*c).to_owned()).collect(), ..ModelInfo::new(id) }
    }

    #[test]
    fn choix_du_modele() {
        let list = [m("llama3.2:3b", 2.0, &["completion", "tools"]), m("qwen3:14b", 9.0, &["completion", "tools"]), m("qwen3:8b", 5.0, &["completion", "tools"]), m("qwen3.6:35b", 20.0, &["completion", "tools"])];
        assert_eq!(pick_model(&list), "qwen3:8b");
        let list = [m("qwen2.5-coder:7b", 4.0, &["completion", "tools"]), m("qwen2.5:7b", 4.5, &["completion", "tools"]), m("phi", 1.0, &["completion"])];
        assert_eq!(pick_model(&list), "qwen2.5:7b");
        let list = [m("phi", 1.0, &["completion"]), m("inconnu", 1.0, &[])];
        assert_eq!(pick_model(&list), "inconnu");
        assert_eq!(gb(2_600_000_000.0), "2,6 Go");
        assert_eq!(gb(500_000_000.0), "0,50 Go");
    }
}
