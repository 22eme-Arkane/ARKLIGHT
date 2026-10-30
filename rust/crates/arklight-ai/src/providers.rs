//! Fournisseurs d'IA de l'assistant (portage de `server/ai/providers.js`) : Ollama (local et
//! gratuit, par défaut), AIODOS (l'IA locale d'ARKANE, voir [`crate::aiodos`]), serveurs locaux
//! compatibles OpenAI (LM Studio, llama.cpp, Jan…), et API en ligne avec la clé de l'utilisateur
//! (Anthropic, OpenAI, Mistral, Google Gemini, ou toute API compatible OpenAI).
//!
//! Messages dans un format neutre ([`Message`]), convertis pour chaque fournisseur.

use crate::aiodos::Aiodos;
use crate::error::AiError;
use crate::http::{Cancel, Request};
use crate::keys::KeyStore;
use crate::util::obj;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

/// une réponse
pub const TIMEOUT: Duration = Duration::from_secs(240);
/// premier chargement d'un gros modèle local (plusieurs minutes sur un portable)
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(900);
const LIST_TIMEOUT: Duration = Duration::from_secs(15);

/// Façon de parler au fournisseur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Ollama,
    Aiodos,
    /// API compatible OpenAI (`/chat/completions`)
    OpenAi,
    Anthropic,
}

/// Un fournisseur d'IA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ProviderKind,
    /// sur cet ordinateur
    pub local: bool,
    /// adresse par défaut ('' : à donner, ou sans objet pour AIODOS)
    pub url: &'static str,
    /// demande une clé API
    pub key: bool,
    pub default_model: &'static str,
}

const fn p(id: &'static str, label: &'static str, kind: ProviderKind, local: bool, url: &'static str, key: bool, default_model: &'static str) -> Provider {
    Provider { id, label, kind, local, url, key, default_model }
}

pub const PROVIDERS: [Provider; 8] = [
    p("ollama", "Ollama (local, gratuit)", ProviderKind::Ollama, true, "http://localhost:11434", false, ""),
    p("aiodos", "AIODOS (IA locale d’ARKANE)", ProviderKind::Aiodos, true, "", false, ""),
    p("local", "Serveur local compatible OpenAI (LM Studio, llama.cpp, Jan…)", ProviderKind::OpenAi, true, "http://localhost:1234/v1", false, ""),
    p("anthropic", "Anthropic (Claude)", ProviderKind::Anthropic, false, "https://api.anthropic.com/v1", true, "claude-sonnet-5-5"),
    p("openai", "OpenAI (GPT)", ProviderKind::OpenAi, false, "https://api.openai.com/v1", true, ""),
    p("mistral", "Mistral", ProviderKind::OpenAi, false, "https://api.mistral.ai/v1", true, "mistral-small-latest"),
    p("gemini", "Google Gemini", ProviderKind::OpenAi, false, "https://generativelanguage.googleapis.com/v1beta/openai", true, ""),
    p("custom", "Autre API compatible OpenAI", ProviderKind::OpenAi, false, "", true, ""),
];

/// Le fournisseur `id`.
pub fn provider(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

fn default_provider() -> String {
    "aiodos".into()
}

/// Réglages de l'assistant (`config.ai` de la version JavaScript, sans les clés).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiSettings {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// adresses choisies par l'utilisateur, par fournisseur
    #[serde(default)]
    pub urls: BTreeMap<String, String>,
}

impl Default for AiSettings {
    fn default() -> Self {
        AiSettings { provider: default_provider(), model: String::new(), urls: BTreeMap::new() }
    }
}

impl AiSettings {
    /// Change de fournisseur (le modèle repart de celui conseillé), comme `ai.config`.
    pub fn set_provider(&mut self, id: &str) {
        if let Some(p) = provider(id) {
            if p.id != self.provider {
                self.model = p.default_model.to_owned();
            }
            self.provider = p.id.to_owned();
        }
    }

    /// Choisit le modèle (120 caractères au plus).
    pub fn set_model(&mut self, model: &str) {
        self.model = model.trim().chars().take(120).collect();
    }

