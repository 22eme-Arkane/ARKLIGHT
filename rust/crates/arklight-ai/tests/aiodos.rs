//! AIODOS face à un faux AIODOS local : accès par code à 6 chiffres (relève de la clé), preuve
//! d'identité avant tout envoi de la clé, état, modèles, conversation en flux avec appels d'outils,
//! retrait ; refus, preuve fausse, abandon.

mod common;

use arklight_ai::aiodos::compute_proof;
use arklight_ai::providers::ChatOptions;
use arklight_ai::{AiSettings, Aiodos, AiodosOptions, Backend, KeyStore, Message};
use common::{FakeServer, Reply, Req, aiodos_json};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const KEY: &str = "aiodos_app_k3j7m2qa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn info(auth: &str) -> Value {
    json!({ "object": "aiodos.info", "product": "aiodos", "version": "2.1.0", "api": 1, "instance": "Hk3v9QmZ",
        "features": ["access", "models", "chat", "embed", "open"], "access": { "apps": true, "requests": true, "approval_ui": true }, "auth": auth })
}

/// Le faux AIODOS : `polls_before_grant` relèves « pas encore » avant l'accord ; `outcome` :
/// « grant », « deny » ou « wait » (jamais de réponse) ; `honest` : preuve juste.
fn fake_aiodos(polls_before_grant: usize, outcome: &'static str, honest: bool) -> FakeServer {
    let polls = Arc::new(AtomicUsize::new(0));
    let chats = Arc::new(AtomicUsize::new(0));
    FakeServer::start(move |r: &Req| {
        let keyed = r.header("authorization") == Some(&format!("Bearer {KEY}"));
        match (r.method.as_str(), r.route()) {
            ("GET", "/aiodos/v1/info") => {
                if let Some(challenge) = r.query("challenge") {
                    let mut v = info("none");
                    v["proof"] = if honest { compute_proof(KEY, &challenge) } else { compute_proof("aiodos_app_zzzzzzzz_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB", &challenge) }.into();
                    return aiodos_json(200, v);
                }
                aiodos_json(200, info(if keyed { "valid" } else { "none" }))
                    .with_app(keyed)
            }
            ("POST", "/aiodos/v1/access/requests") => aiodos_json(200, json!({ "device_code": "Zb4l3Fq0iTq9mJmQyH8w7mY2uYV1zq3Kk1vB0aE1cQo", "user_code": "482 197", "expires_in": 600, "interval": 0.1, "request_id": "req_8f3KqZ2m" })),
            ("POST", "/aiodos/v1/access/token") => {
                let n = polls.fetch_add(1, Ordering::SeqCst);
                if outcome == "wait" || n < polls_before_grant {
                    return aiodos_json(400, json!({ "error": "authorization_pending", "error_description": "La demande attend la réponse de l'utilisateur dans AIODOS." }));
                }
                if outcome == "deny" {
                    return aiodos_json(400, json!({ "error": "access_denied", "error_description": "L'accès a été refusé dans AIODOS." }));
                }
                aiodos_json(200, json!({ "access_token": KEY, "token_type": "Bearer", "scope": "chat", "aiodos": { "app_id": "k3j7m2qa", "where": ["here"], "priority": "interactive", "expires": null } }))
            }
            ("GET", "/aiodos/v1/models") if keyed => aiodos_json(
                200,
                json!({ "object": "list", "models": [
                    { "id": "mistral:7b", "kind": "chat", "installed": true, "context": 16384, "vision": false, "tools": true, "thinking": false, "size_bytes": 4400000000u64, "loaded": true, "here": true, "default": false,
                      "where": [{ "id": "local:mistral:7b", "machine": "PC-BUREAU", "place": "here", "loaded": true }] },
                    { "id": "qwen3.5:9b", "kind": "chat", "installed": true, "context": 16384, "vision": true, "tools": true, "thinking": true, "size_bytes": 6600000000u64, "loaded": false, "here": true, "default": true,
                      "where": [{ "id": "local:qwen3.5:9b", "machine": "PC-BUREAU", "place": "here", "loaded": false }] } ] }),
            ),
            ("POST", "/v1/chat/completions") if keyed => {
                let headers = vec![("X-Aiodos-Api", "1".to_owned()), ("X-Aiodos-Model", "qwen3.5%3A9b".to_owned()), ("X-Aiodos-Machine", "PC%20BUREAU".to_owned()), ("X-Aiodos-Place", "here".to_owned())];
                let chunk = |delta: Value, finish: Value| format!("data: {}\n", json!({ "object": "chat.completion.chunk", "model": "qwen3.5:9b", "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }] }));
                if chats.fetch_add(1, Ordering::SeqCst) == 0 {
                    Reply::Sse(
                        vec![
                            ": aiodos loading 5\n".into(),
                            chunk(json!({ "role": "assistant", "content": null }), Value::Null),
                            chunk(json!({ "tool_calls": [{ "index": 0, "id": "call_8yT2", "type": "function", "function": { "name": "set_environment", "arguments": "" } }] }), Value::Null),
                            chunk(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "{\"haze\": 40," } }] }), Value::Null),
                            chunk(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": " \"floor\": false}" } }] }), Value::Null),
                            format!("data: {}\n", json!({ "choices": [{ "index": 0, "delta": {}, "finish_reason": "tool_calls" }], "aiodos": { "model": "qwen3.5:9b", "machine": "PC-BUREAU", "place": "here", "stopped_by": "tool_calls" } })),
                            "data: [DONE]\n".into(),
                        ],
                        headers,
                    )
                } else {
                    Reply::Sse(vec![chunk(json!({ "content": "Brume" }), Value::Null), chunk(json!({ "content": " réglée." }), json!("stop")), "data: [DONE]\n".into()], headers)
                }
            }
            ("POST", "/aiodos/v1/access/revoke") if keyed => aiodos_json(200, json!({ "revoked": true })),
            _ => aiodos_json(401, json!({ "error": { "message": "Clé d'accès d'AIODOS manquante ou invalide.", "type": "invalid_request_error", "code": "invalid_api_key" } })),
        }
    })
}

