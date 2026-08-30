use rewrite_model::{CandidateOutputCeilingsV1, GenerationQualificationOperationLimitsV1};
use rewrite_types::{
    CancellationToken, Digest, DocumentId, RewriteUnit, RewriteUnitId, SourceSpan,
};

use super::super::*;
use super::support::{Fixture, SOURCE, common_limits};

#[test]
fn invalid_utf8_and_zero_unit_bom_fail_before_request_identity() {
    let mut invalid = Fixture::new(b"valid prefix\xff", Vec::new());
    let lease = invalid.source.acquire();
    assert!(matches!(
        invalid
            .builder
            .derive_preplanning(&lease, 7, common_limits(), &CancellationToken::new(),),
        Err(GenerationQualificationRequestBuildError::SourceEncodingUnsupported)
    ));

    let mut bom_only = Fixture::new(b"\xef\xbb\xbf", Vec::new());
    let lease = bom_only.source.acquire();
    assert!(matches!(
        bom_only
            .builder
            .derive_preplanning(&lease, 7, common_limits(), &CancellationToken::new(),),
        Err(GenerationQualificationRequestBuildError::UnitPolicyMismatch)
    ));
}

#[test]
fn multiple_or_partial_units_are_rejected_by_the_frozen_unit_policy() {
    let document = DocumentId::from_digest(&Digest::sha256(b"unit fixture"));
    let unit = |ordinal, start, end, text: &str| RewriteUnit {
        id: RewriteUnitId::new(&document, ordinal),
        source_span: SourceSpan::new(start, end).expect("span"),
        text: text.to_owned(),
    };
    assert_eq!(
        validation::require_single_unit_for_test(&[]),
        Err(GenerationQualificationRequestBuildError::UnitPolicyMismatch)
    );
    assert!(matches!(
        validation::require_single_unit_for_test(&[unit(0, 0, 1, "a"), unit(1, 1, 2, "b"),]),
        Err(GenerationQualificationRequestBuildError::UnitPolicyMismatch)
    ));
    assert!(matches!(
        validation::require_single_unit_for_test(&[unit(0, 1, 2, "a")]),
        Err(GenerationQualificationRequestBuildError::UnitPolicyMismatch)
    ));
}

#[test]
fn prompt_and_source_ceilings_fail_independently() {
    let output = CandidateOutputCeilingsV1::new(1, 4_096, 4_096).expect("ceilings");
    let limits = |source, complete| {
        GenerationQualificationOperationLimitsV1::new(
            source,
            complete,
            2_048,
            256,
            output.maximum_envelope_bytes(),
            1,
            output.maximum_candidate_bytes(),
            output.maximum_aggregate_candidate_bytes(),
            2,
            1,
            10_000,
        )
        .expect("bounded limits")
    };
    let mut prompt = Fixture::new(SOURCE, Vec::new());
    let lease = prompt.source.acquire();
    assert!(matches!(
        prompt.builder.derive_preplanning(
            &lease,
            7,
            limits(SOURCE.len() as u64, SOURCE.len() as u64),
            &CancellationToken::new(),
        ),
        Err(GenerationQualificationRequestBuildError::PromptFailed)
    ));

    let mut source = Fixture::new(SOURCE, Vec::new());
    let lease = source.source.acquire();
    assert!(matches!(
        source.builder.derive_preplanning(
            &lease,
            7,
            limits((SOURCE.len() - 1) as u64, SOURCE.len() as u64),
            &CancellationToken::new(),
        ),
        Err(GenerationQualificationRequestBuildError::SourceMismatch)
    ));
}

#[test]
fn original_source_count_may_exceed_masked_and_complete_input() {
    let term = "x".repeat(200);
    let source = std::iter::repeat_n(term.as_str(), 24)
        .collect::<Vec<_>>()
        .join(" ");
    let fixture = Fixture::new(source.as_bytes(), vec![term]);
    let material = validation::build_material_for_seed(
        &fixture.builder,
        7,
        common_limits(),
        source.as_bytes(),
    )
    .expect("masked request");
    assert!(material.protection_plan.masked_source().len() < source.len());
    assert!(material.generation_request.input.len() < source.len());
    assert_eq!(
        material.generation_request.source_byte_count,
        source.len() as u64
    );
}

#[test]
fn deserialized_invalid_common_limits_are_rejected_before_derivation() {
    let mut wire = serde_json::to_value(common_limits()).expect("serialize limits");
    wire["maximum_context_tokens"] = serde_json::Value::from(u32::MAX);
    wire["maximum_output_tokens"] = serde_json::Value::from(u32::MAX);
    wire["maximum_predeclared_attempts"] = serde_json::Value::from(0);
    wire["maximum_elapsed_milliseconds"] = serde_json::Value::from(0);
    assert!(serde_json::from_value::<GenerationQualificationOperationLimitsV1>(wire).is_err());
}

#[test]
fn cancellation_remains_distinct_from_source_drift() {
    let mut fixture = Fixture::new(SOURCE, Vec::new());
    let lease = fixture.source.acquire();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        fixture
            .builder
            .derive_preplanning(&lease, 7, common_limits(), &cancellation),
        Err(GenerationQualificationRequestBuildError::Cancelled)
    ));
}
