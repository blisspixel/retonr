use super::{
    ClaimEvidence, ClaimEvidenceError, ClaimEvidenceSet, ClaimExtractionStatus, ClaimModality,
    ClaimPolarity,
};
use crate::{Digest, DocumentId, RewriteUnitId, SourceSpan};

fn claim(id: &[u8], spans: Vec<SourceSpan>) -> ClaimEvidence {
    ClaimEvidence::new(
        Digest::sha256(id),
        Some(Digest::sha256(b"subject")),
        Digest::sha256(b"predicate"),
        Some(Digest::sha256(b"object")),
        ClaimPolarity::Affirmed,
        ClaimModality::Required,
        1,
        true,
        spans,
        0.98,
    )
    .expect("valid claim fixture")
}

fn unit(text: &str) -> RewriteUnitId {
    RewriteUnitId::new(
        &DocumentId::from_digest(&Digest::sha256(text.as_bytes())),
        0,
    )
}

#[test]
fn canonicalizes_and_binds_redacted_claims() {
    let text = "Ada must ship today.";
    let set = ClaimEvidenceSet::new(
        "fixture-extractor",
        "1.0.0",
        Digest::sha256(b"complete manifest"),
        ClaimExtractionStatus::Complete,
        900_000,
        unit(text),
        text,
        vec![
            claim(
                b"b",
                vec![SourceSpan::new(4, text.len()).expect("valid span")],
            ),
            claim(b"a", vec![SourceSpan::new(0, 3).expect("valid span")]),
        ],
    )
    .expect("valid evidence set");
    let mut expected = [Digest::sha256(b"a"), Digest::sha256(b"b")];
    expected.sort_unstable_by(|left, right| left.as_str().cmp(right.as_str()));
    assert_eq!(set.claims()[0].claim_id(), &expected[0]);
    assert_eq!(set.text_digest(), &Digest::sha256(text.as_bytes()));
    let encoded = serde_json::to_string(&set).expect("evidence serializes");
    assert!(!encoded.contains(text));
    assert!(!encoded.contains("Ada"));
}

#[test]
fn rejects_invalid_confidence_spans_and_duplicates() {
    let span = SourceSpan::new(0, 1).expect("valid span");
    assert_eq!(
        ClaimEvidence::new(
            Digest::sha256(b"c"),
            None,
            Digest::sha256(b"p"),
            None,
            ClaimPolarity::Unknown,
            ClaimModality::Unknown,
            0,
            false,
            vec![span],
            f32::NAN
        ),
        Err(ClaimEvidenceError::InvalidConfidence)
    );
    assert_eq!(
        ClaimEvidence::new(
            Digest::sha256(b"c"),
            None,
            Digest::sha256(b"p"),
            None,
            ClaimPolarity::Unknown,
            ClaimModality::Unknown,
            0,
            false,
            vec![span, span],
            1.0
        ),
        Err(ClaimEvidenceError::DuplicateSpan)
    );
    let text = "one";
    assert_eq!(
        ClaimEvidenceSet::new(
            "fixture",
            "1",
            Digest::sha256(b"m"),
            ClaimExtractionStatus::Complete,
            0,
            unit(text),
            text,
            vec![claim(b"same", vec![span]); 2]
        ),
        Err(ClaimEvidenceError::DuplicateClaim)
    );
}

#[test]
fn canonical_digest_changes_with_effective_identity_and_status() {
    let text = "x";
    let make = |manifest: &[u8], status| {
        ClaimEvidenceSet::new(
            "fixture",
            "1",
            Digest::sha256(manifest),
            status,
            800_000,
            unit(text),
            text,
            vec![claim(
                b"claim",
                vec![SourceSpan::new(0, 1).expect("valid span")],
            )],
        )
        .expect("valid evidence fixture")
    };
    assert_ne!(
        make(b"one", ClaimExtractionStatus::Complete).evidence_digest(),
        make(b"two", ClaimExtractionStatus::Complete).evidence_digest()
    );
    assert_ne!(
        make(b"one", ClaimExtractionStatus::Complete).evidence_digest(),
        make(b"one", ClaimExtractionStatus::Partial).evidence_digest()
    );
}

#[test]
fn canonicalizes_confidence_to_exact_parts_per_million() {
    let text = "x";
    let make = |confidence| {
        ClaimEvidenceSet::new(
            "fixture",
            "1",
            Digest::sha256(b"manifest"),
            ClaimExtractionStatus::Complete,
            900_000,
            unit(text),
            text,
            vec![
                ClaimEvidence::new(
                    Digest::sha256(b"claim"),
                    None,
                    Digest::sha256(b"predicate"),
                    None,
                    ClaimPolarity::Affirmed,
                    ClaimModality::Asserted,
                    0,
                    false,
                    vec![SourceSpan::new(0, 1).expect("valid span")],
                    confidence,
                )
                .expect("valid confidence"),
            ],
        )
        .expect("valid evidence set")
    };
    let threshold = make(0.9);
    assert_eq!(threshold.claims()[0].confidence_ppm(), 900_000);
    assert_eq!(make(0.0).evidence_digest(), make(-0.0).evidence_digest());
}