    /// Adresse d'un fournisseur ('' : celle par défaut).
    pub fn set_url(&mut self, id: &str, url: &str) {
        if provider(id).is_some() {
            self.urls.insert(id.to_owned(), url.trim().chars().take(300).collect());
        }
    }
}

/// Adresse du fournisseur (réglage de l'utilisateur, sinon celle par défaut), sans « / » final.
pub fn provider_url(id: &str, settings: &AiSettings) -> String {
    let custom = settings.urls.get(id).map(|s| s.trim()).filter(|s| !s.is_empty());
    let url = custom.or_else(|| provider(id).map(|p| p.url)).unwrap_or("");
    url.trim_end_matches('/').to_owned()
}

// ------------------------------------------------------------------ messages
/// Un appel d'outil demandé par le modèle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// arguments (objet JSON)
    pub args: Value,
}

/// Un message de la conversation (format neutre).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    /// la demande ; `images` : photos en data URL (data:image/jpeg;base64,…)
    User {
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
    },
    Assistant {
        content: String,
        #[serde(default, rename = "toolCalls", skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    /// résultat d'un outil, relu par le modèle
    Tool {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        name: String,
        content: String,
    },
}

/// Qui a calculé la réponse (AIODOS le dit : modèle, machine, lieu).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AnsweredBy {
    pub model: String,
    pub machine: String,
    pub place: String,
}

/// Un tour de conversation : le texte, et les outils à appeler.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatReply {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub by: Option<AnsweredBy>,
}

/// Un modèle, pour la liste des réglages.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    /// octets (0 : inconnu)
    #[serde(default)]
    pub size: f64,
    /// capacités : completion, tools, vision, thinking, embedding
    #[serde(default)]
    pub caps: Vec<String>,
    /// « 9B »
    #[serde(default)]
    pub params: String,
    /// AIODOS : machine qui l'a, sur ce PC, chargé, celui d'AIODOS, contexte
    #[serde(default)]
    pub machine: String,
    #[serde(default = "yes")]
    pub here: bool,
    #[serde(default)]
    pub loaded: bool,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub context: Option<f64>,
    /// installé (AIODOS)
    #[serde(default = "yes")]
    pub installed: bool,
}

fn yes() -> bool {
    true
}

impl ModelInfo {
    pub fn new(id: &str) -> ModelInfo {
        ModelInfo { id: id.to_owned(), here: true, installed: true, ..ModelInfo::default() }
    }

    pub fn has(&self, cap: &str) -> bool {
        self.caps.iter().any(|c| c == cap)
    }
}

/// Partie d'une data URL : (type, données base64).
pub(crate) fn data_url_parts(u: &str) -> Option<(&str, &str)> {
    let rest = u.strip_prefix("data:")?;
    let (mime, data) = rest.split_once(";base64,")?;
    if mime.is_empty() || mime.contains(';') { None } else { Some((mime, data)) }
}

/// Arguments d'un appel d'outil : objet, ou texte JSON (parfois mal formé chez les petits modèles).
pub(crate) fn parse_args(a: Option<&Value>) -> Value {
    match a {
        Some(v @ (Value::Object(_) | Value::Array(_))) => v.clone(),
        Some(Value::String(s)) => serde_json::from_str(if s.is_empty() { "{}" } else { s }).unwrap_or_else(|_| json!({})),
        None | Some(Value::Null) => json!({}),
        Some(Value::Bool(false)) => json!({}),
        Some(v) => serde_json::from_str(&crate::util::js_string(v)).unwrap_or_else(|_| json!({})),
    }
}

