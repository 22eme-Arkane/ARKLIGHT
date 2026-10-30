//! Vérification des mises à jour (portage de `server/update-check.js`) : la dernière version
//! publiée dans les Releases GitHub d'ARKLIGHT ; les pré-versions (« 1.1.0-beta.1 ») ne sont
//! proposées qu'à une pré-version.

use std::cmp::Ordering;
use std::time::Duration;

pub const UPDATE_REPO: &str = "22eme-Arkane/ARKLIGHT";

/// Compare deux versions semver (« 0.2.0 », « v0.1.0-alpha.3 »).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn parse(v: &str) -> (Vec<u64>, Vec<String>) {
        let v = v.trim().trim_start_matches(['v', 'V']);
        let v = v.split('+').next().unwrap_or("");
        let (core, pre) = v.split_once('-').unwrap_or((v, ""));
        (core.split('.').map(|n| n.parse().unwrap_or(0)).collect(), if pre.is_empty() { Vec::new() } else { pre.split('.').map(str::to_owned).collect() })
    }
    let (a, b) = (parse(a), parse(b));
    for i in 0..3 {
        let (x, y) = (a.0.get(i).copied().unwrap_or(0), b.0.get(i).copied().unwrap_or(0));
        if x != y {
            return x.cmp(&y);
        }
    }
    // une version finale passe avant ses pré-versions (1.0.0 > 1.0.0-alpha.9)
    if a.1.is_empty() || b.1.is_empty() {
        return b.1.len().min(1).cmp(&a.1.len().min(1));
    }
    for i in 0..a.1.len().max(b.1.len()) {
        let (Some(x), Some(y)) = (a.1.get(i), b.1.get(i)) else { return a.1.len().cmp(&b.1.len()) };
        let (nx, ny) = (x.parse::<u64>(), y.parse::<u64>());
        match (nx, ny) {
            (Ok(p), Ok(q)) if p != q => return p.cmp(&q),
            (Ok(_), Err(_)) => return Ordering::Less,
            (Err(_), Ok(_)) => return Ordering::Greater,
            _ if x != y => return x.cmp(y),
            _ => {}
        }
    }
    Ordering::Equal
}

/// Résultat d'une vérification (fenêtre « À propos »).
pub enum Status {
    /// nouvelle version : (version, page de la version, installeur à télécharger)
    Available(String, String, String),
    UpToDate,
    /// aucune version publiée
    NoneYet,
    Failed(String),
}

/// Vérification complète : dernière version publiée comparée à `current`.
pub fn status(current: &str) -> Status {
    let url = format!("https://api.github.com/repos/{UPDATE_REPO}/releases?per_page=15");
    let req = arklight_ai::http::Request::get(&url).header("Accept", "application/vnd.github+json").header("User-Agent", "ARKLIGHT").timeout(Duration::from_secs(10));
    let res = match req.send(None) {
        Ok(r) => r,
        Err(e) => return Status::Failed(e.to_string()),
    };
    if res.status == 404 {
        return Status::NoneYet;
    }
    if res.status != 200 {
        return Status::Failed(format!("HTTP {}", res.status));
    }
    let Ok((_, json)) = res.text_and_json() else { return Status::Failed("JSON".into()) };
    let mut list: Vec<(String, String, String)> = json
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        // version finale : seulement les versions finales
        .filter(|r| current.contains('-') || !(r["prerelease"].as_bool().unwrap_or(false) || r["tag_name"].as_str().is_some_and(|t| t.contains('-'))))
        .filter_map(|r| {
            let exe = r["assets"].as_array().into_iter().flatten().find(|a| a["name"].as_str().is_some_and(|n| n.to_ascii_lowercase().ends_with(".exe"))).and_then(|a| a["browser_download_url"].as_str()).unwrap_or("");
            Some((r["tag_name"].as_str()?.trim_start_matches(['v', 'V']).to_owned(), r["html_url"].as_str().unwrap_or("").to_owned(), exe.to_owned()))
        })
        .collect();
    list.sort_by(|x, y| compare_versions(&y.0, &x.0));
    match list.into_iter().next() {
        None => Status::NoneYet,
        Some((v, page, exe)) if compare_versions(&v, current) == Ordering::Greater => Status::Available(v, page, exe),
        Some(_) => Status::UpToDate,
    }
}

/// Télécharge l'installeur de la nouvelle version dans `dest` (fichier provisoire, renommé à la
/// fin) ; `progress(octets reçus, taille totale)`. Erreur vide : téléchargement annulé.
pub fn download(url: &str, dest: &std::path::Path, cancel: &arklight_ai::Cancel, mut progress: impl FnMut(u64, Option<u64>)) -> Result<std::path::PathBuf, String> {
    if !url.starts_with("https://") {
        return Err("adresse invalide".into());
    }
    let res = arklight_ai::http::Request::get(url).header("User-Agent", "ARKLIGHT").header("Accept", "application/octet-stream").follow_redirects().timeout(Duration::from_secs(600)).send(Some(cancel)).map_err(|e| if cancel.is_cancelled() { String::new() } else { e.to_string() })?;
    if res.status != 200 {
        return Err(format!("HTTP {}", res.status));
    }
    let total = res.header("content-length").and_then(|v| v.trim().parse::<u64>().ok());
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = dest.with_extension("part");
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let r = res.copy_to(&mut file, |done| progress(done, total));
    drop(file);
    match r {
        Ok(n) if total.is_none_or(|t| t == n) => {
            std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
            Ok(dest.to_path_buf())
        }
        Ok(_) => {
            let _ = std::fs::remove_file(&part);
            Err("téléchargement incomplet".into())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(if cancel.is_cancelled() { String::new() } else { e.to_string() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::compare_versions;
    use std::cmp::Ordering::*;

    #[test]
    fn versions() {
        assert_eq!(compare_versions("0.2.0-alpha.1", "0.1.0-alpha.5"), Greater);
        assert_eq!(compare_versions("v0.1.0-alpha.10", "0.1.0-alpha.9"), Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.0-alpha.9"), Greater);
        assert_eq!(compare_versions("1.0.0-alpha", "1.0.0-alpha.1"), Less);
        assert_eq!(compare_versions("1.0.0-beta", "1.0.0-alpha.3"), Greater);
        assert_eq!(compare_versions("0.2.0", "v0.2.0"), Equal);
    }
}
