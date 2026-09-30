use rewrite_types::Digest;
use serde_json::Value;

use super::super::operation_contracts::operation_policy_test_support as test_support;
use super::*;
use crate::generation_qualification::*;
use crate::{ModelLicenseControlId, ModelPackageFoundationId};

#[path = "tests/decision.rs"]
mod decision;
#[path = "tests/invalidation.rs"]
mod invalidation;
#[path = "tests/selection.rs"]
mod selection;

struct RejectedFixture {
    base: test_support::Fixture,
    request_inputs: Vec<GenerationQualificationRequestProjectionEntryV1Input>,
    projection: GenerationQualificationRequestProjectionV1,
    foundation: ModelPackageFoundationId,
    license_control: ModelLicenseControlId,
    platform_input: GenerationQualificationPlatformEvidenceV1Input,
    platform: GenerationQualificationPlatformEvidenceV1,
    license_input: GenerationQualificationLicenseEvidenceV1Input,
    license: GenerationQualificationLicenseEvidenceV1,
    ledger: GenerationAttemptLedgerManifestV1,
    repeatability: GenerationRepeatabilityEvidenceManifestV1,
    resource: GenerationResourceEvidenceManifestV1,
    human: GenerationHumanAdjudicationEvidenceManifestV1,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
    receipt: GenerationQualificationOperationReceiptV1,
}