/// Appels d'outils écrits dans le texte par certains modèles locaux (au lieu du champ prévu) :
/// `{"name": "...", "arguments": {...}}` ou `<tool_call>{...}</tool_call>`.
pub fn tool_calls_in_text(text: &str, tool_names: &HashSet<&str>) -> Vec<ToolCall> {
    let mut out: Vec<ToolCall> = Vec::new();
    let tagged = regex::Regex::new(r"(?s)<tool_call>\s*(.*?)\s*</tool_call>").expect("motif");
    let mut candidates: Vec<String> = tagged.captures_iter(text).map(|c| c[1].to_owned()).collect();
    if candidates.is_empty() {
        let fences = regex::Regex::new(r"^```(?:json)?\s*|\s*```$").expect("motif");
        let trimmed = fences.replace_all(text.trim(), "").into_owned();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            candidates.push(trimmed);
        }
    }
    for c in candidates {
        let Ok(v) = serde_json::from_str::<Value>(&c) else { continue };
        let items = match v {
            Value::Array(a) => a,
            other => vec![other],
        };
        for it in items {
            let name = [it.get("name"), it.get("function").and_then(|f| f.get("name"))].into_iter().flatten().find(|n| crate::util::truthy(Some(n))).and_then(Value::as_str).map(str::to_owned);
            let Some(name) = name.filter(|n| tool_names.contains(n.as_str())) else { continue };
            let raw = it.get("arguments").filter(|v| !v.is_null()).or_else(|| it.get("parameters").filter(|v| !v.is_null())).or_else(|| it.get("function").and_then(|f| f.get("arguments")));
            out.push(ToolCall { id: format!("t{}", out.len() + 1), name, args: parse_args(raw) });
        }
    }
    out
}

// ------------------------------------------------------------------ requêtes
/// Requête à un fournisseur : erreurs avec les messages de `providers.js`.
fn request(req: Request, cancel: Option<&Cancel>) -> Result<Value, AiError> {
    let res = req.send(cancel).map_err(AiError::from_provider)?;
    let status = res.status;
    let ok = res.ok();
    let (text, data) = res.text_and_json().map_err(AiError::from_provider)?;
    if !ok {
        let truthy = |v: Option<&Value>| v.filter(|x| crate::util::truthy(Some(x))).cloned();
        let err = data.get("error");
        let picked = truthy(err.and_then(|e| e.get("message")))
            .or_else(|| truthy(err))
            .or_else(|| truthy(data.get("message")))
            .or_else(|| Some(Value::String(text.chars().take(300).collect())).filter(|v| v.as_str().is_some_and(|s| !s.is_empty())))
            .unwrap_or_else(|| Value::String(format!("HTTP {status}")));
        let msg = match picked {
            Value::String(s) => s,
            other => other.to_string(),
        };
        let mut e = AiError::new(format!("{status} — {msg}"));
        e.status = Some(status);
        return Err(e);
    }
    Ok(data)
}

/// Les outils au format « function » d'OpenAI et d'Ollama.
fn function_tools(tools: &[Value]) -> Value {
    Value::Array(tools.iter().map(|t| json!({ "type": "function", "function": t })).collect())
}

/// Ce qu'il faut pour un tour de conversation.
pub struct ChatOptions<'a> {
    pub system: &'a str,
    pub messages: &'a [Message],
    /// définitions des outils ({ name, description, parameters })
    pub tools: &'a [Value],
    /// Some(false) : sans « réflexion » (Ollama) ; None : réglage du modèle
    pub think: Option<bool>,
    pub cancel: Option<&'a Cancel>,
    /// le modèle n'est pas encore chargé (« chargement du modèle… »)
    pub on_loading: &'a mut dyn FnMut(),
}

// ------------------------------------------------------------------ Ollama
/// Le modèle est-il déjà en mémoire dans Ollama ? (None : on ne sait pas)
fn ollama_loaded(url: &str, model: &str, cancel: Option<&Cancel>) -> Option<bool> {
    let ps = request(Request::get(&format!("{url}/api/ps")).timeout(Duration::from_secs(4)), cancel).ok()?;
    let names: Vec<&str> = ps.get("models").and_then(Value::as_array).into_iter().flatten().flat_map(|m| [m.get("name"), m.get("model")]).flatten().filter_map(Value::as_str).collect();
    let latest = format!("{model}:latest");
    Some(names.contains(&model) || names.contains(&latest.as_str()))
}

