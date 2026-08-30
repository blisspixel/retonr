use rewrite_types::Digest;

use super::*;
use crate::ArtifactId;

mod adversarial;
mod attempt_chain;
mod bounds;
mod candidate_judge;
mod codec;
mod deterministic_evaluation;
mod managed_cleanup;
mod phase_evidence_full;

#[derive(Clone)]
struct Fixture {
    cluster: GenerationClusterRecordV1,
    cases: Vec<GenerationCaseManifestV1>,
    suite: GenerationSuiteManifestV1,
    repetition: GenerationRepetitionRecordV1,
    repetitions: Vec<GenerationRepetitionRecordV1>,
    systems: Vec<GenerationSystemRecordV1>,
    attempts: Vec<PlannedCandidateAttemptV1>,
    plan: GenerationQualificationPlanV1,
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn case(
    cluster: &GenerationClusterRecordV1,
    key: impl Into<String>,
    source_label: &str,
) -> GenerationCaseManifestV1 {
    let source_digest = digest(source_label);
    GenerationCaseManifestV1::new(
        cluster,
        GenerationCaseManifestV1Input {
            case_key: key.into(),
            source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
            source_digest,
            source_byte_count: source_label.len() as u64,
            case_contract_digest: digest("case contract"),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
        },
    )
    .expect("case fixture")
}

fn system_records(count: usize) -> Vec<GenerationSystemRecordV1> {
    let mut values = (0..count)
        .map(|index| {
            let fixture = super::generation_system::test_support::fixture(false);
            let mut input = fixture.input();
            input.strategy_digest = digest(&format!("system {index}"));
            GenerationSystemRecordV1::new(fixture.relations(), input).expect("system fixture")
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    values
}

fn attempt_records(
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    systems: &[GenerationSystemRecordV1],
    count: usize,
) -> Vec<PlannedCandidateAttemptV1> {
    (0..count)
        .map(|index| {
            attempt_record(
                suite,
                case,
                cluster,
                repetition,
                &systems[index % systems.len()],
                index,
            )
        })
        .collect()
}

fn attempt_record(
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    system: &GenerationSystemRecordV1,
    index: usize,
) -> PlannedCandidateAttemptV1 {
    PlannedCandidateAttemptV1::new(
        PlannedCandidateAttemptV1Relations {
            suite,
            case,
            cluster,
            repetition,
            generation_system: system,
        },
        PlannedCandidateAttemptV1Input {
            attempt_ordinal: u32::try_from(index).expect("attempt ordinal"),
            declared_seed: u64::try_from(index).expect("declared seed"),
            grounded_request_digest: digest(&format!("grounded request {index}")),
            generation_request_binding_id: GenerationRequestBindingId::from_derived_digest(digest(
                &format!("request binding {index}"),
            )),
            candidate_output_contract_digest: digest("candidate output contract"),
            output_ceilings: CandidateOutputCeilingsV1::new(2, 1_024, 2_048)
                .expect("candidate ceilings"),
        },
    )
    .expect("planned attempt fixture")
}

fn limits(maximum_attempts: u32) -> GenerationQualificationPlanLimitsV1 {
    GenerationQualificationPlanLimitsV1::new(
        MAX_GENERATION_CANDIDATES_PER_COMPLETION,
        maximum_attempts,
        MAX_GENERATION_RETAINED_INPUT_BYTES,
        MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES,
        MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
        MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    )
    .expect("limits fixture")
}

fn fixture() -> Fixture {
    let cluster = GenerationClusterRecordV1::new("editorial-core", digest("cluster policy"))
        .expect("cluster fixture");
    let cases = vec![
        case(&cluster, "active-voice", "source one"),
        case(&cluster, "nominalization", "source two"),
    ];
    let suite = GenerationSuiteManifestV1::new(digest("protocol"), &cases).expect("suite fixture");
    let repetition = GenerationRepetitionRecordV1::new(&suite, 0, digest("repetition policy"))
        .expect("repetition fixture");
    let repetitions = vec![repetition.clone()];
    let systems = system_records(2);
    let attempts = attempt_records(&suite, &cases[0], &cluster, &repetition, &systems, 4);
    let plan = GenerationQualificationPlanV1::new(
        &suite,
        &repetitions,
        &systems,
        &attempts,
        GenerationQualificationPlanV1Input {
            limits: limits(4),
            selection_policy_digest: digest("selection policy"),
            failure_policy_digest: digest("failure policy"),
        },
    )
    .expect("plan fixture");
    Fixture {
        cluster,
        cases,
        suite,
        repetition,
        repetitions,
        systems,
        attempts,
        plan,
    }
}

#[test]
fn records_expose_exact_bounded_relationships() {
    let fixture = fixture();
    assert_eq!(fixture.cluster.schema_version(), 1);
    assert_eq!(fixture.cluster.cluster_key(), "editorial-core");
    assert_eq!(
        fixture.cluster.cluster_policy_digest(),
        &digest("cluster policy")
    );

    let first = &fixture.cases[0];
    assert_eq!(first.schema_version(), 1);
    assert_eq!(first.case_key(), "active-voice");
    assert_eq!(first.cluster_id(), fixture.cluster.cluster_id());
    assert_eq!(first.source_artifact_id().digest(), first.source_digest());
    assert_eq!(first.source_byte_count(), 10);
    assert_eq!(first.case_contract_digest(), &digest("case contract"));
    assert_eq!(first.language_digest(), &digest("language"));
    assert_eq!(first.mode_digest(), &digest("mode"));
    assert_eq!(first.format_digest(), &digest("format"));

    assert_eq!(fixture.suite.schema_version(), 1);
    assert_eq!(fixture.suite.protocol_digest(), &digest("protocol"));
    assert_eq!(fixture.suite.case_ids()[0], *first.case_id());
    assert_eq!(fixture.repetition.schema_version(), 1);
    assert_eq!(
        fixture.repetition.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );
    assert_eq!(fixture.repetition.repetition_ordinal(), 0);
    assert_eq!(
        fixture.repetition.repetition_policy_digest(),
        &digest("repetition policy")
    );

    assert_eq!(fixture.plan.schema_version(), 1);
    assert_eq!(
        fixture.plan.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );
    assert_eq!(fixture.plan.generation_system_ids().len(), 2);
    assert_eq!(fixture.plan.planned_attempt_ids().len(), 4);
    assert_eq!(fixture.plan.limits(), limits(4));
    assert_eq!(
        fixture.plan.selection_policy_digest(),
        &digest("selection policy")
    );
    assert_eq!(
        fixture.plan.failure_policy_digest(),
        &digest("failure policy")
    );
    assert_eq!(
        fixture.plan.retry_policy(),
        GenerationQualificationRetryPolicyV1::NoRetry
    );
}

#[test]
fn typed_owner_bindings_are_inert_and_round_trip() {
    macro_rules! check {
        ($type:ty, $label:literal) => {{
            let value = <$type>::from_derived_digest(digest($label));
            assert_eq!(value.digest(), &digest($label));
            let encoded = serde_json::to_vec(&value).expect("ID encodes");
            assert_eq!(
                serde_json::from_slice::<$type>(&encoded).expect("ID decodes"),
                value
            );
        }};
    }
    check!(RuntimeAdmissionJoinId, "admission");
    check!(ManagedGenerationPathId, "path");
    check!(FrozenExternalComponentSetId, "frozen");
    check!(ManagedOllamaEffectiveRuntimeStateJoinId, "live join");
    check!(GenerationRequestBindingId, "request");
    check!(StructuredCompletionRequestBindingId, "wire request");
    check!(OllamaRetainedSessionResponseId, "response");
    check!(ManagedOllamaGenerationBracketObservationV1Id, "bracket");
}

#[test]
fn reserved_portable_ids_are_strict_transparent_values() {
    macro_rules! check {
        ($type:ident, $label:literal) => {{
            let value = $type(digest($label));
            let encoded = serde_json::to_vec(&value).expect("reserved ID encodes");
            assert_eq!(
                serde_json::from_slice::<$type>(&encoded).expect("reserved ID decodes"),
                value
            );
        }};
    }
    check!(GenerationSystemId, "system");
    check!(PlannedCandidateAttemptId, "planned");
    check!(CandidateGenerationAttemptPrecursorId, "precursor");
    check!(
        ManagedOllamaCandidateGenerationEvidenceV2Id,
        "managed evidence"
    );
    check!(CandidateGenerationCleanupId, "cleanup");
    check!(CandidateGenerationEvidenceBundleId, "bundle");
    check!(CandidateGenerationEvidenceBundleReadbackId, "readback");
    check!(CandidateEvidenceId, "candidate");
    check!(CandidateGenerationReceiptId, "receipt");
    check!(CandidateGenerationAttemptRecordId, "attempt record");
    check!(CandidateSelectionPolicyId, "selection policy");
    check!(CandidateGenerationReceiptSetId, "receipt set");
    check!(CandidateReceiptPairSetId, "receipt pair set");
    check!(
        CandidateDeterministicEvaluationId,
        "deterministic evaluation"
    );
}

#[test]
fn debug_is_redacted_to_ids_counts_and_statuses() {
    let fixture = fixture();
    let debug_values = [
        format!("{:?}", fixture.cluster),
        format!("{:?}", fixture.cases[0]),
        format!("{:?}", fixture.suite),
        format!("{:?}", fixture.repetition),
        format!("{:?}", fixture.plan),
    ];
    for debug in debug_values {
        for secret in [
            "editorial-core",
            "active-voice",
            digest("source one").as_str(),
            digest("selection policy").as_str(),
            digest("failure policy").as_str(),
        ] {
            assert!(!debug.contains(secret));
        }
    }
}

#[test]
fn identity_domains_and_known_vectors_are_stable() {
    let fixture = fixture();
    assert_eq!(
        GENERATION_CLUSTER_ID_DOMAIN,
        b"retonr:generation-cluster:v1\0"
    );
    assert_eq!(
        GENERATION_CASE_ID_DOMAIN,
        b"retonr:generation-case-manifest:v1\0"
    );
    assert_eq!(
        GENERATION_SUITE_MANIFEST_ID_DOMAIN,
        b"retonr:generation-suite-manifest:v1\0"
    );
    assert_eq!(
        GENERATION_REPETITION_ID_DOMAIN,
        b"retonr:generation-repetition:v1\0"
    );
    assert_eq!(
        GENERATION_QUALIFICATION_PLAN_ID_DOMAIN,
        b"retonr:generation-qualification-plan:v1\0"
    );
    assert_eq!(
        GENERATION_SYSTEM_ID_DOMAIN,
        b"retonr:generation-system-record:v1\0"
    );
    assert_eq!(
        PLANNED_CANDIDATE_ATTEMPT_ID_DOMAIN,
        b"retonr:planned-candidate-attempt:v1\0"
    );
    assert_eq!(
        CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_ID_DOMAIN,
        b"retonr:candidate-generation-attempt-precursor:v1\0"
    );
    assert_eq!(
        MANAGED_OLLAMA_CANDIDATE_EVIDENCE_ID_DOMAIN,
        b"retonr:managed-ollama-candidate-generation-evidence:v2\0"
    );
    assert_eq!(
        CANDIDATE_GENERATION_CLEANUP_ID_DOMAIN,
        b"retonr:candidate-generation-cleanup:v1\0"
    );
    assert_eq!(
        CANDIDATE_GENERATION_EVIDENCE_BUNDLE_ID_DOMAIN,
        b"retonr:candidate-generation-evidence-bundle:v1\0"
    );
    assert_eq!(
        CANDIDATE_GENERATION_EVIDENCE_READBACK_ID_DOMAIN,
        b"retonr:candidate-generation-evidence-readback:v1\0"
    );
    assert_eq!(
        CANDIDATE_EVIDENCE_ID_DOMAIN,
        b"retonr:candidate-evidence:v1\0"
    );
    assert_eq!(
        CANDIDATE_GENERATION_RECEIPT_ID_DOMAIN,
        b"retonr:candidate-generation-receipt:v1\0"
    );
    assert_eq!(
        CANDIDATE_GENERATION_ATTEMPT_RECORD_ID_DOMAIN,
        b"retonr:candidate-generation-attempt-record:v1\0"
    );
    assert_eq!(
        CANDIDATE_SELECTION_POLICY_ID_DOMAIN,
        b"retonr:candidate-selection-policy:v1\0"
    );
    assert_eq!(
        CANDIDATE_GENERATION_RECEIPT_SET_ID_DOMAIN,
        b"retonr:candidate-generation-receipt-set:v1\0"
    );
    assert_eq!(
        fixture.cluster.cluster_id().digest().as_str(),
        "eef935d72dc1aafe5f1407770f311f18e0a7751b71a186c4da47821660a7157d"
    );
    assert_eq!(
        fixture.cases[0].case_id().digest().as_str(),
        "3b4ad277ea742acbd5886530b73126af0d9b3bbab311565891c0dd9d4bade91a"
    );
    assert_eq!(
        fixture.suite.suite_manifest_id().digest().as_str(),
        "55ad76890ab06591572fc02d1376d669c9911dbcf5cac024c3378cd3a9ea2c5c"
    );
    assert_eq!(
        fixture.repetition.repetition_id().digest().as_str(),
        "4ce3b4f8a6feb10c7fb1a808b0a655034689ecfe52bed8fda7e3e7076a09ca2d"
    );
    assert_eq!(
        fixture.plan.qualification_plan_id().digest().as_str(),
        "c54265844c3ddb76353c8b6ea0d3ce5da26006be2eaa3449dd46e7ffd14e7374"
    );
}
