//! Adversarial and frozen-vector coverage for phase evidence contracts.

use rewrite_types::Digest;
use serde_json::Value;

use super::super::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureCategoryV1,
    CandidateGenerationAttemptFailurePhaseV1, CandidateGenerationAttemptFailureV1Input,
    CandidateGenerationAttemptRecordV1, CandidateOutputCeilingsV1, GenerationCaseManifestV1,
    GenerationCaseManifestV1Input, GenerationClusterRecordV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationRepetitionRecordV1, GenerationRequestBindingId, GenerationSuiteManifestV1,
    GenerationSystemRecordV1, MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES, MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
    MAX_GENERATION_RETAINED_INPUT_BYTES, PlannedCandidateAttemptV1, PlannedCandidateAttemptV1Input,
    PlannedCandidateAttemptV1Relations,
};
use super::*;
use crate::ArtifactId;

struct Fixture {
    cluster: GenerationClusterRecordV1,
    case: GenerationCaseManifestV1,
    suite: GenerationSuiteManifestV1,
    repetitions: Vec<GenerationRepetitionRecordV1>,
    systems: Vec<GenerationSystemRecordV1>,
    planned: Vec<PlannedCandidateAttemptV1>,
    plan: GenerationQualificationPlanV1,
    target_records: Vec<CandidateGenerationAttemptRecordV1>,
    baseline_records: Vec<CandidateGenerationAttemptRecordV1>,
    ledger_policy: Digest,
}

impl Fixture {
    fn scope(&self) -> GenerationQualificationPhaseScopeV1<'_> {
        GenerationQualificationPhaseScopeV1 {
            generation_system: &self.systems[0],
            qualification_plan: &self.plan,
            suite: &self.suite,
        }
    }

    fn ledger_relations<'a>(
        &'a self,
        records: &'a [CandidateGenerationAttemptRecordV1],
        status: GenerationQualificationPhaseStatusV1,
    ) -> GenerationAttemptLedgerManifestV1Relations<'a> {
        GenerationAttemptLedgerManifestV1Relations {
            scope: self.scope(),
            phase_policy_digest: &self.ledger_policy,
            planned_attempts: &self.planned,
            attempt_records: records,
            status,
        }
    }
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn policy(label: &str) -> Digest {
    digest(label)
}

fn case_fixture() -> (GenerationClusterRecordV1, GenerationCaseManifestV1) {
    let cluster =
        GenerationClusterRecordV1::new("phase-case", digest("cluster policy")).expect("cluster");
    let source_digest = digest("phase source");
    let case = GenerationCaseManifestV1::new(
        &cluster,
        GenerationCaseManifestV1Input {
            case_key: "phase-case".to_owned(),
            source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
            source_digest,
            source_byte_count: 12,
            case_contract_digest: digest("case contract"),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
        },
    )
    .expect("case");
    (cluster, case)
}

fn repetition_and_system_fixtures(
    suite: &GenerationSuiteManifestV1,
) -> (
    Vec<GenerationRepetitionRecordV1>,
    Vec<GenerationSystemRecordV1>,
) {
    let repetitions = (0..2)
        .map(|ordinal| {
            GenerationRepetitionRecordV1::new(
                suite,
                ordinal,
                digest(&format!("repetition {ordinal}")),
            )
            .expect("repetition")
        })
        .collect::<Vec<_>>();
    let support = super::super::generation_system::test_support::fixture(false);
    let mut systems = ["target", "baseline"]
        .map(|label| {
            let mut input = support.input();
            input.strategy_digest = digest(label);
            GenerationSystemRecordV1::new(support.relations(), input).expect("system")
        })
        .to_vec();
    systems.sort_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    (repetitions, systems)
}