impl RejectedFixture {
    fn new() -> Self {
        let base = test_support::fixture();
        let request_inputs = base
            .attempts
            .iter()
            .enumerate()
            .map(
                |(index, attempt)| GenerationQualificationRequestProjectionEntryV1Input {
                    structured_completion_request_binding_id:
                        StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                            "qualification structured request {index}"
                        ))),
                    complete_input_byte_count: attempt.source_byte_count() + 100,
                    context_token_limit: base.policy.limits().maximum_context_tokens(),
                    output_token_limit: base.policy.limits().maximum_output_tokens(),
                },
            )
            .collect::<Vec<_>>();
        let projection_relations = projection_relations(&base);
        let projection =
            GenerationQualificationRequestProjectionV1::new(projection_relations, &request_inputs)
                .expect("request projection");
        let platform_input = GenerationQualificationPlatformEvidenceV1Input {
            status: GenerationQualificationPlatformStatusV1::Rejected,
            reason: GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied,
        };
        let platform = GenerationQualificationPlatformEvidenceV1::new(
            platform_relations(&base, &projection),
            platform_input,
        )
        .expect("platform evidence");
        let foundation = ModelPackageFoundationId::from_derived_digest(digest("foundation"));
        let license_control = ModelLicenseControlId::from_derived_digest(digest("license control"));
        let license_input = GenerationQualificationLicenseEvidenceV1Input {
            decision: GenerationQualificationLicenseDecisionV1::LocalUseOnly,
            reason: GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration,
        };
        let license = GenerationQualificationLicenseEvidenceV1::new(
            license_relations(&base, &projection, &foundation, &license_control),
            license_input,
        )
        .expect("license evidence");
        let scope = phase_scope(&base);
        let ledger = GenerationAttemptLedgerManifestV1::new(ledger_relations(&base))
            .expect("skipped ledger");
        let repeatability =
            GenerationRepeatabilityEvidenceManifestV1::new(repeatability_relations(&base))
                .expect("skipped repeatability");
        let resource = GenerationResourceEvidenceManifestV1::new(
            GenerationResourceEvidenceManifestV1Relations {
                scope,
                phase_policy_digest: &base.policy_input.resource_policy_digest,
                evidence_record_digests: &[],
                status: GenerationQualificationPhaseStatusV1::Skipped,
            },
        )
        .expect("skipped resource");
        let human = GenerationHumanAdjudicationEvidenceManifestV1::new(
            GenerationHumanAdjudicationEvidenceManifestV1Relations {
                scope,
                phase_policy_digest: &base.policy_input.human_adjudication_policy_digest,
                evidence_record_digests: &[],
                status: GenerationQualificationPhaseStatusV1::Skipped,
            },
        )
        .expect("skipped human adjudication");
        let receipt_input =
            receipt_input(GenerationQualificationOperationTerminalStatusV1::Completed);
        let receipt = GenerationQualificationOperationReceiptV1::new(
            GenerationQualificationOperationReceiptV1Relations {
                operation_policy: &base.policy,
                request_projection: &projection,
                platform_evidence: &platform,
                license_evidence: &license,
                attempt_ledger_manifest: &ledger,
                repeatability_manifest: &repeatability,
                resource_manifest: &resource,
                human_adjudication_manifest: &human,
            },
            receipt_input,
        )
        .expect("operation receipt");
        Self {
            base,
            request_inputs,
            projection,
            foundation,
            license_control,
            platform_input,
            platform,
            license_input,
            license,
            ledger,
            repeatability,
            resource,
            human,
            receipt_input,
            receipt,
        }
    }

    fn receipt_relations(&self) -> GenerationQualificationOperationReceiptV1Relations<'_> {
        GenerationQualificationOperationReceiptV1Relations {
            operation_policy: &self.base.policy,
            request_projection: &self.projection,
            platform_evidence: &self.platform,
            license_evidence: &self.license,
            attempt_ledger_manifest: &self.ledger,
            repeatability_manifest: &self.repeatability,
            resource_manifest: &self.resource,
            human_adjudication_manifest: &self.human,
        }
    }

    fn relations(&self) -> GenerationQualificationRecordV1Relations<'_> {
        GenerationQualificationRecordV1Relations {
            operation_receipt: &self.receipt,
            operation_receipt_relations: self.receipt_relations(),
            operation_receipt_input: self.receipt_input,
            operation_policy_relations: self.base.relations(),
            operation_policy_input: &self.base.policy_input,
            request_projection_relations: projection_relations(&self.base),
            request_projection_entry_inputs: &self.request_inputs,
            platform_evidence_relations: platform_relations(&self.base, &self.projection),
            platform_evidence_input: self.platform_input,
            license_evidence_relations: license_relations(
                &self.base,
                &self.projection,
                &self.foundation,
                &self.license_control,
            ),
            license_evidence_input: self.license_input,
            attempt_ledger_relations: ledger_relations(&self.base),
            repeatability_manifest_relations: repeatability_relations(&self.base),
            repeatability_result_relations: &[],
            resource_manifest_relations: GenerationResourceEvidenceManifestV1Relations {
                scope: phase_scope(&self.base),
                phase_policy_digest: &self.base.policy_input.resource_policy_digest,
                evidence_record_digests: &[],
                status: GenerationQualificationPhaseStatusV1::Skipped,
            },
            human_adjudication_manifest_relations:
                GenerationHumanAdjudicationEvidenceManifestV1Relations {
                    scope: phase_scope(&self.base),
                    phase_policy_digest: &self.base.policy_input.human_adjudication_policy_digest,
                    evidence_record_digests: &[],
                    status: GenerationQualificationPhaseStatusV1::Skipped,
                },
        }
    }
}