fn chat_ollama(url: &str, model: &str, o: &mut ChatOptions) -> Result<ChatReply, AiError> {
    // pas encore chargé : on prévient (« chargement du modèle… ») et on lui laisse le temps
    let loaded = ollama_loaded(url, model, o.cancel);
    if loaded == Some(false) {
        (o.on_loading)();
    }
    let mut msgs = vec![json!({ "role": "system", "content": o.system })];
    for m in o.messages {
        msgs.push(match m {
            Message::Tool { content, name, .. } => json!({ "role": "tool", "content": content, "tool_name": name }),
            Message::Assistant { content, tool_calls } => {
                let mut v = json!({ "role": "assistant", "content": content });
                if !tool_calls.is_empty() {
                    v["tool_calls"] = Value::Array(tool_calls.iter().map(|c| json!({ "function": { "name": c.name, "arguments": c.args } })).collect());
                }
                v
            }
            Message::User { content, images } => {
                let mut v = json!({ "role": "user", "content": content });
                if !images.is_empty() {
                    v["images"] = Value::Array(images.iter().filter_map(|u| data_url_parts(u)).map(|(_, d)| Value::from(d)).collect());
                }
                v
            }
        });
    }
    let mut body = json!({
        "model": model,
        "messages": msgs,
        "stream": false,
        "keep_alive": "15m",
        "options": { "temperature": 0.2, "num_ctx": 16384 },
    });
    if !o.tools.is_empty() {
        body["tools"] = function_tools(o.tools);
    }
    if let Some(t) = o.think {
        body["think"] = Value::Bool(t);
    }
    let timeout = if loaded == Some(false) { LOAD_TIMEOUT } else { TIMEOUT };
    let data = request(Request::post_json(&format!("{url}/api/chat"), &body).timeout(timeout), o.cancel)?;
    let msg = data.get("message").cloned().unwrap_or(Value::Null);
    let tool_calls = msg
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, c)| {
            let f = c.get("function")?;
            let name = f.get("name").and_then(Value::as_str).filter(|n| !n.is_empty())?;
            Some(ToolCall { id: format!("t{}", i + 1), name: name.to_owned(), args: parse_args(f.get("arguments")) })
        })
        .collect();
    Ok(ChatReply { text: msg.get("content").and_then(Value::as_str).unwrap_or("").to_owned(), tool_calls, by: None })
}

// ------------------------------------------------------------------ compatible OpenAI
fn chat_openai(id: &str, url: &str, key: &str, model: &str, o: &mut ChatOptions) -> Result<ChatReply, AiError> {
    let mut msgs = vec![json!({ "role": "system", "content": o.system })];
    for m in o.messages {
        msgs.push(match m {
            Message::Tool { tool_call_id, content, .. } => json!({ "role": "tool", "tool_call_id": tool_call_id, "content": content }),
            Message::Assistant { content, tool_calls } => {
                let c = if !content.is_empty() {
                    Value::from(content.as_str())
                } else if tool_calls.is_empty() {
                    Value::from("")
                } else {
                    Value::Null
                };
                let mut v = json!({ "role": "assistant", "content": c });
                if !tool_calls.is_empty() {
                    v["tool_calls"] = Value::Array(tool_calls.iter().map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.args.to_string() } })).collect());
                }
                v
            }
            Message::User { content, images } => {
                if images.is_empty() {
                    json!({ "role": "user", "content": content })
                } else {
                    let mut parts = vec![json!({ "type": "text", "text": content })];
                    parts.extend(images.iter().map(|u| json!({ "type": "image_url", "image_url": { "url": u } })));
                    json!({ "role": "user", "content": parts })
                }
            }
        });
    }
    let mut body = json!({ "model": model, "messages": msgs });
    if !o.tools.is_empty() {
        body["tools"] = function_tools(o.tools);
        body["tool_choice"] = "auto".into();
    }
    // les modèles « à raisonnement » d'OpenAI n'acceptent que la température par défaut
    if id != "openai" && id != "gemini" {
        body["temperature"] = json!(0.2);
    }
    let mut req = Request::post_json(&format!("{url}/chat/completions"), &body);
    if !key.is_empty() {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let data = request(req, o.cancel)?;
    let msg = data.get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("message")).cloned().unwrap_or(Value::Null);
    let text = match msg.get("content") {
        Some(Value::Array(parts)) => parts.iter().map(|p| p.get("text").and_then(Value::as_str).unwrap_or("")).collect::<String>(),
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    let tool_calls = msg
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, c)| {
            let f = c.get("function")?;
            let name = f.get("name").and_then(Value::as_str).filter(|n| !n.is_empty())?;
            let id = c.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()).map_or_else(|| format!("t{}", i + 1), str::to_owned);
            Some(ToolCall { id, name: name.to_owned(), args: parse_args(f.get("arguments")) })
        })
        .collect();
    Ok(ChatReply { text, tool_calls, by: None })
}

