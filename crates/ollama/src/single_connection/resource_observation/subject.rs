use std::{fmt, sync::Arc};

pub(super) struct RetainedSessionSubjectMarker;

pub(in crate::single_connection) struct OllamaRetainedSessionSubject {
    pub(super) marker: Arc<RetainedSessionSubjectMarker>,
}

impl OllamaRetainedSessionSubject {
    pub(in crate::single_connection) fn new() -> Self {
        Self {
            marker: Arc::new(RetainedSessionSubjectMarker),
        }
    }
}

/// Ephemeral identity of one exact retained Ollama session.
///
/// The token is backed by process-local shared ownership and cannot be
/// constructed or serialized by callers. Clones retain the same exact subject.
/// It proves only identity equality for one ephemeral retained session. It does
/// not prove execution, provider-resource accuracy, model use, runtime state,
/// persistence, or qualification.
///
/// ```compile_fail
/// fn serialize_token(value: &rewrite_ollama::OllamaRetainedSessionSubjectToken) {
///     let _json = serde_json::to_string(value).unwrap();
/// }
/// ```
///
/// ```compile_fail
/// fn construct_token() -> rewrite_ollama::OllamaRetainedSessionSubjectToken {
///     rewrite_ollama::OllamaRetainedSessionSubjectToken {}
/// }
/// ```
#[derive(Clone)]
pub struct OllamaRetainedSessionSubjectToken {
    pub(super) marker: Arc<RetainedSessionSubjectMarker>,
}

impl fmt::Debug for OllamaRetainedSessionSubjectToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OllamaRetainedSessionSubjectToken")
            .field("content", &"redacted")
            .finish()
    }
}