#[test]
fn exact_pretraffic_rejection_round_trips_with_compact_stable_identity() {
    let fixture = RejectedFixture::new();
    let record =
        GenerationQualificationRecordV1::new(&fixture.relations()).expect("qualification record");
    assert_eq!(record.schema_version(), 1);
    assert_eq!(record.status(), GenerationQualificationStatusV1::Rejected);
    assert_eq!(
        record.target_generation_system_id(),
        fixture.base.policy.target_generation_system_id()
    );
    assert_eq!(
        record.baseline_generation_system_id(),
        fixture.base.policy.baseline_generation_system_id()
    );
    assert_eq!(
        record.operation_policy_id(),
        fixture.base.policy.operation_policy_id()
    );
    assert_eq!(
        record.request_projection_id(),
        fixture.projection.request_projection_id()
    );
    assert_eq!(
        record.platform_evidence_id(),
        fixture.platform.platform_evidence_id()
    );
    assert_eq!(
        record.license_evidence_id(),
        fixture.license.license_evidence_id()
    );
    assert_eq!(
        record.operation_receipt_id(),
        fixture.receipt.operation_receipt_id()
    );
    record
        .validate_against(&fixture.relations())
        .expect("record revalidation");
    let bytes = serde_json::to_vec(&record).expect("qualification JSON");
    assert_eq!(
        GenerationQualificationRecordV1::from_json_bytes(&bytes, &fixture.relations())
            .expect("qualification decode"),
        record
    );
    assert_eq!(
        record.generation_qualification_id().digest().as_str(),
        "4a0fa1c7a6ae1212cbfaf54360790885537b6c8c4cbc7ddce4f81b1cb4b6d62d"
    );
    let value: Value = serde_json::from_slice(&bytes).expect("qualification value");
    assert_eq!(value.as_object().expect("object").len(), 9);
    for forbidden in [
        "generation_qualification_plan_id",
        "suite_manifest_id",
        "attempt_ledger_manifest_id",
        "candidate_judge_join_id",
    ] {
        assert!(value.get(forbidden).is_none());
    }
}

#[test]
fn decoder_is_strict_bounded_canonical_and_derives_status() {
    let fixture = RejectedFixture::new();
    let record =
        GenerationQualificationRecordV1::new(&fixture.relations()).expect("qualification record");
    let bytes = serde_json::to_vec(&record).expect("qualification JSON");
    assert_eq!(
        GenerationQualificationRecordV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES + 1],
            &fixture.relations(),
        ),
        Err(GenerationQualificationRecordV1Error::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationQualificationRecordV1::from_json_bytes(b"{", &fixture.relations()),
        Err(GenerationQualificationRecordV1Error::InvalidEncoding)
    );
    let mut unknown: Value = serde_json::from_slice(&bytes).expect("qualification value");
    unknown["manifest_id"] = Value::String(digest("forbidden").to_string());
    assert_eq!(
        decode_value(&unknown, &fixture.relations()),
        Err(GenerationQualificationRecordV1Error::InvalidEncoding)
    );
    let text = String::from_utf8(bytes.clone()).expect("UTF-8 qualification JSON");
    let duplicate = text.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_eq!(
        GenerationQualificationRecordV1::from_json_bytes(
            duplicate.as_bytes(),
            &fixture.relations(),
        ),
        Err(GenerationQualificationRecordV1Error::InvalidEncoding)
    );
    let spaced = format!(" {text}");
    assert_eq!(
        GenerationQualificationRecordV1::from_json_bytes(spaced.as_bytes(), &fixture.relations()),
        Err(GenerationQualificationRecordV1Error::NonCanonicalEncoding)
    );
    let mut wrong_status: Value = serde_json::from_slice(&bytes).expect("qualification value");
    wrong_status["status"] = Value::String("qualified".to_owned());
    assert_eq!(
        decode_value(&wrong_status, &fixture.relations()),
        Err(GenerationQualificationRecordV1Error::RelationshipMismatch)
    );
    let mut wrong_schema: Value = serde_json::from_slice(&bytes).expect("qualification value");
    wrong_schema["schema_version"] = Value::from(2);
    assert_eq!(
        decode_value(&wrong_schema, &fixture.relations()),
        Err(GenerationQualificationRecordV1Error::UnsupportedSchema)
    );
}

#[test]
fn noncompleted_receipt_cannot_produce_a_qualification_record() {
    let fixture = RejectedFixture::new();
    let cancelled_input =
        receipt_input(GenerationQualificationOperationTerminalStatusV1::Cancelled);
    let cancelled = GenerationQualificationOperationReceiptV1::new(
        fixture.receipt_relations(),
        cancelled_input,
    )
    .expect("cancelled receipt");
    let relations = GenerationQualificationRecordV1Relations {
        operation_receipt: &cancelled,
        operation_receipt_input: cancelled_input,
        ..fixture.relations()
    };
    assert_eq!(
        GenerationQualificationRecordV1::new(&relations),
        Err(GenerationQualificationRecordV1Error::IneligibleOperation)
    );
}