// ------------------------------------------------------------------ Anthropic
fn chat_anthropic(url: &str, key: &str, model: &str, o: &mut ChatOptions) -> Result<ChatReply, AiError> {
    let mut msgs: Vec<(String, Vec<Value>)> = Vec::new();
    let mut push = |role: &str, blocks: Vec<Value>| match msgs.last_mut() {
        Some((r, b)) if r == role => b.extend(blocks),
        _ => msgs.push((role.to_owned(), blocks)),
    };
    for m in o.messages {
        match m {
            Message::Tool { tool_call_id, content, .. } => push("user", vec![json!({ "type": "tool_result", "tool_use_id": tool_call_id, "content": content })]),
            Message::Assistant { content, tool_calls } => {
                let mut blocks = Vec::new();
                if !content.is_empty() {
                    blocks.push(json!({ "type": "text", "text": content }));
                }
                for c in tool_calls {
                    let input = if c.args.is_object() { c.args.clone() } else { json!({}) };
                    blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": input }));
                }
                if !blocks.is_empty() {
                    push("assistant", blocks);
                }
            }
            Message::User { content, images } => {
                let mut blocks: Vec<Value> = images.iter().filter_map(|u| data_url_parts(u)).map(|(mime, data)| json!({ "type": "image", "source": { "type": "base64", "media_type": mime, "data": data } })).collect();
                blocks.push(json!({ "type": "text", "text": if content.is_empty() { "…" } else { content.as_str() } }));
                push("user", blocks);
            }
        }
    }
    let messages: Vec<Value> = msgs.into_iter().map(|(role, content)| json!({ "role": role, "content": content })).collect();
    let mut body = json!({ "model": model, "max_tokens": 4096, "system": o.system, "messages": messages });
    if !o.tools.is_empty() {
        body["tools"] = Value::Array(o.tools.iter().map(|t| json!({ "name": t["name"], "description": t["description"], "input_schema": t["parameters"] })).collect());
    }
    let req = Request::post_json(&format!("{url}/messages"), &body).header("x-api-key", key).header("anthropic-version", "2023-06-01");
    let data = request(req, o.cancel)?;
    let blocks = data.get("content").and_then(Value::as_array).cloned().unwrap_or_default();
    let kind = |b: &Value, k: &str| b.get("type").and_then(Value::as_str) == Some(k);
    Ok(ChatReply {
        text: blocks.iter().filter(|b| kind(b, "text")).filter_map(|b| b.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n"),
        tool_calls: blocks
            .iter()
            .filter(|b| kind(b, "tool_use"))
            .map(|b| ToolCall {
                id: b.get("id").and_then(Value::as_str).unwrap_or("").to_owned(),
                name: b.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
                args: b.get("input").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| json!({})),
            })
            .collect(),
        by: None,
    })
}