trait WithApp {
    fn with_app(self, keyed: bool) -> Reply;
}

impl WithApp for Reply {
    /// L'info avec une clé valide dit les droits de l'appli.
    fn with_app(self, keyed: bool) -> Reply {
        match self {
            Reply::Json(s, mut v, h) if keyed => {
                v["app"] = json!({ "id": "k3j7m2qa", "name": "ARKLIGHT", "kind": "app", "scopes": ["chat"], "where": ["here"] });
                Reply::Json(s, v, h)
            }
            other => other,
        }
    }
}

fn client(server: &FakeServer, keys: Arc<KeyStore>) -> Aiodos {
    let opts = AiodosOptions { url: Some(server.url.clone()), system: false, ..AiodosOptions::arklight("0.2.0-alpha.1") };
    Aiodos::new(opts, keys)
}

#[test]
fn acces_modeles_conversation_et_retrait() {
    let server = fake_aiodos(2, "grant", true);
    let keys = Arc::new(KeyStore::in_memory());
    let aiodos = Arc::new(client(&server, keys.clone()));

    let st = aiodos.status();
    assert_eq!(st.state, "needs_access", "{st:?}");
    assert_eq!(st.version.as_deref(), Some("2.1.0"));

    // le code à 6 chiffres arrive, puis l'accord après deux relèves « pas encore »
    let mut codes = Vec::new();
    let grant = aiodos.connect(&mut |c| codes.push(c.to_owned())).expect("accès accordé");
    assert_eq!(codes, vec!["482 197".to_owned()]);
    assert_eq!(grant.scopes, vec!["chat".to_owned()]);
    assert_eq!(grant.where_, vec!["here".to_owned()]);
    assert_eq!(keys.get("aiodos"), KEY);

    let st = aiodos.status();
    assert_eq!((st.state.as_str(), st.scopes.clone()), ("ready", vec!["chat".to_owned()]), "{st:?}");

    // modèles : le conseillé voit les images, sait les outils, et c'est celui d'AIODOS
    let models = aiodos.models().unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(models[1].params, "9B");
    assert!(models[1].has("vision") && models[1].has("tools") && models[1].default);
    assert_eq!(models[0].machine, "PC-BUREAU");
    assert_eq!(aiodos.pick_model().unwrap(), "qwen3.5:9b");

    // conversation par le fournisseur « aiodos »
    let mut settings = AiSettings::default();
    settings.set_provider("aiodos");
    settings.set_model("qwen3.5:9b");
    let backend = Backend::new(settings, keys.clone(), Some(aiodos.clone()));
    let tools = arklight_ai::tool_defs();
    let messages = vec![Message::User { content: "Plus de brume, sans sol".into(), images: Vec::new() }];
    let mut loading = 0;
    let reply = {
        let mut on_loading = || loading += 1;
        let mut o = ChatOptions { system: "Consignes", messages: &messages, tools, think: Some(false), cancel: None, on_loading: &mut on_loading };
        backend.chat(&mut o).unwrap()
    };
    assert!(loading >= 1, "modèle pas chargé : prévenu");
    assert_eq!(reply.tool_calls.len(), 1);
    assert_eq!(reply.tool_calls[0].id, "call_8yT2");
    assert_eq!(reply.tool_calls[0].name, "set_environment");
    assert_eq!(reply.tool_calls[0].args, json!({ "haze": 40, "floor": false }));
    let by = reply.by.unwrap();
    assert_eq!((by.model.as_str(), by.machine.as_str(), by.place.as_str()), ("qwen3.5:9b", "PC-BUREAU", "here"));

    let mut nothing = || {};
    let mut o = ChatOptions { system: "", messages: &messages, tools: &[], think: None, cancel: None, on_loading: &mut nothing };
    assert_eq!(backend.chat(&mut o).unwrap().text, "Brume réglée.");

    // retrait : AIODOS confirme, la clé est effacée
    assert!(aiodos.disconnect().unwrap());
    assert_eq!(keys.get("aiodos"), "");
    assert_eq!(aiodos.status().state, "needs_access");

    // ce qui a circulé : la clé n'est envoyée qu'après une preuve juste, jamais dans une relève
    let reqs = server.requests();
    let first_keyed = reqs.iter().position(|r| r.header("authorization").is_some()).unwrap();
    assert!(reqs[..first_keyed].iter().any(|r| r.query("challenge").is_some()), "preuve avant la clé");
    assert!(reqs.iter().all(|r| r.header("x-aiodos-client").is_some_and(|c| c.starts_with("rust/"))));
    let polls: Vec<&Req> = reqs.iter().filter(|r| r.route() == "/aiodos/v1/access/token").collect();
    assert_eq!(polls.len(), 3);
    assert!(polls.iter().all(|r| r.header("authorization").is_none() && r.json()["grant_type"] == "urn:ietf:params:oauth:grant-type:device_code"));
    let request = reqs.iter().find(|r| r.route() == "/aiodos/v1/access/requests").unwrap().json();
    assert_eq!(request["client_name"], "ARKLIGHT");
    assert_eq!(request["client_id"], "arkane.arklight");
    assert_eq!(request["scope"], "chat");
    assert_eq!(request["where"], json!(["here"]));
    let chat = reqs.iter().find(|r| r.route() == "/v1/chat/completions").unwrap();
    assert_eq!(chat.header("x-aiodos-where"), Some("here"));
    assert_eq!(chat.header("accept"), Some("text/event-stream"));
    let body = chat.json();
    assert_eq!(body["stream"], true);
    assert_eq!(body["reasoning_effort"], "none");
    assert_eq!(body["messages"][0], json!({ "role": "system", "content": "Consignes" }));
    assert_eq!(body["tools"].as_array().unwrap().len(), 13);
    assert_eq!(body["tools"][0]["type"], "function");
}

