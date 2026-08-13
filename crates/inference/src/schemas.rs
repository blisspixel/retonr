use rewrite_types::Digest;

use crate::OutputContract;

const CANDIDATE_SCHEMA: &str = r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["candidates"],"properties":{"candidates":{"type":"array","minItems":1,"maxItems":16,"items":{"type":"object","additionalProperties":false,"required":["text"],"properties":{"text":{"type":"string"}}}}}}"#;

/// Returns the provider-neutral structured contract for rewrite candidates.
///
/// Runtime adapters may admit this exact contract at the transport boundary.
/// Admission does not qualify any runtime and artifact tuple for generation.
#[must_use]
pub fn candidate_output_contract() -> OutputContract {
    OutputContract {
        schema_digest: Digest::sha256(CANDIDATE_SCHEMA.as_bytes()),
        schema_json: CANDIDATE_SCHEMA.to_owned(),
    }
}
