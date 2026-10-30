//! Séquences et plans d'un projet (portage de la partie « plans » de `server/scene-store.js`).
//!
//! Dans le fichier, le plan ouvert est au premier niveau (projecteurs, décor, ambiance, plan 2D) ;
//! les autres plans gardent leur scène dans `sequences[].plans[].scene` (sans le nom du projet), et
//! `current` dit lequel est ouvert. Changer de plan échange les deux ; le nom du projet et le
//! cartouche (production, chef électro) suivent : ils appartiennent au projet, pas au plan.

use crate::document::Document;
use crate::library::library;
use crate::project::{Environment, Project};
use serde_json::{Map, Value, json};

/// Champs du fichier qui ne font pas partie de la scène d'un plan.
const FILE_KEYS: [&str; 7] = ["app", "format", "savedAt", "current", "sequences", "_session", "thumbnail"];

/// Un plan, pour les interfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanInfo {
    pub id: String,
    pub name: String,
    pub fixtures: usize,
    pub objects: usize,
    pub current: bool,
    /// vignette (image en data URL), prise quand on quitte le plan ou qu'on enregistre
    pub thumbnail: Option<String>,
}

/// Une séquence et ses plans, pour les interfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceInfo {
    pub id: String,
    pub name: String,
    pub plans: Vec<PlanInfo>,
}

/// Comment commence un nouveau plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// copie du plan ouvert
    Copy,
    /// même décor, sans projecteurs ni grip
    Decor,
    Empty,
}

fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    format!("{prefix}{:x}{:x}", t % 0xffff_ffff, N.fetch_add(1, Ordering::Relaxed))
}

fn clean_name(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(60).collect()
}

fn plan_name(n: usize) -> String {
    format!("Plan {n}")
}

fn sequence_name(n: usize) -> String {
    format!("Séquence {n}")
}

/// Premier numéro libre après ceux de noms « Plan 3 », « Séquence 2 »…
fn next_number(names: &[String], count: usize) -> usize {
    names.iter().filter_map(|n| n.rsplit(' ').next()?.parse::<usize>().ok()).max().unwrap_or(0).max(count) + 1
}

/// Scène du plan ouvert (le projet sans ses champs de fichier ni son nom).
fn current_scene(p: &Project) -> Value {
    let mut v = serde_json::to_value(p).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        for k in FILE_KEYS {
            m.remove(k);
        }
        m.remove("name");
    }
    v
}

/// Scène vide d'un nouveau plan (plateau par défaut).
fn empty_scene() -> Value {
    let mut v = current_scene(&Project { environment: Environment::default(), ..Project::default() });
    if let Value::Object(m) = &mut v {
        m.insert("objects".into(), json!([]));
    }
    v
}

impl Document {
    fn sequences(&self) -> Vec<Value> {
        self.project.extra.get("sequences").and_then(Value::as_array).cloned().unwrap_or_default()
    }

    fn current_ids(&self) -> (String, String) {
        let c = self.project.extra.get("current");
        let s = |k: &str| c.and_then(|c| c.get(k)).and_then(Value::as_str).unwrap_or("").to_owned();
        (s("seq"), s("plan"))
    }

    fn store(&mut self, seqs: Vec<Value>, seq: &str, plan: &str) {
        self.project.extra.insert("sequences".into(), Value::Array(seqs));
        self.project.extra.insert("current".into(), json!({ "seq": seq, "plan": plan }));
    }

    /// Met en ordre les séquences lues dans un fichier : le plan ouvert est celui sans scène ; un
    /// projet d'avant les séquences devient « Séquence 1 › Plan 1 ».
    pub fn normalize_plans(&mut self) {
        let mut seqs: Vec<Value> = self.sequences().into_iter().filter(|s| s.get("plans").and_then(Value::as_array).is_some_and(|p| !p.is_empty())).collect();
        let (_, wanted) = self.current_ids();
        let mut cur: Option<(usize, usize)> = None;
        let open = |p: &Value| p.get("scene").is_none_or(Value::is_null);
        for (i, s) in seqs.iter().enumerate() {
            for (j, p) in s["plans"].as_array().into_iter().flatten().enumerate() {
                if cur.is_none() && open(p) && p.get("id").and_then(Value::as_str) == Some(wanted.as_str()) {
                    cur = Some((i, j));
                }
            }
        }
        for (i, s) in seqs.iter().enumerate() {
            for (j, p) in s["plans"].as_array().into_iter().flatten().enumerate() {
                if cur.is_none() && open(p) {
                    cur = Some((i, j));
                }
            }
        }
        let (ci, cj) = match cur {
            Some(c) => c,
            None => {
                if seqs.is_empty() {
                    seqs.push(json!({ "id": new_id("s"), "name": sequence_name(1), "plans": [] }));
                }
                let n = seqs[0]["plans"].as_array().map_or(0, Vec::len);
                if let Some(plans) = seqs[0]["plans"].as_array_mut() {
                    plans.insert(0, json!({ "id": new_id("p"), "name": plan_name(n + 1), "scene": null }));
                }
                (0, 0)
            }
        };
        // fichier abîmé : un autre plan sans scène repart vide
        for (i, s) in seqs.iter_mut().enumerate() {
            for (j, p) in s["plans"].as_array_mut().into_iter().flatten().enumerate() {
                if (i, j) != (ci, cj) && p.get("scene").is_none_or(Value::is_null) {
                    p["scene"] = empty_scene();
                }
            }
        }
        let seq = seqs[ci]["id"].as_str().unwrap_or("").to_owned();
        let plan = seqs[ci]["plans"][cj]["id"].as_str().unwrap_or("").to_owned();
        self.store(seqs, &seq, &plan);
    }

