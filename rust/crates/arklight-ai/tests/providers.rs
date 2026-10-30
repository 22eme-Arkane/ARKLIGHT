//! Fournisseurs face à de faux serveurs locaux : Ollama, serveur compatible OpenAI, Anthropic
//! (conversation avec outils, listes de modèles, erreurs), et le client HTTP (réponses découpées,
//! connexion refusée, arrêt).

mod common;

use arklight_ai::http::{Cancel, HttpError, Request};
use arklight_ai::ollama::{ollama_status, pull_model};
use arklight_ai::providers::{ChatOptions, ToolCall, chat, list_models};
use arklight_ai::{AiSettings, Message};
use common::{FakeServer, Reply, Req, json};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn settings(provider: &str, url: &str) -> AiSettings {
    let mut s = AiSettings::default();
    s.set_provider(provider);
    s.set_model("modele-essai");
    s.set_url(provider, url);
    s
}

fn history() -> Vec<Message> {
    vec![
        Message::User { content: "Ajoute un Titan".into(), images: vec!["data:image/png;base64,iVBORw0KGgo=".into()] },
        Message::Assistant { content: String::new(), tool_calls: vec![ToolCall { id: "c1".into(), name: "get_scene".into(), args: json!({}) }] },
        Message::Tool { tool_call_id: "c1".into(), name: "get_scene".into(), content: "{\"fixtures\":[]}".into() },
    ]
}

fn ask(provider: &str, s: &AiSettings, key: &str, think: Option<bool>) -> Result<arklight_ai::ChatReply, arklight_ai::AiError> {
    let msgs = history();
    let mut loading = || {};
    let mut o = ChatOptions { system: "Consignes", messages: &msgs, tools: arklight_ai::tool_defs(), think, cancel: None, on_loading: &mut loading };
    chat(provider, &s.model, key, s, None, &mut o)
}

#[test]
fn ollama_conversation_et_modeles() {
    let server = FakeServer::start(|r: &Req| match r.route() {
        "/api/ps" => json(200, json!({ "models": [{ "name": "autre:7b", "model": "autre:7b" }] })),
        "/api/chat" => json(200, json!({ "message": { "role": "assistant", "content": "", "tool_calls": [{ "function": { "name": "add_fixtures", "arguments": { "type": "astera.titan", "count": 2 } } }] } })),
        "/api/tags" => json(200, json!({ "models": [
            { "name": "qwen3:8b", "model": "qwen3:8b", "size": 5200000000u64, "capabilities": ["completion", "tools"], "details": { "parameter_size": "8.2B" } },
            { "name": "nomic-embed-text", "model": "nomic-embed-text", "capabilities": ["embedding"] } ] })),
        "/api/version" => json(200, json!({ "version": "0.12.0" })),
        "/api/pull" => Reply::Sse(vec!["{\"status\":\"pulling manifest\"}".into(), "{\"status\":\"downloading\",\"total\":2000000000,\"completed\":1000000000}".into(), "{\"status\":\"success\"}".into()], Vec::new()),
        _ => json(404, json!({ "error": "inconnu" })),
    });
    let s = settings("ollama", &server.url);
    let mut loading = 0;
    let msgs = history();
    let reply = {
        let mut on_loading = || loading += 1;
        let mut o = ChatOptions { system: "Consignes", messages: &msgs, tools: arklight_ai::tool_defs(), think: Some(false), cancel: None, on_loading: &mut on_loading };
        chat("ollama", "modele-essai", "", &s, None, &mut o).unwrap()
    };
    assert_eq!(loading, 1, "modèle pas encore en mémoire : prévenu");
    assert_eq!(reply.tool_calls, vec![ToolCall { id: "t1".into(), name: "add_fixtures".into(), args: json!({ "type": "astera.titan", "count": 2 }) }]);
    let body = server.requests().into_iter().find(|r| r.route() == "/api/chat").unwrap().json();
    assert_eq!(body["think"], false);
    assert_eq!(body["stream"], false);
    assert_eq!(body["options"]["num_ctx"], 16384);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["images"], json!(["iVBORw0KGgo="]));
    assert_eq!(body["messages"][2]["tool_calls"][0]["function"]["name"], "get_scene");
    assert_eq!(body["messages"][3], json!({ "role": "tool", "content": "{\"fixtures\":[]}", "tool_name": "get_scene" }));
    assert_eq!(body["tools"][0]["type"], "function");

    // liste : sans les modèles d'embedding
    let models = list_models("ollama", "", &s, None).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!((models[0].id.as_str(), models[0].params.as_str()), ("qwen3:8b", "8.2B"));
    let st = ollama_status(&server.url);
    assert!(st.running && st.installed && st.version == "0.12.0" && st.models.len() == 2);
    assert_eq!(arklight_ai::ollama::pick_model(&st.models), "qwen3:8b");

    // téléchargement d'un modèle : progression (flux découpé, une ligne JSON par état)
    let mut seen = Vec::new();
    pull_model(&server.url, "qwen3:8b", &mut |p| seen.push(p), None).unwrap();
    assert_eq!(seen.first().map(|p| p.text.as_str()), Some("qwen3:8b — pulling manifest"));
    assert_eq!(seen.last().and_then(|p| p.pct), Some(100.0));
}