/// Un tour de conversation avec le modèle choisi.
pub fn chat(provider_id: &str, model: &str, key: &str, settings: &AiSettings, aiodos: Option<&Aiodos>, o: &mut ChatOptions) -> Result<ChatReply, AiError> {
    let p = provider(provider_id).ok_or_else(|| AiError::new(format!("Fournisseur inconnu : {provider_id}")))?;
    if model.is_empty() {
        return Err(AiError::new("Choisis un modèle dans les réglages de l’assistant"));
    }
    let local_custom = provider_id == "custom" && {
        let u = provider_url(provider_id, settings);
        u.contains("localhost") || u.contains("127.0.0.1")
    };
    if p.key && key.is_empty() && !local_custom {
        return Err(AiError::with_code("needs_key", format!("Clé API {} manquante : ajoute-la dans les réglages de l’assistant", p.label)));
    }
    if p.kind == ProviderKind::Aiodos {
        let a = aiodos.ok_or_else(|| AiError::new("AIODOS indisponible"))?;
        return a.chat(model, o);
    }
    let url = provider_url(provider_id, settings);
    if url.is_empty() {
        return Err(AiError::new("Adresse du serveur manquante"));
    }
    let mut res = match p.kind {
        ProviderKind::Ollama => chat_ollama(&url, model, o)?,
        ProviderKind::Anthropic => chat_anthropic(&url, key, model, o)?,
        _ => chat_openai(provider_id, &url, key, model, o)?,
    };
    // certains modèles locaux écrivent l'appel d'outil dans le texte
    if res.tool_calls.is_empty() && !o.tools.is_empty() && (res.text.contains("<tool_call>") || regex::Regex::new(r#""name"\s*:"#).expect("motif").is_match(&res.text)) {
        let names: HashSet<&str> = o.tools.iter().filter_map(|t| t.get("name").and_then(Value::as_str)).collect();
        let found = tool_calls_in_text(&res.text, &names);
        if !found.is_empty() {
            res = ChatReply { text: String::new(), tool_calls: found, by: None };
        }
    }
    Ok(res)
}

/// Modèles disponibles chez un fournisseur (pour la liste des réglages).
pub fn list_models(provider_id: &str, key: &str, settings: &AiSettings, aiodos: Option<&Aiodos>) -> Result<Vec<ModelInfo>, AiError> {
    let Some(p) = provider(provider_id) else { return Ok(Vec::new()) };
    if p.kind == ProviderKind::Aiodos {
        return aiodos.map_or(Ok(Vec::new()), Aiodos::models);
    }
    let url = provider_url(provider_id, settings);
    if p.kind == ProviderKind::Ollama {
        let data = request(Request::get(&format!("{url}/api/tags")).timeout(LIST_TIMEOUT), None)?;
        return Ok(data
            .get("models")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|m| m.get("capabilities").and_then(Value::as_array).is_none_or(|c| c.iter().any(|x| x == "completion")))
            .map(ollama_model)
            .collect());
    }
    if p.key && key.is_empty() && provider_id != "custom" {
        return Ok(Vec::new());
    }
    if p.kind == ProviderKind::Anthropic {
        let req = Request::get(&format!("{url}/models?limit=100")).header("x-api-key", key).header("anthropic-version", "2023-06-01").timeout(LIST_TIMEOUT);
        let data = request(req, None)?;
        return Ok(data
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| m.get("id").and_then(Value::as_str))
            .filter(|id| id.starts_with("claude-"))
            .map(|id| ModelInfo { caps: vec!["tools".into(), "vision".into()], ..ModelInfo::new(id) })
            .collect());
    }
    let mut req = Request::get(&format!("{url}/models")).timeout(LIST_TIMEOUT);
    if !key.is_empty() {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let data = request(req, None)?;
    let skip = regex::Regex::new(r"(?i)embed|whisper|tts|audio|image|dall-e|moderation|realtime|transcribe|search|rerank|ocr").expect("motif");
    let mut ids: Vec<String> = data
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|m| m.get("id").map(crate::util::js_string).unwrap_or_default())
        .map(|id| id.strip_prefix("models/").map(str::to_owned).unwrap_or(id))
        .filter(|id| !id.is_empty() && !skip.is_match(id))
        .collect();
    ids.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    Ok(ids.iter().map(|id| ModelInfo::new(id)).collect())
}