    /// Séquences et plans (le plan ouvert compte la scène actuelle).
    pub fn plan_info(&self) -> Vec<SequenceInfo> {
        let (_, cur) = self.current_ids();
        self.sequences()
            .iter()
            .map(|s| SequenceInfo {
                id: s["id"].as_str().unwrap_or("").into(),
                name: s["name"].as_str().unwrap_or("").into(),
                plans: s["plans"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| {
                        let id = p["id"].as_str().unwrap_or("").to_owned();
                        let current = id == cur;
                        let count = |k: &str| if current { if k == "fixtures" { self.project.fixtures.len() } else { self.project.objects.len() } } else { p["scene"][k].as_array().map_or(0, Vec::len) };
                        let thumbnail = p["thumbnail"].as_str().filter(|t| t.starts_with("data:image/")).map(str::to_owned);
                        PlanInfo { name: p["name"].as_str().unwrap_or("").into(), fixtures: count("fixtures"), objects: count("objects"), current, thumbnail, id }
                    })
                    .collect(),
            })
            .collect()
    }

    fn find(seqs: &[Value], plan: &str) -> Option<(usize, usize)> {
        seqs.iter().enumerate().find_map(|(i, s)| s["plans"].as_array()?.iter().position(|p| p["id"].as_str() == Some(plan)).map(|j| (i, j)))
    }

