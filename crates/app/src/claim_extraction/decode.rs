use rewrite_types::{
    ClaimEvidence, ClaimEvidenceSet, ClaimExtractionStatus, ClaimModality, ClaimPolarity, Digest,
    ExtractorManifest, RewriteUnitId, SourceSpan,
};
use serde::Deserialize;

use super::ClaimPairExtractionError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSet {
    status: ClaimExtractionStatus,
    unit_id: RewriteUnitId,
    text_digest: Digest,
    claims: Vec<WireClaim>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireClaim {
    claim_id: Digest,
    subject_id: Option<Digest>,
    predicate_id: Digest,
    object_id: Option<Digest>,
    polarity: ClaimPolarity,
    modality: ClaimModality,
    condition_count: u16,
    attributed: bool,
    evidence_spans: Vec<SourceSpan>,
    confidence_ppm: u32,
}

pub(super) fn decode_claim_set(
    output_json: &str,
    text: &str,
    unit_id: &RewriteUnitId,
    manifest: &ExtractorManifest,
    minimum_confidence_ppm: u32,
) -> Result<ClaimEvidenceSet, ClaimPairExtractionError> {
    let wire: WireSet =
        serde_json::from_str(output_json).map_err(|_| ClaimPairExtractionError::MalformedOutput)?;
    if &wire.unit_id != unit_id || wire.text_digest != Digest::sha256(text.as_bytes()) {
        return Err(ClaimPairExtractionError::MalformedOutput);
    }
    let claims = wire
        .claims
        .into_iter()
        .map(|claim| {
            ClaimEvidence::from_canonical(
                claim.claim_id,
                claim.subject_id,
                claim.predicate_id,
                claim.object_id,
                claim.polarity,
                claim.modality,
                claim.condition_count,
                claim.attributed,
                claim.evidence_spans,
                claim.confidence_ppm,
            )
            .map_err(ClaimPairExtractionError::InvalidEvidence)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ClaimEvidenceSet::new(
        manifest.extractor_id(),
        manifest.extractor_version(),
        manifest.manifest_digest(),
        wire.status,
        minimum_confidence_ppm,
        unit_id.clone(),
        text,
        claims,
    )
    .map_err(ClaimPairExtractionError::InvalidEvidence)
}

#[cfg(test)]
mod tests {
    use rewrite_types::{DocumentId, ExtractorManifest, SourceSpan};

    use super::*;
    use crate::claim_extraction::{CLAIM_PAIR_OPERATION_ID, CLAIM_PAIR_PROMPT_TEMPLATE};

    fn manifest() -> ExtractorManifest {
        ExtractorManifest::new(
            "literal-claims",
            "1.0.0",
            Digest::sha256(b"subject-policy"),
            Digest::sha256(CLAIM_PAIR_PROMPT_TEMPLATE.as_bytes()),
            rewrite_inference::claim_output_contract().schema_digest,
            Digest::sha256(CLAIM_PAIR_OPERATION_ID.as_bytes()),
            Digest::sha256(b"confidence-policy"),
            Digest::sha256(b"language-policy"),
        )
        .expect("valid extractor")
    }

    fn unit() -> RewriteUnitId {
        RewriteUnitId::new(&DocumentId::from_digest(&Digest::sha256(b"doc")), 0)
    }

    #[test]
    fn accepts_complete_redacted_payload_and_rejects_unknown_fields() {
        let text = "Pay 10 now.";
        let unit_id = unit();
        let claim_id = Digest::sha256(b"claim");
        let payload = serde_json::json!({
            "status": "complete",
            "unit_id": unit_id.as_str(),
            "text_digest": Digest::sha256(text.as_bytes()),
            "claims": [{
                "claim_id": claim_id,
                "subject_id": null,
                "predicate_id": Digest::sha256(b"pay"),
                "object_id": null,
                "polarity": "affirmed",
                "modality": "asserted",
                "condition_count": 0,
                "attributed": false,
                "evidence_spans": [{"start": 0, "end": text.len()}],
                "confidence_ppm": 990_000
            }]
        });
        let set = decode_claim_set(
            &payload.to_string(),
            text,
            &unit_id,
            &manifest(),
            900_000,
        )
        .expect("valid claim payload");
        assert_eq!(set.extraction_status(), ClaimExtractionStatus::Complete);
        assert_eq!(set.claims().len(), 1);
        assert_eq!(set.claims()[0].confidence_ppm(), 990_000);
        assert_eq!(
            set.claims()[0].evidence_spans(),
            [SourceSpan::new(0, text.len()).expect("span")]
        );

        let mut unknown = payload;
        unknown["authority"] = serde_json::json!(true);
        assert_eq!(
            decode_claim_set(&unknown.to_string(), text, &unit_id, &manifest(), 900_000),
            Err(ClaimPairExtractionError::MalformedOutput)
        );
    }

    #[test]
    fn rejects_text_digest_and_unit_mismatch() {
        let text = "Pay 10 now.";
        let unit_id = unit();
        let payload = serde_json::json!({
            "status": "complete",
            "unit_id": unit_id.as_str(),
            "text_digest": Digest::sha256(b"other"),
            "claims": []
        });
        assert_eq!(
            decode_claim_set(&payload.to_string(), text, &unit_id, &manifest(), 0),
            Err(ClaimPairExtractionError::MalformedOutput)
        );
    }
}