#[test]
fn compatible_openai() {
    let server = FakeServer::start(|r: &Req| match (r.method.as_str(), r.route()) {
        ("POST", "/v1/chat/completions") => json(200, json!({ "choices": [{ "message": { "role": "assistant", "content": null, "tool_calls": [{ "id": "call_1", "type": "function", "function": { "name": "set_environment", "arguments": "{\"haze\": 30}" } }] } }] })),
        ("GET", "/v1/models") => json(200, json!({ "data": [{ "id": "models/gemini-zeta" }, { "id": "text-embedding-3" }, { "id": "Alpha" }, { "id": "whisper-1" }, { "id": "beta" }] })),
        _ => json(404, Value::Null),
    });
    let url = format!("{}/v1", server.url);
    // serveur local : pas de clé
    let s = settings("local", &url);
    let reply = ask("local", &s, "", Some(false)).unwrap();
    assert_eq!(reply.tool_calls, vec![ToolCall { id: "call_1".into(), name: "set_environment".into(), args: json!({ "haze": 30 }) }]);
    let req = server.requests().into_iter().find(|r| r.route() == "/v1/chat/completions").unwrap();
    assert!(req.header("authorization").is_none());
    let body = req.json();
    assert_eq!(body["temperature"], 0.2);
    assert_eq!(body["tool_choice"], "auto");
    assert_eq!(body["messages"][1]["content"][1]["image_url"]["url"], "data:image/png;base64,iVBORw0KGgo=");
    assert_eq!(body["messages"][2]["content"], Value::Null);
    assert_eq!(body["messages"][2]["tool_calls"][0]["function"]["arguments"], "{}");
    assert_eq!(body["messages"][3], json!({ "role": "tool", "tool_call_id": "c1", "content": "{\"fixtures\":[]}" }));

    // API en ligne : clé obligatoire, envoyée en « Bearer »
    let s = settings("openai", &url);
    let err = ask("openai", &s, "", None).unwrap_err();
    assert_eq!(err.message, "Clé API OpenAI (GPT) manquante : ajoute-la dans les réglages de l’assistant");
    ask("openai", &s, "sk-essai", None).unwrap();
    let last = server.requests().into_iter().rev().find(|r| r.route() == "/v1/chat/completions").unwrap();
    assert_eq!(last.header("authorization"), Some("Bearer sk-essai"));
    assert!(last.json().get("temperature").is_none(), "OpenAI : température par défaut");
    // « Autre API compatible OpenAI » sur cet ordinateur : sans clé
    let s = settings("custom", &url);
    assert!(ask("custom", &s, "", None).is_ok());

    // modèles : triés, sans embeddings ni audio, « models/ » retiré
    let ids: Vec<String> = list_models("local", "", &settings("local", &url), None).unwrap().into_iter().map(|m| m.id).collect();
    assert_eq!(ids, vec!["Alpha", "beta", "gemini-zeta"]);
    assert!(list_models("openai", "", &settings("openai", &url), None).unwrap().is_empty(), "sans clé : rien");
}

