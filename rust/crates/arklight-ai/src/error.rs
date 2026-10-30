//! Erreur de l'assistant : un message pour l'utilisateur, un code stable pour l'interface.

use crate::http::HttpError;

/// Une erreur de l'assistant, d'un fournisseur d'IA ou d'AIODOS.
#[derive(Debug, Clone, PartialEq)]
pub struct AiError {
    /// message montré tel quel (français ou anglais)
    pub message: String,
    /// code stable : « cancelled », « timeout », « unreachable », et ceux d'AIODOS
    /// (« access_denied », « needs_access », « tools_unavailable »…)
    pub code: Option<String>,
    /// statut HTTP de la réponse d'erreur
    pub status: Option<u16>,
    /// conseil (AIODOS)
    pub hint: Option<String>,
    /// réessayer peut marcher
    pub retryable: bool,
    /// secondes à attendre avant de réessayer (Retry-After)
    pub retry_after: Option<f64>,
}

impl AiError {
    pub fn new(message: impl Into<String>) -> AiError {
        AiError { message: message.into(), code: None, status: None, hint: None, retryable: false, retry_after: None }
    }

    pub fn with_code(code: &str, message: impl Into<String>) -> AiError {
        AiError { code: Some(code.to_owned()), ..AiError::new(message) }
    }

    /// « Arrêté » (bouton Arrêter).
    pub fn cancelled() -> AiError {
        AiError::with_code("cancelled", "Arrêté")
    }

    pub fn is_cancelled(&self) -> bool {
        self.code.as_deref() == Some("cancelled")
    }

    pub fn code(&self) -> &str {
        self.code.as_deref().unwrap_or("")
    }

    /// Échec de transport vers un fournisseur (Ollama, API en ligne) : les messages de
    /// `providers.js`.
    pub(crate) fn from_provider(e: HttpError) -> AiError {
        match e {
            HttpError::Cancelled => AiError::cancelled(),
            HttpError::Timeout => AiError::with_code("timeout", "Délai dépassé : le modèle ne répond pas"),
            HttpError::Refused(host) => AiError::with_code("unreachable", format!("Connexion refusée ({host}) : le serveur ne tourne pas")),
            other => AiError::with_code("unreachable", other.to_string()),
        }
    }
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AiError {}
