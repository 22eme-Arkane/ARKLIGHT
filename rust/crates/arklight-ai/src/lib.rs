//! Assistant IA d'ARKLIGHT (portage de `server/ai/`) : il comprend une demande, agit dans le projet
//! ouvert avec ses outils (chaque action est une modification [`arklight_core::document::Edit`],
//! donc annulable), puis répond.
//!
//! - [`assistant`] : consignes du modèle, conversation avec appels d'outils ;
//! - [`tools`] : définitions des outils et exécution sur le [`arklight_core::document::Document`] ;
//! - [`providers`] : fournisseurs (Ollama, AIODOS, serveurs compatibles OpenAI, Anthropic, OpenAI,
//!   Mistral, Gemini), conversation, listes de modèles ;
//! - [`aiodos`] : AIODOS, l'IA locale d'ARKANE (accès par code à 6 chiffres, preuve d'identité) ;
//! - [`ollama`] : état, démarrage, téléchargement des modèles et de l'installeur d'Ollama ;
//! - [`keys`] : coffre des clés API (chiffrées par Windows, DPAPI) ;
//! - [`http`] : petit client HTTP/1.1 bloquant (HTTPS par rustls).
//!
//! Pas encore porté : la reconnaissance vocale (`server/ai/voice.js`, whisper.cpp et ses modèles à
//! télécharger) ; l'interface peut garder la dictée du système en attendant.
//!
//! Utilisation (application de bureau) :
//!
//! ```no_run
//! use arklight_ai::{AiSettings, AskRequest, Assistant, Backend, DocumentWorkspace, KeyStore, NoHooks};
//! use std::sync::Arc;
//! # fn demo(doc: &mut arklight_core::document::Document) -> Result<(), arklight_ai::AiError> {
//! let keys = Arc::new(KeyStore::open("ai-keys.json"));
//! let backend = Backend::new(AiSettings::default(), keys, None);
//! let mut assistant = Assistant::new();
//! let mut hooks = NoHooks;
//! let mut ws = DocumentWorkspace { doc, hooks: &mut hooks };
//! let req = AskRequest { text: "Ajoute 4 Titan en arc derrière la comédienne".into(), lang: "fr".into(), ..Default::default() };
//! let out = assistant.ask(&mut ws, &backend, req, &mut |e| println!("{e:?}"))?;
//! println!("{}", out.reply);
//! # Ok(()) }
//! ```

pub mod aiodos;
pub mod assistant;
pub mod error;
pub mod http;
pub mod keys;
pub mod ollama;
pub mod providers;
pub mod tools;
mod util;

pub use aiodos::{Aiodos, AiodosOptions, AiodosStatus, Grant};
pub use assistant::{AskOutcome, AskRequest, Assistant, AssistantEvent, DocumentTask, DocumentWorkspace, SharedWorkspace, Workspace, system_prompt};
pub use error::AiError;
pub use http::Cancel;
pub use keys::KeyStore;
pub use providers::{AiSettings, AnsweredBy, Backend, ChatOptions, ChatReply, Message, ModelInfo, PROVIDERS, Provider, ToolCall};
pub use tools::{Changed, DmxState, NoHooks, ToolHooks, ToolRunner, tool_defs};
pub use util::{base64_decode, base64_encode, base64url_encode};
