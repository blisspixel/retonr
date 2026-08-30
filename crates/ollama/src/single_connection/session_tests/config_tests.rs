use crate::{OllamaLimits, OllamaRetainedStreamSessionConfig};

use super::fixture::binding;

#[test]
fn config_rejects_duplicate_and_unbounded_session_identity() {
    let endpoint =
        crate::OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback endpoint");
    let duplicate = OllamaRetainedStreamSessionConfig::new(
        endpoint.clone(),
        vec![binding(), binding()],
        OllamaLimits::default(),
        1024,
    )
    .expect_err("duplicate bindings fail");
    assert_eq!(duplicate.code, "duplicate_session_binding");
    let empty = OllamaRetainedStreamSessionConfig::new(
        endpoint,
        Vec::new(),
        OllamaLimits::default(),
        usize::MAX,
    )
    .expect_err("empty bindings fail");
    assert_eq!(empty.code, "invalid_session_bindings");
}