/// Un modèle de la liste d'Ollama (`/api/tags`).
pub(crate) fn ollama_model(m: &Value) -> ModelInfo {
    let id = m.get("model").filter(|v| crate::util::truthy(Some(v))).or_else(|| m.get("name")).and_then(Value::as_str).unwrap_or("");
    ModelInfo {
        size: m.get("size").and_then(Value::as_f64).unwrap_or(0.0),
        caps: m.get("capabilities").and_then(Value::as_array).map(|c| c.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default(),
        params: m.get("details").and_then(|d| d.get("parameter_size")).and_then(Value::as_str).unwrap_or("").to_owned(),
        ..ModelInfo::new(id)
    }
}

// ------------------------------------------------------------------ l'ensemble
/// Ce qui sert à parler au modèle : réglages, clés, AIODOS.
#[derive(Clone)]
pub struct Backend {
    pub settings: AiSettings,
    pub keys: Arc<KeyStore>,
    /// AIODOS (absent : le fournisseur « aiodos » répond « AIODOS indisponible »)
    pub aiodos: Option<Arc<Aiodos>>,
}

impl Backend {
    pub fn new(settings: AiSettings, keys: Arc<KeyStore>, aiodos: Option<Arc<Aiodos>>) -> Backend {
        Backend { settings, keys, aiodos }
    }

    /// Un tour de conversation avec le fournisseur et le modèle des réglages.
    pub fn chat(&self, o: &mut ChatOptions) -> Result<ChatReply, AiError> {
        let key = self.keys.get(&self.settings.provider);
        chat(&self.settings.provider, &self.settings.model, &key, &self.settings, self.aiodos.as_deref(), o)
    }

    /// Modèles d'un fournisseur (celui des réglages si `provider` est None).
    pub fn list_models(&self, provider: Option<&str>) -> Result<Vec<ModelInfo>, AiError> {
        let id = provider.filter(|p| self::provider(p).is_some()).unwrap_or(&self.settings.provider);
        list_models(id, &self.keys.get(id), &self.settings, self.aiodos.as_deref())
    }

    /// Fournisseurs pour les réglages de l'interface (jamais les clés elles-mêmes).
    pub fn providers_state(&self) -> Value {
        Value::Array(
            PROVIDERS
                .iter()
                .map(|p| {
                    obj([
                        ("id", p.id.into()),
                        ("label", p.label.into()),
                        ("key", p.key.into()),
                        ("local", p.local.into()),
                        ("url", provider_url(p.id, &self.settings).into()),
                        ("defaultUrl", p.url.into()),
                        ("defaultModel", p.default_model.into()),
                        ("hasKey", (!self.keys.get(p.id).is_empty()).into()),
                    ])
                })
                .collect(),
        )
    }
}

/// Liste au format JSON (pour l'interface).
pub fn models_json(list: &[ModelInfo]) -> Value {
    serde_json::to_value(list).unwrap_or_else(|_| Value::Array(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appels_dans_le_texte() {
        let names: HashSet<&str> = ["get_scene", "add_fixtures"].into_iter().collect();
        let t = tool_calls_in_text("Je regarde.\n<tool_call>{\"name\": \"get_scene\", \"arguments\": {}}</tool_call>", &names);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].name, "get_scene");
        let t = tool_calls_in_text("```json\n[{\"function\": {\"name\": \"add_fixtures\", \"arguments\": \"{\\\"type\\\": \\\"astera.titan\\\"}\"}}, {\"name\": \"inconnu\"}]\n```", &names);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].args, json!({ "type": "astera.titan" }));
        assert!(tool_calls_in_text("Bonjour", &names).is_empty());
    }

    #[test]
    fn adresses_des_fournisseurs() {
        let mut s = AiSettings::default();
        assert_eq!(provider_url("ollama", &s), "http://localhost:11434");
        s.set_url("ollama", "http://127.0.0.1:9999/ ");
        assert_eq!(provider_url("ollama", &s), "http://127.0.0.1:9999");
        s.set_provider("anthropic");
        assert_eq!(s.model, "claude-sonnet-5-5");
        assert_eq!(data_url_parts("data:image/png;base64,AAAA"), Some(("image/png", "AAAA")));
    }
}
