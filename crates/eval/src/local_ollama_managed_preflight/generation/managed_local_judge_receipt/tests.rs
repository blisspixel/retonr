use rewrite_model::{
    CandidateJudgeChoiceV1, CandidateJudgeObservationBatchV1, CandidateJudgeObservationV1,
    CandidateJudgeResponseAggregateV1, GenerationSystemRecordV1,
    ManagedLocalJudgeReceiptRecordV1Input, ManagedOllamaEffectiveRuntimeStateJoinId,
    OllamaRetainedSessionResponseId,
};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    RetainedTcpConnectionEvidence, RetainedTcpConnectionEvidenceInput,
    TcpConnectionAttributionKind, TcpConnectionSharingLimitation,
};
use rewrite_types::{CancellationToken, Digest};

use super::{
    ManagedLocalJudgeReceipt, ManagedLocalJudgeReceiptCompiler,
    ManagedLocalJudgeReceiptCompilerError, ManagedLocalJudgeReceiptRelationship,
    ReleasedReceiptAuthority, compile_record_from_authority, derive_preflight_observer_binding,
    map_authority_error,
};
use crate::candidate_judge_preparation::tests::{prepare, ready_config};
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::local_ollama_managed_preflight::generation::managed_schedule_runner::{
    ManagedJudgeScheduleAuthorityFailures, ManagedJudgeScheduleExecutionAuthorityError,
};
use crate::local_ollama_managed_preflight::report::managed_preflight_outcome_fixture;
use crate::{
    CandidateJudgePreparationOutcome, CandidateJudgeRunnerHandoff,
    LocalOllamaManagedPreflightOutcome,
};

struct FakeReleasedAuthority<'records> {
    judge_system: &'records GenerationSystemRecordV1,
    portable_relationship: bool,
    attempt_count: u64,
    preflight_observer_binding_digest: Digest,
    input: ManagedLocalJudgeReceiptRecordV1Input,
}

trait AmbiguousIfClone<A> {
    fn marker() {}
}

impl<T: ?Sized> AmbiguousIfClone<()> for T {}
impl<T: Clone> AmbiguousIfClone<u8> for T {}

trait AmbiguousIfSerialize<A> {
    fn marker() {}
}

impl<T: ?Sized> AmbiguousIfSerialize<()> for T {}
impl<T: ?Sized + serde::Serialize> AmbiguousIfSerialize<u8> for T {}

type Receipt = ManagedLocalJudgeReceipt<'static, 'static, 'static, 'static>;

impl ReleasedReceiptAuthority for FakeReleasedAuthority<'_> {
    fn exact_portable_relationship(&self, _eval: &CandidateJudgeRunnerHandoff<'_>) -> bool {
        self.portable_relationship
    }

    fn judge_system(&self) -> &GenerationSystemRecordV1 {
        self.judge_system
    }

    fn attempt_count(&self) -> u64 {
        self.attempt_count
    }

    fn preflight_observer_binding_digest(&self) -> &Digest {
        &self.preflight_observer_binding_digest
    }

    fn receipt_input(
        &self,
        managed_preflight_digest: Digest,
    ) -> ManagedLocalJudgeReceiptRecordV1Input {
        let mut input = self.input.clone();
        input.managed_preflight_digest = managed_preflight_digest;
        input
    }
}

#[test]
fn exact_owner_derived_fields_compile_content_free_receipt() {
    let fixture = Fixture::judge_pair();
    let handoff = handoff(&fixture, "receipt-compiler");
    let (responses, observations) = portable_outputs(&handoff);
    let managed_preflight = managed_preflight_outcome_fixture();
    let authority = authority(&handoff, &managed_preflight);

    let record = compile_record_from_authority(
        &handoff,
        &responses,
        &observations,
        &managed_preflight,
        &authority,
    )
    .expect("durable receipt record");

    assert_eq!(
        record.attempt_count(),
        handoff.judge_schedule().entry_count()
    );
    assert_eq!(
        record.managed_preflight_digest(),
        &managed_preflight.report().binding_digest
    );
    assert_eq!(
        record.retained_session_preflight_digest(),
        &authority.input.retained_session_preflight_digest
    );
    assert_eq!(
        record.residency_receipt_aggregate_digest(),
        &authority.input.residency_receipt_aggregate_digest
    );
    assert_eq!(
        record.process_observation_aggregate_digest(),
        &authority.input.process_observation_aggregate_digest
    );
    assert_eq!(
        record.native_load_observation_aggregate_digest(),
        &authority.input.native_load_observation_aggregate_digest
    );
    assert_eq!(
        record.connection_observation_aggregate_digest(),
        &authority.input.connection_observation_aggregate_digest
    );
    assert_eq!(
        record.effective_runtime_state_observation_aggregate_digest(),
        &authority
            .input
            .effective_runtime_state_observation_aggregate_digest
    );
    assert_eq!(
        record.judge_effective_runtime_state_join_id(),
        &authority.input.judge_effective_runtime_state_join_id
    );
    assert_eq!(record.first_response_ordinal(), 8);
    assert_eq!(record.last_response_ordinal(), 25);

    let encoded = serde_json::to_string(&record).expect("receipt JSON");
    let debug = format!("{record:?}");
    for content in [
        "Acme 42 needs polish.",
        "Acme, 42 needs polish!",
        "Acme 42 needs polish?",
        "selected candidate receipt-compiler-a 1",
        "selected candidate receipt-compiler-b 1",
        "prompt",
        "rationale",
    ] {
        assert!(!encoded.contains(content));
        assert!(!debug.contains(content));
    }
}