#[test]
fn independent_assessment_inputs_cannot_be_recovered_from_json() {
    let fixture = RejectedFixture::new();
    let relations = GenerationQualificationRecordV1Relations {
        platform_evidence_input: GenerationQualificationPlatformEvidenceV1Input {
            status: GenerationQualificationPlatformStatusV1::Supported,
            reason: GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu,
        },
        ..fixture.relations()
    };
    assert!(matches!(
        GenerationQualificationRecordV1::new(&relations),
        Err(GenerationQualificationRecordV1Error::InvalidOperationEvidence(_))
    ));
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn projection_relations(
    fixture: &test_support::Fixture,
) -> GenerationQualificationRequestProjectionV1Relations<'_> {
    GenerationQualificationRequestProjectionV1Relations {
        operation_policy: &fixture.policy,
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
        planned_attempts: &fixture.attempts,
    }
}

fn platform_relations<'a>(
    fixture: &'a test_support::Fixture,
    projection: &'a GenerationQualificationRequestProjectionV1,
) -> GenerationQualificationPlatformEvidenceV1Relations<'a> {
    GenerationQualificationPlatformEvidenceV1Relations {
        operation_policy: &fixture.policy,
        request_projection: projection,
        target_generation_system: &fixture.systems[0],
        target_generation_system_relations: fixture.system_fixture.relations(),
    }
}

fn license_relations<'a>(
    fixture: &'a test_support::Fixture,
    projection: &'a GenerationQualificationRequestProjectionV1,
    foundation: &'a ModelPackageFoundationId,
    license_control: &'a ModelLicenseControlId,
) -> GenerationQualificationLicenseEvidenceV1Relations<'a> {
    GenerationQualificationLicenseEvidenceV1Relations {
        operation_policy: &fixture.policy,
        request_projection: projection,
        target_generation_system: &fixture.systems[0],
        target_generation_system_relations: fixture.system_fixture.relations(),
        model_package_foundation_id: foundation,
        model_license_control_id: license_control,
    }
}

fn phase_scope(fixture: &test_support::Fixture) -> GenerationQualificationPhaseScopeV1<'_> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[0],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
    }
}

fn ledger_relations(
    fixture: &test_support::Fixture,
) -> GenerationAttemptLedgerManifestV1Relations<'_> {
    GenerationAttemptLedgerManifestV1Relations {
        scope: phase_scope(fixture),
        phase_policy_digest: &fixture.policy_input.attempt_ledger_policy_digest,
        planned_attempts: &fixture.attempts,
        attempt_records: &[],
        status: GenerationQualificationPhaseStatusV1::Skipped,
    }
}

fn repeatability_relations(
    fixture: &test_support::Fixture,
) -> GenerationRepeatabilityEvidenceManifestV1Relations<'_> {
    GenerationRepeatabilityEvidenceManifestV1Relations {
        scope: phase_scope(fixture),
        phase_policy_digest: &fixture.policy_input.repeatability_policy_digest,
        planned_attempts: &fixture.attempts,
        preregistered_repetitions: &fixture.repetitions,
        results: &[],
        status: GenerationQualificationPhaseStatusV1::Skipped,
    }
}

const fn receipt_input(
    terminal_status: GenerationQualificationOperationTerminalStatusV1,
) -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds: 1,
        peak_concurrent_attempts: 0,
        terminal_status,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    }
}

fn decode_value(
    value: &Value,
    relations: &GenerationQualificationRecordV1Relations<'_>,
) -> Result<GenerationQualificationRecordV1, GenerationQualificationRecordV1Error> {
    GenerationQualificationRecordV1::from_json_bytes(
        &serde_json::to_vec(value).expect("encoded qualification value"),
        relations,
    )
}