    /// Ouvre un autre plan (changer de plan n'est pas une modification à enregistrer).
    pub fn switch_plan(&mut self, id: &str) -> bool {
        let mut seqs = self.sequences();
        let (_, cur) = self.current_ids();
        let (Some((fi, fj)), Some((ti, tj))) = (Self::find(&seqs, &cur), Self::find(&seqs, id)) else { return false };
        if (fi, fj) == (ti, tj) {
            return false;
        }
        seqs[fi]["plans"][fj]["scene"] = current_scene(&self.project);
        let mut scene = seqs[ti]["plans"][tj]["scene"].take();
        if !scene.is_object() {
            scene = empty_scene();
        }
        seqs[ti]["plans"][tj]["scene"] = Value::Null;
        // le nom du projet et le cartouche suivent
        let name = self.project.name.clone();
        let title = self.project.extra.get("plot").and_then(|p| p.get("title")).cloned();
        let keep: Map<String, Value> = self.project.extra.iter().filter(|(k, _)| FILE_KEYS.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
        let Ok(mut next) = serde_json::from_value::<Project>(scene) else { return false };
        next.name = name;
        next.extra.extend(keep);
        if let Some(t) = title {
            let plot = next.extra.entry("plot").or_insert_with(|| json!({}));
            if let Value::Object(pm) = plot {
                let entry = pm.entry("title").or_insert_with(|| json!({}));
                for k in ["production", "author"] {
                    entry[k] = t.get(k).cloned().unwrap_or(json!(""));
                }
            }
        }
        next.sanitize();
        self.project = next;
        let seq = seqs[ti]["id"].as_str().unwrap_or("").to_owned();
        self.store(seqs, &seq, id);
        self.touch_plans();
        true
    }

    /// Les plans ont changé : nouvelle version, et l'historique repart (il porte sur un seul plan).
    fn touch_plans(&mut self) {
        self.revision += 1;
        self.clear_history();
    }

    fn start_scene(&self, start: Start) -> Value {
        match start {
            Start::Copy => current_scene(&self.project),
            Start::Empty => empty_scene(),
            Start::Decor => {
                let mut s = empty_scene();
                let lib = library();
                let decor: Vec<Value> = self.project.objects.iter().filter(|o| lib.prop(&o.type_id).is_some()).filter_map(|o| serde_json::to_value(o).ok()).collect();
                s["objects"] = Value::Array(decor);
                s["environment"] = serde_json::to_value(&self.project.environment).unwrap_or(Value::Null);
                s
            }
        }
    }

    /// Nouveau plan juste après le plan ouvert ; il devient le plan ouvert. Renvoie son id.
    pub fn new_plan(&mut self, start: Start) -> Option<String> {
        self.new_plan_in(None, start)
    }

    /// Nouveau plan dans une séquence (`newPlan({ seq })`) : juste après le plan ouvert s'il est
    /// dans cette séquence, sinon à la fin ; il devient le plan ouvert. Renvoie son id.
    pub fn new_plan_in(&mut self, seq: Option<&str>, start: Start) -> Option<String> {
        let mut seqs = self.sequences();
        let (_, cur) = self.current_ids();
        let (ci, cj) = Self::find(&seqs, &cur)?;
        let i = seq.and_then(|s| seqs.iter().position(|x| x["id"].as_str() == Some(s))).unwrap_or(ci);
        let at = if i == ci { cj + 1 } else { seqs[i]["plans"].as_array().map_or(0, Vec::len) };
        let names: Vec<String> = seqs[i]["plans"].as_array()?.iter().filter_map(|p| p["name"].as_str().map(str::to_owned)).collect();
        let id = new_id("p");
        let plan = json!({ "id": id, "name": plan_name(next_number(&names, names.len())), "scene": self.start_scene(start) });
        seqs[i]["plans"].as_array_mut()?.insert(at, plan);
        self.project.extra.insert("sequences".into(), Value::Array(seqs));
        self.switch_plan(&id);
        Some(id)
    }

    /// Plan ouvert (son id).
    pub fn current_plan_id(&self) -> String {
        self.current_ids().1
    }

    /// Vignette d'un plan (pas une modification à enregistrer à elle seule).
    pub fn set_plan_thumbnail(&mut self, id: &str, thumbnail: &str) {
        let mut seqs = self.sequences();
        let Some((i, j)) = Self::find(&seqs, id) else { return };
        seqs[i]["plans"][j]["thumbnail"] = json!(thumbnail);
        self.project.extra.insert("sequences".into(), Value::Array(seqs));
    }

    /// Copie d'un plan, juste après lui (on reste sur le plan ouvert).
    pub fn duplicate_plan(&mut self, id: &str) -> bool {
        let mut seqs = self.sequences();
        let Some((i, j)) = Self::find(&seqs, id) else { return false };
        let p = &seqs[i]["plans"][j];
        let scene = if p["scene"].is_object() { p["scene"].clone() } else { current_scene(&self.project) };
        let mut copy = json!({ "id": new_id("p"), "name": clean_name(&format!("{} (copie)", p["name"].as_str().unwrap_or(""))), "scene": scene });
        if let Some(t) = p.get("thumbnail").filter(|t| t.is_string()) {
            copy["thumbnail"] = t.clone();
        }
        let Some(plans) = seqs[i]["plans"].as_array_mut() else { return false };
        plans.insert(j + 1, copy);
        self.project.extra.insert("sequences".into(), Value::Array(seqs));
        self.revision += 1;
        true
    }

    /// Supprime une séquence et ses plans (pas la dernière du projet).
    pub fn remove_sequence(&mut self, id: &str) -> bool {
        let seqs = self.sequences();
        let Some(k) = seqs.iter().position(|s| s["id"].as_str() == Some(id)) else { return false };
        if seqs.len() <= 1 {
            return false;
        }
        let (_, cur) = self.current_ids();
        if seqs[k]["plans"].as_array().into_iter().flatten().any(|p| p["id"].as_str() == Some(cur.as_str())) {
            let other = seqs.get(k + 1).or_else(|| k.checked_sub(1).and_then(|i| seqs.get(i)));
            let first = other.and_then(|s| s["plans"][0]["id"].as_str()).unwrap_or("").to_owned();
            self.switch_plan(&first);
        }
        let mut seqs = self.sequences();
        seqs.retain(|s| s["id"].as_str() != Some(id));
        self.project.extra.insert("sequences".into(), Value::Array(seqs));
        self.touch_plans();
        true
    }

    /// Déplace un plan : à une autre place de sa séquence (`index` compté sans lui), ou dans une
    /// autre séquence (à la fin si `index` est None).
    pub fn move_plan(&mut self, id: &str, seq: Option<&str>, index: Option<usize>) -> bool {
        let mut seqs = self.sequences();
        let Some((i, j)) = Self::find(&seqs, id) else { return false };
        let target_id = seq.map(str::to_owned).or_else(|| seqs[i]["id"].as_str().map(str::to_owned)).unwrap_or_default();
        if !seqs.iter().any(|s| s["id"].as_str() == Some(target_id.as_str())) {
            return false;
        }
        let Some(plan) = seqs[i]["plans"].as_array_mut().map(|p| p.remove(j)) else { return false };
        let Some(t) = seqs.iter().position(|s| s["id"].as_str() == Some(target_id.as_str())) else { return false };
        let Some(plans) = seqs[t]["plans"].as_array_mut() else { return false };
        let at = index.unwrap_or(plans.len()).min(plans.len());
        plans.insert(at, plan);
        seqs.retain(|s| s["plans"].as_array().is_some_and(|p| !p.is_empty()));
        let (cur_seq, cur) = self.current_ids();
        let seq_now = if cur == id { target_id } else { cur_seq };
        self.store(seqs, &seq_now, &cur);
        self.revision += 1;
        true
    }

    /// Déplace une séquence à une autre place.
    pub fn move_sequence(&mut self, id: &str, index: usize) -> bool {
        let mut seqs = self.sequences();
        let Some(from) = seqs.iter().position(|s| s["id"].as_str() == Some(id)) else { return false };
        let to = index.min(seqs.len().saturating_sub(1));
        if from == to {
            return false;
        }
        let s = seqs.remove(from);
        seqs.insert(to, s);
        self.project.extra.insert("sequences".into(), Value::Array(seqs));
        self.revision += 1;
        true
    }

    /// Nouvelle séquence (un premier plan) après la séquence ouverte ; elle devient ouverte.
    pub fn new_sequence(&mut self, start: Start) -> Option<String> {
        let mut seqs = self.sequences();
        let (_, cur) = self.current_ids();
        let (i, _) = Self::find(&seqs, &cur)?;
        let names: Vec<String> = seqs.iter().filter_map(|s| s["name"].as_str().map(str::to_owned)).collect();
        let plan_id = new_id("p");
        let seq = json!({ "id": new_id("s"), "name": sequence_name(next_number(&names, names.len())), "plans": [{ "id": plan_id, "name": plan_name(1), "scene": self.start_scene(start) }] });
        let seq_id = seq["id"].as_str().unwrap_or("").to_owned();
        seqs.insert(i + 1, seq);
        self.project.extra.insert("sequences".into(), Value::Array(seqs));
        self.switch_plan(&plan_id);
        Some(seq_id)
    }

    /// Renomme un plan ou une séquence.
    pub fn rename_plan(&mut self, id: &str, name: &str) -> bool {
        let n = clean_name(name);
        if n.is_empty() {
            return false;
        }
        let mut seqs = self.sequences();
        let mut done = false;
        for s in &mut seqs {
            if s["id"].as_str() == Some(id) {
                s["name"] = json!(n);
                done = true;
            }
            for p in s["plans"].as_array_mut().into_iter().flatten() {
                if p["id"].as_str() == Some(id) {
                    p["name"] = json!(n);
                    done = true;
                }
            }
        }
        if done {
            self.project.extra.insert("sequences".into(), Value::Array(seqs));
            self.revision += 1;
        }
        done
    }

    /// Supprime un plan (pas le dernier du projet) ; une séquence vidée disparaît avec lui.
    pub fn remove_plan(&mut self, id: &str) -> bool {
        let seqs = self.sequences();
        let all: Vec<String> = seqs.iter().flat_map(|s| s["plans"].as_array().into_iter().flatten().filter_map(|p| p["id"].as_str().map(str::to_owned))).collect();
        let Some(k) = all.iter().position(|x| x == id) else { return false };
        if all.len() <= 1 {
            return false;
        }
        let (_, cur) = self.current_ids();
        if cur == id {
            let other = all.get(k + 1).or_else(|| all.get(k.wrapping_sub(1))).cloned().unwrap_or_default();
            self.switch_plan(&other);
        }
        let mut seqs = self.sequences();
        for s in &mut seqs {
            if let Some(plans) = s["plans"].as_array_mut() {
                plans.retain(|p| p["id"].as_str() != Some(id));
            }
        }
        seqs.retain(|s| s["plans"].as_array().is_some_and(|p| !p.is_empty()));
        self.project.extra.insert("sequences".into(), Value::Array(seqs));
        self.touch_plans();
        true
    }

    /// Plan suivant (ou précédent) dans l'ordre des séquences.
    pub fn step_plan(&mut self, forward: bool) -> bool {
        let all: Vec<String> = self.plan_info().iter().flat_map(|s| s.plans.iter().map(|p| p.id.clone())).collect();
        let (_, cur) = self.current_ids();
        let Some(k) = all.iter().position(|x| *x == cur) else { return false };
        let next = if forward { all.get(k + 1) } else { k.checked_sub(1).and_then(|i| all.get(i)) };
        next.cloned().is_some_and(|id| self.switch_plan(&id))
    }

    /// Nom du plan ouvert (« Séquence 1 › Plan 2 »).
    pub fn current_plan_label(&self) -> String {
        let (_, cur) = self.current_ids();
        self.plan_info().iter().find_map(|s| s.plans.iter().find(|p| p.id == cur).map(|p| format!("{} › {}", s.name, p.name))).unwrap_or_default()
    }
}
