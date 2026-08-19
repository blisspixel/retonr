use rewrite_types::Digest;

use crate::OutputContract;

const CANDIDATE_SCHEMA: &str = r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["candidates"],"properties":{"candidates":{"type":"array","minItems":1,"maxItems":16,"items":{"type":"object","additionalProperties":false,"required":["text"],"properties":{"text":{"type":"string"}}}}}}"#;
const CLAIM_SCHEMA: &str = r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["status","unit_id","text_digest","claims"],"properties":{"status":{"type":"string","enum":["complete","partial","abstained","failed"]},"unit_id":{"type":"string","minLength":1,"maxLength":128},"text_digest":{"type":"string","pattern":"^[0-9a-f]{64}$"},"claims":{"type":"array","maxItems":256,"items":{"type":"object","additionalProperties":false,"required":["claim_id","predicate_id","polarity","modality","condition_count","attributed","evidence_spans","confidence_ppm"],"properties":{"claim_id":{"type":"string","pattern":"^[0-9a-f]{64}$"},"subject_id":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$"},"predicate_id":{"type":"string","pattern":"^[0-9a-f]{64}$"},"object_id":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$"},"polarity":{"type":"string","enum":["affirmed","negated","unknown"]},"modality":{"type":"string","enum":["asserted","required","permitted","possible","conditional","unknown"]},"condition_count":{"type":"integer","minimum":0,"maximum":65535},"attributed":{"type":"boolean"},"evidence_spans":{"type":"array","minItems":1,"maxItems":16,"items":{"type":"object","additionalProperties":false,"required":["start","end"],"properties":{"start":{"type":"integer","minimum":0},"end":{"type":"integer","minimum":0}}}},"confidence_ppm":{"type":"integer","minimum":0,"maximum":1000000}}}}}}"#;

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

/// Returns the provider-neutral structured contract for claim evidence.
///
/// Runtime adapters must admit this exact digest before claim extraction.
/// Admission is not qualification, semantic proof, or an activation grant.
#[must_use]
pub fn claim_output_contract() -> OutputContract {
    OutputContract {
        schema_digest: Digest::sha256(CLAIM_SCHEMA.as_bytes()),
        schema_json: CLAIM_SCHEMA.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_and_claim_contracts_are_distinct_and_stable() {
        let candidate = candidate_output_contract();
        let claim = claim_output_contract();
        assert_ne!(candidate.schema_digest, claim.schema_digest);
        assert!(!claim.schema_json.contains("\"text\""));
        assert!(claim.schema_json.contains("text_digest"));
        assert!(claim.schema_json.contains("claim_id"));
        assert!(claim.schema_json.contains("additionalProperties\":false"));
        assert_eq!(
            candidate.schema_digest,
            Digest::sha256(CANDIDATE_SCHEMA.as_bytes())
        );
        assert_eq!(claim.schema_digest, Digest::sha256(CLAIM_SCHEMA.as_bytes()));
    }
}
