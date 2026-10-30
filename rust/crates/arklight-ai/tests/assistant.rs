//! L'assistant entier face à un faux serveur compatible OpenAI : la demande, les appels d'outils
//! appliqués au projet (annulables), la réponse ; modèle sans « réflexion » (Ollama), erreur,
//! arrêt.

mod common;

use arklight_ai::{AiSettings, AskRequest, Assistant, AssistantEvent, Backend, DocumentWorkspace, KeyStore, Message, NoHooks, SharedWorkspace};
use common::{FakeServer, Reply, Req, demo, json};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn backend(provider: &str, url: &str) -> Backend {
    let mut s = AiSettings::default();
    s.set_provider(provider);
    s.set_model("qwen3.5:9b");
    s.set_url(provider, url);
    Backend::new(s, Arc::new(KeyStore::in_memory()), None)
}

fn call(id: &str, name: &str, args: Value) -> Value {
    json!({ "id": id, "type": "function", "function": { "name": name, "arguments": args.to_string() } })
}

#[test]
fn demande_complete() {
    let turns = Arc::new(AtomicUsize::new(0));
    let server = FakeServer::start(move |_r: &Req| {
        let reply = match turns.fetch_add(1, Ordering::SeqCst) {
            0 => json!({ "content": "Je place les contres.", "tool_calls": [call("c1", "add_fixtures", json!({ "type": "astera.titan", "count": 2, "positions": [[-1, 0, -3], [1, 0, -3]], "name": "Contre" }))] }),
            1 => json!({ "content": null, "tool_calls": [call("c2", "set_environment", json!({ "haze": 50 })), call("c3", "update_fixtures", json!({ "target": { "ids": ["inexistant"] } }))] }),
            _ => json!({ "content": "J'ai ajouté 2 Titan derrière et réglé la brume à 50 %." }),
        };
        json(200, json!({ "choices": [{ "message": reply }] }))
    });
    let mut doc = demo();
    let start = serde_json::to_string(&doc.project).unwrap();
    let n0 = doc.project.fixtures.len();
    let mut hooks = NoHooks;
    let mut assistant = Assistant::new();
    let mut events = Vec::new();
    let out = {
        let mut ws = DocumentWorkspace { doc: &mut doc, hooks: &mut hooks };
        let req = AskRequest { text: "Deux Titan en contre, et plus de brume".into(), images: vec!["data:image/jpeg;base64,/9j/4AAQ".into()], image_aspect: None, lang: "fr".into() };
        assistant.ask(&mut ws, &backend("local", &format!("{}/v1", server.url)), req, &mut |e| events.push(e)).unwrap()
    };
    assert_eq!(out.reply, "J'ai ajouté 2 Titan derrière et réglé la brume à 50 %.");
    assert_eq!(out.last_added.len(), 2);
    assert!(out.changed.fixtures && out.changed.environment && !out.changed.objects);
    assert_eq!(doc.project.fixtures.len(), n0 + 2);
    assert_eq!(doc.project.fixtures[n0].name, "Contre 1");
    assert_eq!(doc.project.environment.haze, 0.5);

    // ce que l'interface a vu
    let tools: Vec<(&str, bool, &str)> = events
        .iter()
        .filter_map(|e| match e {
            AssistantEvent::Tool { name, ok, summary, .. } => Some((name.as_str(), *ok, summary.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        tools,
        vec![
            ("add_fixtures", true, "2 × Astera Titan Tube ajouté(s)"),
            ("set_environment", true, "Ambiance réglée"),
            ("update_fixtures", false, "Aucun projecteur trouvé : appelle get_scene pour connaître les ids."),
        ]
    );
    assert!(events.contains(&AssistantEvent::Note { text: "Je place les contres.".into() }));
    assert!(matches!(events.first(), Some(AssistantEvent::Thinking { round: 0, loading: false })));
    assert!(matches!(events.last(), Some(AssistantEvent::Reply { .. })));

    // la conversation : la photo part avec sa demande, les résultats des outils sont relus
    let reqs = server.requests();
    assert_eq!(reqs.len(), 3);
    let first = reqs[0].json();
    let system = first["messages"][0]["content"].as_str().unwrap();
    assert!(system.starts_with("Tu es l'assistant intégré d'ARKLIGHT"));
    assert!(system.contains("PROJET OUVERT : « demo-night » — Séquence 1 › Plan 1 — 15 projecteur(s), 68 objet(s)."));
    assert!(system.contains("astera.titan — Astera Titan Tube"));
    assert_eq!(first["messages"][1]["content"][1]["image_url"]["url"], "data:image/jpeg;base64,/9j/4AAQ");
    assert_eq!(first["tools"].as_array().unwrap().len(), 13);
    let third = reqs[2].json();
    let msgs = third["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1 + 1 + 2 + 3, "système, demande, (appel, résultat), (appels, 2 résultats)");
    assert!(third["messages"][0]["content"].as_str().unwrap().contains("17 projecteur(s)"), "le résumé du projet suit");
    let tool_msgs: Vec<&Value> = msgs.iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(tool_msgs[0]["tool_call_id"], "c1");
    assert!(tool_msgs[0]["content"].as_str().unwrap().starts_with("{\"added\":["));
    assert_eq!(tool_msgs[1]["content"], "{\"environment\":{\"haze\":0.5}}");
    assert_eq!(tool_msgs[2]["content"], "{\"error\":\"Aucun projecteur trouvé : appelle get_scene pour connaître les ids.\"}");

    // une demande suivante : la photo n'est plus renvoyée
    assert!(matches!(&assistant.messages()[0], Message::User { images, .. } if images.len() == 1));
    // tout s'annule
    while doc.undo() {}
    assert_eq!(serde_json::to_string(&doc.project).unwrap(), start);
}

#[test]
fn sur_un_fil_de_travail() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Backend>();
    send_sync::<arklight_ai::Aiodos>();
    send_sync::<KeyStore>();
    fn send<T: Send>() {}
    send::<Assistant>();

    let turns = Arc::new(AtomicUsize::new(0));
    let server = FakeServer::start(move |_r: &Req| {
        let reply = match turns.fetch_add(1, Ordering::SeqCst) {
            0 => json!({ "tool_calls": [call("c1", "set_environment", json!({ "haze": 80, "cyc": false }))] }),
            _ => json!({ "content": "Brume à 80 %." }),
        };
        json(200, json!({ "choices": [{ "message": reply }] }))
    });
    // le projet reste à l'interface ; l'assistant ne le verrouille que le temps de chaque passage
    let doc = Arc::new(std::sync::Mutex::new(demo()));
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s = seen.clone();
    let mut ws = SharedWorkspace { doc: doc.clone(), hooks: NoHooks, on_change: Some(Box::new(move |c| s.lock().unwrap().push(c))) };
    let b = backend("local", &format!("{}/v1", server.url));
    let worker = std::thread::spawn(move || {
        let mut assistant = Assistant::new();
        assistant.ask(&mut ws, &b, AskRequest { text: "Plus de brume".into(), lang: "en".into(), ..AskRequest::default() }, &mut |_| {})
    });
    let out = worker.join().unwrap().unwrap();
    assert_eq!(out.reply, "Brume à 80 %.");
    assert!(out.changed.environment);
    assert_eq!(doc.lock().unwrap().project.environment.haze, 0.8);
    assert_eq!(seen.lock().unwrap().len(), 1);
    // interface en anglais : le prompt le dit
    assert!(server.requests()[0].json()["messages"][0]["content"].as_str().unwrap().contains("interface en anglais"));
}

#[test]
fn modele_sans_reflexion_puis_reponse() {
    let server = FakeServer::start(|r: &Req| match r.route() {
        "/api/ps" => json(200, json!({ "models": [{ "name": "qwen3.5:9b" }] })),
        "/api/chat" if r.json().get("think").is_some() => json(400, json!({ "error": "\"qwen3.5:9b\" does not support thinking" })),
        "/api/chat" => json(200, json!({ "message": { "content": "Bonjour !" } })),
        _ => json(404, Value::Null),
    });
    let mut doc = demo();
    let mut hooks = NoHooks;
    let mut ws = DocumentWorkspace { doc: &mut doc, hooks: &mut hooks };
    let mut assistant = Assistant::new();
    let out = assistant.ask(&mut ws, &backend("ollama", &server.url), AskRequest { text: "Bonjour".into(), lang: "fr".into(), ..AskRequest::default() }, &mut |_| {}).unwrap();
    assert_eq!(out.reply, "Bonjour !");
    let chats: Vec<Req> = server.requests().into_iter().filter(|r| r.route() == "/api/chat").collect();
    assert_eq!(chats.len(), 2, "réessayé sans « think »");
    assert_eq!(assistant.messages().len(), 2);
}

#[test]
fn erreur_et_arret() {
    let server = FakeServer::start(|r: &Req| match r.route() {
        "/v1/chat/completions" if r.json()["messages"][1]["content"] == "lent" => Reply::Slow(Duration::from_secs(5), Box::new(json(200, json!({ "choices": [{ "message": { "content": "trop tard" } }] })))),
        _ => json(500, json!({ "error": "panne du moteur" })),
    });
    let b = backend("local", &format!("{}/v1", server.url));
    let mut doc = demo();
    let mut hooks = NoHooks;
    let mut assistant = Assistant::new();
    {
        let mut ws = DocumentWorkspace { doc: &mut doc, hooks: &mut hooks };
        let err = assistant.ask(&mut ws, &b, AskRequest { text: "panne".into(), ..AskRequest::default() }, &mut |_| {}).unwrap_err();
        assert_eq!(err.message, "500 — panne du moteur");
    }
    assert!(assistant.messages().is_empty(), "la demande sans réponse est retirée");
    assert!(!assistant.busy());

    // « Arrêter » depuis un autre fil
    let stopper = assistant.stopper();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        stopper.cancel();
    });
    let mut ws = DocumentWorkspace { doc: &mut doc, hooks: &mut hooks };
    let started = std::time::Instant::now();
    let err = assistant.ask(&mut ws, &b, AskRequest { text: "lent".into(), ..AskRequest::default() }, &mut |_| {}).unwrap_err();
    t.join().unwrap();
    assert!(err.is_cancelled(), "{err:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(assistant.messages().is_empty());
}