fn planned_fixtures(
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    cluster: &GenerationClusterRecordV1,
    repetitions: &[GenerationRepetitionRecordV1],
    systems: &[GenerationSystemRecordV1],
) -> Vec<PlannedCandidateAttemptV1> {
    // First occurrence order intentionally differs from repetition ordinal order.
    [(1_usize, 0_usize), (0, 1), (0, 0), (1, 1)]
        .into_iter()
        .enumerate()
        .map(|(ordinal, (system, repetition))| {
            PlannedCandidateAttemptV1::new(
                PlannedCandidateAttemptV1Relations {
                    suite,
                    case,
                    cluster,
                    repetition: &repetitions[repetition],
                    generation_system: &systems[system],
                },
                PlannedCandidateAttemptV1Input {
                    attempt_ordinal: u32::try_from(ordinal).expect("ordinal"),
                    declared_seed: u64::try_from(ordinal).expect("seed"),
                    grounded_request_digest: digest(&format!("request {ordinal}")),
                    generation_request_binding_id: GenerationRequestBindingId::from_derived_digest(
                        digest(&format!("binding {ordinal}")),
                    ),
                    candidate_output_contract_digest: digest("output contract"),
                    output_ceilings: CandidateOutputCeilingsV1::new(1, 256, 256).expect("ceilings"),
                },
            )
            .expect("planned attempt")
        })
        .collect()
}

fn failed_attempt(attempt: &PlannedCandidateAttemptV1) -> CandidateGenerationAttemptRecordV1 {
    CandidateGenerationAttemptRecordV1::failed(
        attempt,
        None,
        CandidateGenerationAttemptFailureV1Input {
            failure_phase: CandidateGenerationAttemptFailurePhaseV1::RequestCompilation,
            failure_category: CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
            traffic_observed: false,
            output_observed: false,
            cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1::NotRequired,
        },
    )
    .expect("failed attempt")
}

fn fixture() -> Fixture {
    let (cluster, case) = case_fixture();
    let suite =
        GenerationSuiteManifestV1::new(digest("suite protocol"), std::slice::from_ref(&case))
            .expect("suite");
    let (repetitions, systems) = repetition_and_system_fixtures(&suite);
    let planned = planned_fixtures(&suite, &case, &cluster, &repetitions, &systems);
    let plan = GenerationQualificationPlanV1::new(
        &suite,
        &repetitions,
        &systems,
        &planned,
        GenerationQualificationPlanV1Input {
            limits: GenerationQualificationPlanLimitsV1::new(
                1,
                4,
                MAX_GENERATION_RETAINED_INPUT_BYTES,
                MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES,
                MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
                MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
            )
            .expect("limits"),
            selection_policy_digest: digest("selection policy"),
            failure_policy_digest: digest("failure policy"),
        },
    )
    .expect("plan");
    let target_records = vec![failed_attempt(&planned[1])];
    let baseline_records = vec![failed_attempt(&planned[0])];
    Fixture {
        cluster,
        case,
        suite,
        repetitions,
        systems,
        planned,
        plan,
        target_records,
        baseline_records,
        ledger_policy: policy("attempt ledger"),
    }
}

fn assert_frozen_ids(
    ledger: &GenerationAttemptLedgerManifestV1,
    result: &GenerationRepeatabilityResultRecordV1,
    repeat: &GenerationRepeatabilityEvidenceManifestV1,
    resource: &GenerationResourceEvidenceManifestV1,
    human: &GenerationHumanAdjudicationEvidenceManifestV1,
) {
    assert_eq!(
        [
            ledger.attempt_ledger_manifest_id().digest().as_str(),
            result.repeatability_result_id().digest().as_str(),
            repeat
                .repeatability_evidence_manifest_id()
                .digest()
                .as_str(),
            resource.resource_evidence_manifest_id().digest().as_str(),
            human
                .human_adjudication_evidence_manifest_id()
                .digest()
                .as_str(),
        ],
        [
            "ffdb6095fedd6bd46a5c1de26bb6a1eebd90517df7f785a6e291d2510506c22b",
            "f6d59db1264b13fc1aa699b0cf7475db7edad064b9bacb5b505412d38d3d954d",
            "d415aeb4ce7bae67bdbf89da4f2c9586e46f2e4a4f593ec01fd4b17693046fea",
            "0d0d8babba7f5adbe83341e39b1e6105669421d87ad13d758098c28d57530e1a",
            "cd440f4631fd3c0a9fcb1061c51882241e451f47f376dd51cd6875657c29e04d",
        ]
    );
}