#[test]
fn portable_count_preflight_and_record_failures_are_closed() {
    let fixture = Fixture::judge_pair();
    let handoff = handoff(&fixture, "receipt-failures");
    let (responses, observations) = portable_outputs(&handoff);
    let managed_preflight = managed_preflight_outcome_fixture();
    let mut authority = authority(&handoff, &managed_preflight);

    authority.portable_relationship = false;
    assert_relationship(
        &compile_record_from_authority(
            &handoff,
            &responses,
            &observations,
            &managed_preflight,
            &authority,
        ),
        ManagedLocalJudgeReceiptRelationship::PortableJudge,
    );
    authority.portable_relationship = true;
    authority.attempt_count += 1;
    assert_relationship(
        &compile_record_from_authority(
            &handoff,
            &responses,
            &observations,
            &managed_preflight,
            &authority,
        ),
        ManagedLocalJudgeReceiptRelationship::AttemptCount,
    );
    authority.attempt_count -= 1;
    authority.preflight_observer_binding_digest = Digest::sha256(b"foreign preflight observer");
    assert_relationship(
        &compile_record_from_authority(
            &handoff,
            &responses,
            &observations,
            &managed_preflight,
            &authority,
        ),
        ManagedLocalJudgeReceiptRelationship::ManagedPreflight,
    );
    authority.preflight_observer_binding_digest =
        derive_preflight_observer_binding(handoff.judge_schedule(), managed_preflight.report())
            .expect("observer binding");
    authority.input.judge_runtime_installation_generation = 0;
    assert!(matches!(
        compile_record_from_authority(
            &handoff,
            &responses,
            &observations,
            &managed_preflight,
            &authority,
        ),
        Err(ManagedLocalJudgeReceiptCompilerError::Portable(_))
    ));
}

#[test]
fn preflight_observer_binding_is_exact_ordered_and_schedule_sensitive() {
    let fixture = Fixture::judge_pair();
    let handoff = handoff(&fixture, "receipt-preflight");
    let managed_preflight = managed_preflight_outcome_fixture();
    let expected =
        derive_preflight_observer_binding(handoff.judge_schedule(), managed_preflight.report())
            .expect("complete preflight binding");
    // This freezes the app framing over the exact schedule ID, three process
    // witnesses, native-load ID, and ordered initial-plus-seven connection span.
    assert_eq!(
        expected.as_str(),
        "9cc64f5b430c241eb02b57d2a94a31da08787b0631ebf8fa1483c1cd762f0e40"
    );

    let mut incomplete = managed_preflight.report().clone();
    incomplete.connection_observations.pop();
    assert_eq!(
        derive_preflight_observer_binding(handoff.judge_schedule(), &incomplete),
        None
    );

    let mut extra = managed_preflight.report().clone();
    extra
        .connection_observations
        .push(extra.connection_witness.clone());
    assert_eq!(
        derive_preflight_observer_binding(handoff.judge_schedule(), &extra),
        None
    );

    let mut wrong_terminal = managed_preflight.report().clone();
    wrong_terminal.connection_witness = connection_with_label(b"wrong terminal");
    assert_eq!(
        derive_preflight_observer_binding(handoff.judge_schedule(), &wrong_terminal),
        None
    );

    let mut mutated_connection = managed_preflight.report().clone();
    mutated_connection.connection_observations[2] = connection_with_label(b"intermediate");
    let mutated_connection_digest =
        derive_preflight_observer_binding(handoff.judge_schedule(), &mutated_connection)
            .expect("complete mutated connection binding");
    assert_ne!(mutated_connection_digest, expected);

    let mut ordered_connections = managed_preflight.report().clone();
    ordered_connections.connection_observations[2] = connection_with_label(b"ordered a");
    ordered_connections.connection_observations[3] = connection_with_label(b"ordered b");
    let mut reversed_connections = ordered_connections.clone();
    reversed_connections.connection_observations.swap(2, 3);
    assert_ne!(
        derive_preflight_observer_binding(handoff.judge_schedule(), &ordered_connections),
        derive_preflight_observer_binding(handoff.judge_schedule(), &reversed_connections)
    );

    let alternate_process = process_with_label(b"alternate process");
    let mut changed_process = managed_preflight.report().clone();
    changed_process.initial_process_witness = alternate_process.clone();
    assert_ne!(
        derive_preflight_observer_binding(handoff.judge_schedule(), &changed_process),
        Some(expected.clone())
    );

    let mut changed_native = managed_preflight.report().clone();
    changed_native.native_load = crate::local_ollama_managed_preflight::test_support::native_load(
        &crate::local_ollama_managed_preflight::test_support::package(),
        &alternate_process,
    );
    assert_ne!(
        derive_preflight_observer_binding(handoff.judge_schedule(), &changed_native),
        Some(expected.clone())
    );

    let other_handoff = self::handoff(&fixture, "receipt-preflight-other-schedule");
    assert_ne!(
        handoff.judge_schedule().candidate_judge_schedule_id(),
        other_handoff.judge_schedule().candidate_judge_schedule_id()
    );
    assert_ne!(
        derive_preflight_observer_binding(
            other_handoff.judge_schedule(),
            managed_preflight.report(),
        ),
        Some(expected)
    );
}