#[test]
fn acces_refuse() {
    let server = fake_aiodos(1, "deny", true);
    let aiodos = client(&server, Arc::new(KeyStore::in_memory()));
    let err = aiodos.connect(&mut |_| {}).unwrap_err();
    assert_eq!(err.code(), "access_denied");
    assert_eq!(err.message, "L'accès a été refusé dans AIODOS.");
    assert_eq!(aiodos.status().state, "denied");
}

#[test]
fn preuve_fausse_la_cle_ne_part_pas() {
    let server = fake_aiodos(0, "grant", false);
    let keys = Arc::new(KeyStore::in_memory());
    keys.set("aiodos", KEY).unwrap();
    let aiodos = client(&server, keys);
    let err = aiodos.models().unwrap_err();
    assert_eq!(err.code(), "untrusted");
    assert_eq!(aiodos.status().state, "untrusted");
    assert!(server.requests().iter().all(|r| r.header("authorization").is_none()), "aucune requête avec la clé");
}

#[test]
fn demande_abandonnee() {
    let server = fake_aiodos(0, "wait", true);
    let aiodos = Arc::new(client(&server, Arc::new(KeyStore::in_memory())));
    let a = aiodos.clone();
    let worker = std::thread::spawn(move || a.connect(&mut |_| {}));
    // la demande attend l'utilisateur : l'état le dit, avec le code
    let mut st = aiodos.status();
    for _ in 0..100 {
        if st.code_to_show.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        st = aiodos.status();
    }
    assert_eq!((st.state.as_str(), st.code_to_show.as_deref()), ("pending", Some("482 197")));
    aiodos.cancel();
    let err = worker.join().unwrap().unwrap_err();
    assert!(err.is_cancelled(), "{err:?}");
    assert_eq!(aiodos.status().state, "needs_access");
}

#[test]
fn aiodos_absent_ou_autre_programme() {
    // rien n'écoute : pas lancé (sans fichier de découverte ni installation, « pas installé »)
    let opts = AiodosOptions { url: Some(format!("http://127.0.0.1:{}", common::closed_port())), system: false, ..AiodosOptions::arklight("0.2.0") };
    let aiodos = Aiodos::new(opts, Arc::new(KeyStore::in_memory()));
    let st = aiodos.status();
    assert_eq!((st.state.as_str(), st.code.as_deref()), ("not_installed", Some("unreachable")));
    // Ollama sur le port : route inconnue
    let other = FakeServer::start(|_| common::json(404, Value::Null));
    let aiodos = client(&other, Arc::new(KeyStore::in_memory()));
    assert_eq!(aiodos.status().state, "not_aiodos");
    // adresse non locale refusée sans rien envoyer
    let opts = AiodosOptions { url: Some("http://192.0.2.1:11435".into()), system: false, ..AiodosOptions::arklight("0.2.0") };
    let aiodos = Aiodos::new(opts, Arc::new(KeyStore::in_memory()));
    assert_eq!(aiodos.status().code.as_deref(), Some("non_local_url"));
}