fn assert_public_projections(
    ledger: &GenerationAttemptLedgerManifestV1,
    result: &GenerationRepeatabilityResultRecordV1,
    repeat: &GenerationRepeatabilityEvidenceManifestV1,
    resource: &GenerationResourceEvidenceManifestV1,
    human: &GenerationHumanAdjudicationEvidenceManifestV1,
) {
    assert_eq!(ledger.generation_system_id(), result.generation_system_id());
    assert_eq!(
        ledger.generation_qualification_plan_id(),
        result.generation_qualification_plan_id()
    );
    assert_eq!(ledger.suite_manifest_id(), result.suite_manifest_id());
    assert_eq!(
        result.attempt_ledger_manifest_id(),
        ledger.attempt_ledger_manifest_id()
    );
    assert_eq!(
        result.attempt_ledger_root_digest(),
        ledger.evidence_root_digest()
    );
    assert!(result.candidate_generation_receipt_set_id().is_none());
    assert!(result.candidate_deterministic_evaluation_id().is_none());
    assert!(result.candidate_judge_join_id().is_none());
    assert_eq!(
        result.terminal_evidence_digest(),
        &digest("candidate generation terminal evidence")
    );
    assert_eq!(repeat.generation_system_id(), ledger.generation_system_id());
    assert_eq!(
        repeat.generation_qualification_plan_id(),
        ledger.generation_qualification_plan_id()
    );
    assert_eq!(repeat.suite_manifest_id(), ledger.suite_manifest_id());
    assert_eq!(repeat.phase_policy_digest(), &policy("repeatability"));
    assert_eq!(repeat.evidence_item_count(), 1);
    assert_eq!(
        repeat.status(),
        GenerationQualificationPhaseStatusV1::Failed
    );
    for manifest in [
        (
            resource.generation_system_id(),
            resource.generation_qualification_plan_id(),
            resource.suite_manifest_id(),
            resource.phase_policy_digest(),
            resource.evidence_item_count(),
            resource.status(),
        ),
        (
            human.generation_system_id(),
            human.generation_qualification_plan_id(),
            human.suite_manifest_id(),
            human.phase_policy_digest(),
            human.evidence_item_count(),
            human.status(),
        ),
    ] {
        assert_eq!(manifest.0, ledger.generation_system_id());
        assert_eq!(manifest.1, ledger.generation_qualification_plan_id());
        assert_eq!(manifest.2, ledger.suite_manifest_id());
        assert!(!manifest.3.as_str().is_empty());
        assert!(manifest.4 > 0);
        assert_ne!(manifest.5, GenerationQualificationPhaseStatusV1::Skipped);
    }
    assert!(!format!("{result:?}{repeat:?}{resource:?}{human:?}").is_empty());
}

fn assert_manifest_round_trips(
    ledger: &GenerationAttemptLedgerManifestV1,
    ledger_relations: GenerationAttemptLedgerManifestV1Relations<'_>,
    repeat: &GenerationRepeatabilityEvidenceManifestV1,
    repeat_relations: GenerationRepeatabilityEvidenceManifestV1Relations<'_>,
) {
    assert_eq!(
        &GenerationAttemptLedgerManifestV1::from_json_bytes(
            &serde_json::to_vec(ledger).expect("ledger JSON"),
            ledger_relations,
        )
        .expect("ledger decode"),
        ledger
    );
    assert_eq!(
        &GenerationRepeatabilityEvidenceManifestV1::from_json_bytes(
            &serde_json::to_vec(repeat).expect("repeat JSON"),
            repeat_relations,
        )
        .expect("repeat decode"),
        repeat
    );
    assert_strict_json(ledger, "evidence_root_digest", |bytes| {
        GenerationAttemptLedgerManifestV1::from_json_bytes(bytes, ledger_relations).map(drop)
    });
    assert_strict_json(repeat, "evidence_root_digest", |bytes| {
        GenerationRepeatabilityEvidenceManifestV1::from_json_bytes(bytes, repeat_relations)
            .map(drop)
    });
}