#[test]
fn authority_error_mapping_preserves_every_initial_callback_and_final_combination() {
    assert!(matches!(
        map_authority_error(ManagedJudgeScheduleExecutionAuthorityError::Initial(
            failures()
        )),
        ManagedLocalJudgeReceiptCompilerError::InitialAuthority(_)
    ));
    assert!(matches!(
        map_authority_error(
            ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
                initial: failures(),
                final_validation: failures(),
            }
        ),
        ManagedLocalJudgeReceiptCompilerError::InitialAndFinalAuthority { .. }
    ));
    assert!(matches!(
        map_authority_error(ManagedJudgeScheduleExecutionAuthorityError::Callback(
            ManagedLocalJudgeReceiptCompilerError::Relationship(
                ManagedLocalJudgeReceiptRelationship::PortableJudge
            )
        )),
        ManagedLocalJudgeReceiptCompilerError::Relationship(
            ManagedLocalJudgeReceiptRelationship::PortableJudge
        )
    ));
    assert!(matches!(
        map_authority_error(ManagedJudgeScheduleExecutionAuthorityError::Final(
            failures()
        )),
        ManagedLocalJudgeReceiptCompilerError::FinalAuthority(_)
    ));
    assert!(matches!(
        map_authority_error(
            ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
                callback: ManagedLocalJudgeReceiptCompilerError::Relationship(
                    ManagedLocalJudgeReceiptRelationship::ManagedPreflight,
                ),
                final_validation: failures(),
            }
        ),
        ManagedLocalJudgeReceiptCompilerError::CompilationAndFinalAuthority { .. }
    ));
}

#[test]
fn compiler_errors_and_receipt_capability_expose_no_content_traits() {
    let errors = [
        ManagedLocalJudgeReceiptCompilerError::InitialAuthority(failures()),
        ManagedLocalJudgeReceiptCompilerError::InitialAndFinalAuthority {
            initial: failures(),
            final_validation: failures(),
        },
        ManagedLocalJudgeReceiptCompilerError::Relationship(
            ManagedLocalJudgeReceiptRelationship::AttemptCount,
        ),
        ManagedLocalJudgeReceiptCompilerError::Portable(
            rewrite_model::GenerationQualificationContractError::InvalidEncoding,
        ),
        ManagedLocalJudgeReceiptCompilerError::FinalAuthority(failures()),
        ManagedLocalJudgeReceiptCompilerError::CompilationAndFinalAuthority {
            compilation: Box::new(ManagedLocalJudgeReceiptCompilerError::Relationship(
                ManagedLocalJudgeReceiptRelationship::ManagedPreflight,
            )),
            final_validation: failures(),
        },
    ];
    for error in errors {
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(debug.starts_with("ManagedLocalJudgeReceiptCompilerError"));
        assert!(!debug.contains("source content"));
        assert!(!debug.contains("candidate content"));
        assert!(!debug.contains("prompt content"));
        assert!(!debug.contains("rationale content"));
        assert!(!display.contains("source content"));
        assert!(!display.contains("candidate content"));
        std::hint::black_box(std::error::Error::source(&error));
    }

    let _ = <Receipt as AmbiguousIfClone<_>>::marker;
    let _ = <Receipt as AmbiguousIfSerialize<_>>::marker;
    std::hint::black_box(ManagedLocalJudgeReceiptCompiler::compile);
}