#[test]
fn anthropic() {
    let server = FakeServer::start(|r: &Req| match (r.method.as_str(), r.route()) {
        ("POST", "/v1/messages") => json(200, json!({ "content": [{ "type": "text", "text": "Je regarde." }, { "type": "tool_use", "id": "toolu_1", "name": "get_scene", "input": { "include_objects": false } }] })),
        ("GET", "/v1/models") => json(200, json!({ "data": [{ "id": "claude-sonnet-5-5" }, { "id": "autre" }] })),
        _ => json(404, Value::Null),
    });
    let s = settings("anthropic", &format!("{}/v1", server.url));
    let reply = ask("anthropic", &s, "cle-anthropic", None).unwrap();
    assert_eq!(reply.text, "Je regarde.");
    assert_eq!(reply.tool_calls, vec![ToolCall { id: "toolu_1".into(), name: "get_scene".into(), args: json!({ "include_objects": false }) }]);
    let req = server.requests().into_iter().find(|r| r.route() == "/v1/messages").unwrap();
    assert_eq!(req.header("x-api-key"), Some("cle-anthropic"));
    assert_eq!(req.header("anthropic-version"), Some("2023-06-01"));
    let body = req.json();
    assert_eq!(body["system"], "Consignes");
    assert_eq!(body["max_tokens"], 4096);
    assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
    // l'image et le texte de la demande ; l'appel et son résultat ; rôles alternés
    assert_eq!(body["messages"][0]["content"][0]["source"], json!({ "type": "base64", "media_type": "image/png", "data": "iVBORw0KGgo=" }));
    assert_eq!(body["messages"][1]["content"][0]["type"], "tool_use");
    assert_eq!(body["messages"][2]["content"][0], json!({ "type": "tool_result", "tool_use_id": "c1", "content": "{\"fixtures\":[]}" }));
    let models = list_models("anthropic", "cle", &s, None).unwrap();
    assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["claude-sonnet-5-5"]);
}

#[test]
fn erreurs_et_appels_ecrits_dans_le_texte() {
    let server = FakeServer::start(|r: &Req| match r.route() {
        "/bad/chat/completions" => json(401, json!({ "error": { "message": "Clé refusée", "type": "auth" } })),
        "/text/chat/completions" => json(200, json!({ "choices": [{ "message": { "content": "<tool_call>{\"name\": \"get_scene\", \"arguments\": {}}</tool_call>" } }] })),
        _ => json(500, json!({ "detail": "panne" })),
    });
    let err = ask("local", &settings("local", &format!("{}/bad", server.url)), "", None).unwrap_err();
    assert_eq!((err.message.as_str(), err.status), ("401 — Clé refusée", Some(401)));
    let err = ask("local", &settings("local", &format!("{}/x", server.url)), "", None).unwrap_err();
    assert_eq!(err.message, "500 — {\"detail\":\"panne\"}");
    let reply = ask("local", &settings("local", &format!("{}/text", server.url)), "", None).unwrap();
    assert_eq!((reply.text.as_str(), reply.tool_calls[0].name.as_str()), ("", "get_scene"));

    let port = common::closed_port();
    let err = ask("ollama", &settings("ollama", &format!("http://127.0.0.1:{port}")), "", None).unwrap_err();
    assert_eq!(err.message, format!("Connexion refusée (127.0.0.1:{port}) : le serveur ne tourne pas"));
    let err = ask("inconnu", &AiSettings::default(), "", None).unwrap_err();
    assert_eq!(err.message, "Fournisseur inconnu : inconnu");
    let mut s = settings("ollama", "http://127.0.0.1:1");
    s.model.clear();
    assert_eq!(ask("ollama", &s, "", None).unwrap_err().message, "Choisis un modèle dans les réglages de l’assistant");
}

#[test]
fn client_http_arret_et_delai() {
    let server = FakeServer::start(|_r: &Req| Reply::Slow(Duration::from_secs(3), Box::new(json(200, json!({ "ok": true })))));
    // délai d'inactivité
    let err = Request::get(&format!("{}/lent", server.url)).timeout(Duration::from_millis(300)).send(None).unwrap_err();
    assert_eq!(err, HttpError::Timeout);
    // arrêt depuis un autre fil : la connexion est fermée tout de suite
    let cancel = Cancel::new();
    let c = cancel.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        c.cancel();
    });
    let start = Instant::now();
    let err = Request::get(&format!("{}/lent", server.url)).send(Some(&cancel)).unwrap_err();
    stopper.join().unwrap();
    assert_eq!(err, HttpError::Cancelled);
    assert!(start.elapsed() < Duration::from_secs(2));
}