fn assert_record_round_trips(
    result: &GenerationRepeatabilityResultRecordV1,
    result_relations: GenerationRepeatabilityResultRecordV1Relations<'_>,
    resource: &GenerationResourceEvidenceManifestV1,
    resource_relations: GenerationResourceEvidenceManifestV1Relations<'_>,
    human: &GenerationHumanAdjudicationEvidenceManifestV1,
    human_relations: GenerationHumanAdjudicationEvidenceManifestV1Relations<'_>,
) {
    assert_eq!(
        &GenerationRepeatabilityResultRecordV1::from_json_bytes(
            &serde_json::to_vec(result).expect("result JSON"),
            result_relations,
        )
        .expect("result decode"),
        result
    );
    assert_eq!(
        &GenerationResourceEvidenceManifestV1::from_json_bytes(
            &serde_json::to_vec(resource).expect("resource JSON"),
            resource_relations,
        )
        .expect("resource decode"),
        resource
    );
    assert_eq!(
        &GenerationHumanAdjudicationEvidenceManifestV1::from_json_bytes(
            &serde_json::to_vec(human).expect("human JSON"),
            human_relations,
        )
        .expect("human decode"),
        human
    );
    assert_strict_json(result, "terminal_evidence_digest", |bytes| {
        GenerationRepeatabilityResultRecordV1::from_json_bytes(bytes, result_relations).map(drop)
    });
    assert_strict_json(resource, "evidence_root_digest", |bytes| {
        GenerationResourceEvidenceManifestV1::from_json_bytes(bytes, resource_relations).map(drop)
    });
    assert_strict_json(human, "evidence_root_digest", |bytes| {
        GenerationHumanAdjudicationEvidenceManifestV1::from_json_bytes(bytes, human_relations)
            .map(drop)
    });
}

fn assert_strict_json<T: serde::Serialize>(
    value: &T,
    relationship_field: &str,
    decode: impl Fn(&[u8]) -> Result<(), GenerationQualificationPhaseEvidenceError>,
) {
    let canonical = serde_json::to_vec(value).expect("canonical JSON");
    decode(&canonical).expect("canonical decode");
    let text = String::from_utf8(canonical.clone()).expect("UTF-8 JSON");
    let first_comma = text.find(',').expect("first field");
    let second_comma = text[first_comma + 1..]
        .find(',')
        .map(|index| index + first_comma + 1)
        .expect("second field");
    let reordered = format!(
        "{{{},{},{}",
        &text[first_comma + 1..second_comma],
        &text[1..first_comma],
        &text[second_comma + 1..]
    );
    assert_eq!(
        decode(reordered.as_bytes()),
        Err(GenerationQualificationPhaseEvidenceError::NonCanonicalEncoding)
    );
    let duplicate = text.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_eq!(
        decode(duplicate.as_bytes()),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );
    let mut unknown = canonical.clone();
    unknown.pop();
    unknown.extend_from_slice(b",\"unknown\":0}");
    assert_eq!(
        decode(&unknown),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );
    let mut trailing = canonical.clone();
    trailing.push(b' ');
    assert_eq!(
        decode(&trailing),
        Err(GenerationQualificationPhaseEvidenceError::NonCanonicalEncoding)
    );
    let mut unsupported: Value = serde_json::from_slice(&canonical).expect("JSON value");
    unsupported["schema_version"] = Value::from(2);
    assert_eq!(
        decode(&serde_json::to_vec(&unsupported).expect("unsupported JSON")),
        Err(GenerationQualificationPhaseEvidenceError::UnsupportedSchema)
    );
    let mut substituted: Value = serde_json::from_slice(&canonical).expect("JSON value");
    substituted[relationship_field] = Value::String(digest("substitution").as_str().to_owned());
    assert_eq!(
        decode(&serde_json::to_vec(&substituted).expect("substituted JSON")),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
}

mod contracts;
mod denial_records;