fn handoff<'store>(fixture: &'store Fixture, suffix: &str) -> CandidateJudgeRunnerHandoff<'store> {
    let prepared = prepare(
        fixture,
        suffix,
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("passed deterministic record must prepare")
    };
    ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("runner handoff")
}

fn portable_outputs(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
) -> (
    CandidateJudgeResponseAggregateV1,
    CandidateJudgeObservationBatchV1,
) {
    let response_ids = (0..handoff.judge_schedule().entries().len())
        .map(|index| {
            OllamaRetainedSessionResponseId::from_derived_digest(Digest::sha256(
                format!("managed receipt response {index}").as_bytes(),
            ))
        })
        .collect::<Vec<_>>();
    let responses = CandidateJudgeResponseAggregateV1::new(
        handoff.judge_plan(),
        handoff.judge_schedule(),
        handoff.request_aggregate(),
        response_ids.clone(),
    )
    .expect("response aggregate");
    let observations = response_ids
        .into_iter()
        .enumerate()
        .map(|(index, response_id)| {
            let entry = &handoff.judge_schedule().entries()[index];
            let planned = handoff
                .judge_plan()
                .cases()
                .iter()
                .find(|case| case.case_id() == entry.case_id())
                .expect("planned case");
            CandidateJudgeObservationV1::new(
                handoff.judge_plan(),
                handoff.judge_schedule(),
                handoff.request_aggregate(),
                index,
                response_id,
                CandidateJudgeChoiceV1::Tie,
                planned.rubric_clause_ids().to_vec(),
            )
            .expect("observation")
        })
        .collect();
    let observations = CandidateJudgeObservationBatchV1::new(
        handoff.judge_plan(),
        handoff.judge_schedule(),
        handoff.request_aggregate(),
        observations,
    )
    .expect("observation batch");
    (responses, observations)
}

fn authority<'records>(
    handoff: &'records CandidateJudgeRunnerHandoff<'_>,
    managed_preflight: &LocalOllamaManagedPreflightOutcome,
) -> FakeReleasedAuthority<'records> {
    let count = u64::from(handoff.judge_schedule().entry_count());
    FakeReleasedAuthority {
        judge_system: handoff.judge_system(),
        portable_relationship: true,
        attempt_count: count,
        preflight_observer_binding_digest: derive_preflight_observer_binding(
            handoff.judge_schedule(),
            managed_preflight.report(),
        )
        .expect("preflight observer binding"),
        input: ManagedLocalJudgeReceiptRecordV1Input {
            judge_runtime_installation_generation: 7,
            judge_model_installation_generation: 11,
            managed_preflight_digest: Digest::sha256(b"overwritten by compiler"),
            retained_session_preflight_digest: Digest::sha256(b"retained preflight"),
            residency_receipt_aggregate_digest: Digest::sha256(b"residency aggregate"),
            process_observation_aggregate_digest: Digest::sha256(b"process aggregate"),
            native_load_observation_aggregate_digest: Digest::sha256(b"native aggregate"),
            connection_observation_aggregate_digest: Digest::sha256(b"connection aggregate"),
            effective_runtime_state_observation_aggregate_digest: Digest::sha256(
                b"effective state aggregate",
            ),
            judge_effective_runtime_state_join_id:
                ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(Digest::sha256(
                    b"effective state join",
                )),
            first_response_ordinal: 8,
            last_response_ordinal: 7 + count * 9,
        },
    }
}

fn failures() -> ManagedJudgeScheduleAuthorityFailures {
    ManagedJudgeScheduleAuthorityFailures::cancelled_for_test()
}

fn process_with_label(label: &[u8]) -> AttachedProcessEvidence {
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::WindowsOwnerPidProcessHandle,
        owner_pid: 42,
        process_instance_digest: Digest::sha256(label),
        ownership_snapshot_digest: Digest::sha256(b"alternate ownership"),
        entrypoint_object_digest: Digest::sha256(b"alternate object"),
        entrypoint_digest: Digest::sha256(b"alternate entrypoint"),
        entrypoint_bytes: 11,
        platform_evidence_digest: Digest::sha256(b"alternate platform"),
    })
    .expect("alternate process")
}

fn connection_with_label(label: &[u8]) -> RetainedTcpConnectionEvidence {
    RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
        attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
        sharing_limitation: TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable,
        process_evidence_digest: Digest::sha256(b"connection process"),
        platform_connection_digest: Digest::sha256(label),
    })
    .expect("alternate connection")
}

fn assert_relationship(
    result: &Result<
        rewrite_model::ManagedLocalJudgeReceiptRecordV1,
        ManagedLocalJudgeReceiptCompilerError,
    >,
    expected: ManagedLocalJudgeReceiptRelationship,
) {
    assert!(matches!(
        result,
        Err(ManagedLocalJudgeReceiptCompilerError::Relationship(observed)) if *observed == expected
    ));
}
