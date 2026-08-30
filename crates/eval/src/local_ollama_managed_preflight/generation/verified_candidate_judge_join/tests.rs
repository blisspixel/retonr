use rewrite_model::{CandidateJudgeChoiceV1, CandidateJudgeTriageReportRelationshipV1};
use rewrite_types::CancellationToken;

use super::{
    CandidateJudgeJoinPrimaryError, VerifiedCandidateJudgeJoin, VerifiedCandidateJudgeJoinCompiler,
    VerifiedCandidateJudgeJoinCompilerError, VerifiedCandidateJudgeJoinRelationshipError,
    VerifiedCandidateJudgeJoinRevalidationErrorKind, compile_from_parts,
    compile_from_parts_with_post_triage, map_authority_error, map_revalidation_error,
    validate_join_parts,
};
use crate::candidate_judge_preparation::{
    CandidateJudgeJoinCompilationError, CandidateJudgeJoinCompilationRelationship,
    CandidateJudgeTriageCompilationError,
};
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::local_ollama_managed_preflight::generation::managed_schedule_runner::ManagedJudgeScheduleExecutionAuthorityError;
use support::{
    failures, foreign_judge_system, handoff, managed_receipt, observation_batch, portable_outputs,
    primary, receipt_with_runtime_generation,
};

pub(in crate::local_ollama_managed_preflight::generation) mod support;

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

type Join = VerifiedCandidateJudgeJoin<'static, 'static, 'static, 'static>;

fn typecheck_authority_seam(mut join: Join, cancellation: &CancellationToken) {
    let _ = join.with_revalidated_authorities(cancellation, |_view| Ok::<(), ()>(()));
}

#[test]
fn exact_retained_closure_compiles_content_free_opaque_join() {
    let fixture = Fixture::judge_pair();
    let handoff = handoff(&fixture, "verified-join");
    let (responses, observations) = portable_outputs(&handoff);
    let receipt = managed_receipt(&handoff, &responses, &observations);

    let compiled = compile_from_parts(
        &handoff,
        handoff.judge_system(),
        &responses,
        &observations,
        &receipt,
        &CancellationToken::new(),
    )
    .expect("opaque candidate judge join");

    assert_eq!(
        compiled.record.candidate_judge_plan_id(),
        handoff.judge_plan().candidate_judge_plan_id()
    );
    assert_eq!(
        compiled.record.managed_local_judge_receipt_id(),
        receipt.managed_local_judge_receipt_id()
    );
    assert_eq!(
        compiled.record.triage_report_digest(),
        compiled.triage.relationship().digest()
    );
    assert!(!compiled.record.candidate_semantics_proven());
    assert!(!compiled.record.judge_correctness_proven());
    assert!(!compiled.record.qualified());
    // This freezes the portable closure over both held candidate receipt sets,
    // deterministic gate, schedule, durable receipt, and exact triage digest.
    assert_eq!(
        compiled.record.candidate_judge_join_id().digest().as_str(),
        "05c87b5dbf7111e120e410a98840ed91950591fa379ba07f8bab0fa94d0c7c0c"
    );

    let record_json = serde_json::to_string(&compiled.record).expect("join JSON");
    let triage_json = std::str::from_utf8(compiled.triage.canonical_json()).expect("triage UTF-8");
    let debug = format!("{:?} {:?}", compiled.record, compiled.triage);
    for content in [
        "Acme 42 needs polish.",
        "Acme, 42 needs polish!",
        "Acme 42 needs polish?",
        "selected candidate verified-join-a 1",
        "selected candidate verified-join-b 1",
        "prompt",
        "rationale",
    ] {
        assert!(!record_json.contains(content));
        assert!(!triage_json.contains(content));
        assert!(!debug.contains(content));
    }

    for choice in [
        CandidateJudgeChoiceV1::First,
        CandidateJudgeChoiceV1::Second,
        CandidateJudgeChoiceV1::Tie,
        CandidateJudgeChoiceV1::Abstain,
    ] {
        let observations = observation_batch(&handoff, &responses, choice);
        let receipt = managed_receipt(&handoff, &responses, &observations);
        compile_from_parts(
            &handoff,
            handoff.judge_system(),
            &responses,
            &observations,
            &receipt,
            &CancellationToken::new(),
        )
        .expect("every closed choice compiles");
    }
}

#[test]
fn cancellation_before_and_after_triage_cannot_release_join() {
    let fixture = Fixture::judge_pair();
    let handoff = handoff(&fixture, "join-failures");
    let (responses, observations) = portable_outputs(&handoff);
    let receipt = managed_receipt(&handoff, &responses, &observations);

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        compile_from_parts(
            &handoff,
            handoff.judge_system(),
            &responses,
            &observations,
            &receipt,
            &cancellation,
        ),
        Err(CandidateJudgeJoinPrimaryError::Triage(_))
    ));

    let triage = handoff
        .compile_compatibility_triage(&observations, &CancellationToken::new())
        .expect("triage");
    assert!(matches!(
        handoff.compile_join_record(
            handoff.judge_system(),
            &responses,
            &observations,
            &receipt,
            &triage,
            &cancellation,
        ),
        Err(CandidateJudgeJoinCompilationError::Cancelled)
    ));

    let post_triage_cancellation = CancellationToken::new();
    assert!(matches!(
        compile_from_parts_with_post_triage(
            &handoff,
            handoff.judge_system(),
            &responses,
            &observations,
            &receipt,
            &post_triage_cancellation,
            || post_triage_cancellation.cancel(),
        ),
        Err(CandidateJudgeJoinPrimaryError::Join(
            CandidateJudgeJoinCompilationError::Cancelled
        ))
    ));
}

#[test]
fn foreign_system_response_observation_receipt_and_triage_fail_closed() {
    let fixture = Fixture::judge_pair();
    let handoff = handoff(&fixture, "join-substitution");
    let (responses, observations) = portable_outputs(&handoff);
    let receipt = managed_receipt(&handoff, &responses, &observations);
    let triage = handoff
        .compile_compatibility_triage(&observations, &CancellationToken::new())
        .expect("triage");

    let foreign_system = foreign_judge_system();
    assert!(matches!(
        handoff.compile_join_record(
            &foreign_system,
            &responses,
            &observations,
            &receipt,
            &triage,
            &CancellationToken::new(),
        ),
        Err(CandidateJudgeJoinCompilationError::Relationship(
            CandidateJudgeJoinCompilationRelationship::JudgeSystem
        ))
    ));

    let foreign_triage =
        CandidateJudgeTriageReportRelationshipV1::new(b"{}").expect("foreign triage framing");
    assert!(matches!(
        handoff.compile_join_record_with_triage_for_test(
            handoff.judge_system(),
            &responses,
            &observations,
            &receipt,
            triage.canonical_json(),
            &foreign_triage,
            &CancellationToken::new(),
        ),
        Err(CandidateJudgeJoinCompilationError::Relationship(
            CandidateJudgeJoinCompilationRelationship::TriageReport
        ))
    ));

    let substituted_observations =
        observation_batch(&handoff, &responses, CandidateJudgeChoiceV1::First);
    assert!(matches!(
        compile_from_parts(
            &handoff,
            handoff.judge_system(),
            &responses,
            &substituted_observations,
            &receipt,
            &CancellationToken::new(),
        ),
        Err(CandidateJudgeJoinPrimaryError::Join(
            CandidateJudgeJoinCompilationError::Portable(_)
        ))
    ));

    let other_handoff = self::handoff(&fixture, "join-foreign");
    let (foreign_responses, foreign_observations) = portable_outputs(&other_handoff);
    let foreign_receipt =
        managed_receipt(&other_handoff, &foreign_responses, &foreign_observations);
    for (candidate_responses, candidate_receipt) in [
        (&foreign_responses, &receipt),
        (&responses, &foreign_receipt),
    ] {
        assert!(matches!(
            handoff.compile_join_record(
                handoff.judge_system(),
                candidate_responses,
                &observations,
                candidate_receipt,
                &triage,
                &CancellationToken::new(),
            ),
            Err(CandidateJudgeJoinCompilationError::Portable(_))
        ));
    }
}

#[test]
fn outer_authority_mapping_preserves_every_failure_combination() {
    assert!(matches!(
        map_authority_error(ManagedJudgeScheduleExecutionAuthorityError::Initial(
            failures()
        )),
        VerifiedCandidateJudgeJoinCompilerError::InitialAuthority(_)
    ));
    assert!(matches!(
        map_authority_error(
            ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
                initial: failures(),
                final_validation: failures(),
            }
        ),
        VerifiedCandidateJudgeJoinCompilerError::InitialAndFinalAuthority { .. }
    ));
    assert!(matches!(
        map_authority_error(ManagedJudgeScheduleExecutionAuthorityError::Callback(
            primary()
        )),
        VerifiedCandidateJudgeJoinCompilerError::Compilation(_)
    ));
    assert!(matches!(
        map_authority_error(ManagedJudgeScheduleExecutionAuthorityError::Final(
            failures()
        )),
        VerifiedCandidateJudgeJoinCompilerError::FinalAuthority(_)
    ));
    assert!(matches!(
        map_authority_error(
            ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
                callback: primary(),
                final_validation: failures(),
            }
        ),
        VerifiedCandidateJudgeJoinCompilerError::CompilationAndFinalAuthority { .. }
    ));
}

#[test]
fn public_revalidation_mapping_preserves_every_failure_combination() {
    let mapped = [
        map_revalidation_error(ManagedJudgeScheduleExecutionAuthorityError::Initial(
            failures(),
        )),
        map_revalidation_error(
            ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
                initial: failures(),
                final_validation: failures(),
            },
        ),
        map_revalidation_error(ManagedJudgeScheduleExecutionAuthorityError::Callback(
            VerifiedCandidateJudgeJoinRelationshipError,
        )),
        map_revalidation_error(ManagedJudgeScheduleExecutionAuthorityError::Final(
            failures(),
        )),
        map_revalidation_error(
            ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
                callback: VerifiedCandidateJudgeJoinRelationshipError,
                final_validation: failures(),
            },
        ),
    ];
    assert_eq!(
        mapped
            .iter()
            .map(super::VerifiedCandidateJudgeJoinRevalidationError::kind)
            .collect::<Vec<_>>(),
        [
            VerifiedCandidateJudgeJoinRevalidationErrorKind::InitialAuthority,
            VerifiedCandidateJudgeJoinRevalidationErrorKind::InitialAndFinalAuthority,
            VerifiedCandidateJudgeJoinRevalidationErrorKind::Relationship,
            VerifiedCandidateJudgeJoinRevalidationErrorKind::FinalAuthority,
            VerifiedCandidateJudgeJoinRevalidationErrorKind::RelationshipAndFinalAuthority,
        ]
    );
    for error in mapped {
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(debug.starts_with("VerifiedCandidateJudgeJoinRevalidationError"));
        assert!(!debug.contains("source content"));
        assert!(!debug.contains("candidate content"));
        assert!(!debug.contains("prompt content"));
        assert!(!debug.contains("response content"));
        assert!(!display.contains("source content"));
        assert!(!display.contains("candidate content"));
    }
}

#[test]
fn public_revalidation_kernel_rejects_stored_triage_and_full_record_substitution() {
    let fixture = Fixture::judge_pair();
    let handoff = handoff(&fixture, "join-revalidation");
    let (responses, observations) = portable_outputs(&handoff);
    let receipt = managed_receipt(&handoff, &responses, &observations);
    let compiled = compile_from_parts(
        &handoff,
        handoff.judge_system(),
        &responses,
        &observations,
        &receipt,
        &CancellationToken::new(),
    )
    .expect("stored join");
    assert!(
        validate_join_parts(
            &handoff,
            handoff.judge_system(),
            &responses,
            &observations,
            &receipt,
            &compiled.triage,
            &compiled.record,
            &CancellationToken::new(),
        )
        .is_ok()
    );

    let substituted_observations =
        observation_batch(&handoff, &responses, CandidateJudgeChoiceV1::First);
    let substituted_receipt = managed_receipt(&handoff, &responses, &substituted_observations);
    let substituted_triage = compile_from_parts(
        &handoff,
        handoff.judge_system(),
        &responses,
        &substituted_observations,
        &substituted_receipt,
        &CancellationToken::new(),
    )
    .expect("substituted triage")
    .triage;
    assert!(
        validate_join_parts(
            &handoff,
            handoff.judge_system(),
            &responses,
            &observations,
            &receipt,
            &substituted_triage,
            &compiled.record,
            &CancellationToken::new(),
        )
        .is_err()
    );

    let substituted_receipt =
        receipt_with_runtime_generation(&handoff, &responses, &observations, &receipt, 9);
    let substituted_record = handoff
        .compile_join_record(
            handoff.judge_system(),
            &responses,
            &observations,
            &substituted_receipt,
            &compiled.triage,
            &CancellationToken::new(),
        )
        .expect("valid record over substituted runtime generation");
    assert!(
        validate_join_parts(
            &handoff,
            handoff.judge_system(),
            &responses,
            &observations,
            &receipt,
            &compiled.triage,
            &substituted_record,
            &CancellationToken::new(),
        )
        .is_err()
    );
}

#[test]
fn opaque_join_and_redacted_errors_expose_no_content_traits() {
    let errors = [
        VerifiedCandidateJudgeJoinCompilerError::InitialAuthority(failures()),
        VerifiedCandidateJudgeJoinCompilerError::InitialAndFinalAuthority {
            initial: failures(),
            final_validation: failures(),
        },
        VerifiedCandidateJudgeJoinCompilerError::Compilation(primary()),
        VerifiedCandidateJudgeJoinCompilerError::FinalAuthority(failures()),
        VerifiedCandidateJudgeJoinCompilerError::CompilationAndFinalAuthority {
            compilation: Box::new(primary()),
            final_validation: failures(),
        },
    ];
    for error in errors {
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(debug.starts_with("VerifiedCandidateJudgeJoinCompilerError"));
        assert!(!debug.contains("source content"));
        assert!(!debug.contains("candidate content"));
        assert!(!debug.contains("prompt content"));
        assert!(!debug.contains("rationale content"));
        assert!(!display.contains("source content"));
        assert!(!display.contains("candidate content"));
        std::hint::black_box(std::error::Error::source(&error));
    }

    for error in [
        CandidateJudgeJoinPrimaryError::Triage(CandidateJudgeTriageCompilationError::Encoding),
        primary(),
    ] {
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(!debug.contains("source content"));
        assert!(!display.contains("source content"));
        std::hint::black_box(std::error::Error::source(&error));
    }

    let _ = <Join as AmbiguousIfClone<_>>::marker;
    let _ = <Join as AmbiguousIfSerialize<_>>::marker;
    std::hint::black_box(VerifiedCandidateJudgeJoinCompiler::compile);
    std::hint::black_box(Join::record);
    std::hint::black_box(Join::triage_report);
    std::hint::black_box(Join::revalidate);
    std::hint::black_box(typecheck_authority_seam as fn(Join, &CancellationToken));
}

#[test]
fn handoff_join_errors_are_content_redacted_across_closed_variants() {
    let errors = [
        CandidateJudgeJoinCompilationError::Cancelled,
        CandidateJudgeJoinCompilationError::Relationship(
            CandidateJudgeJoinCompilationRelationship::JudgeSystem,
        ),
        CandidateJudgeJoinCompilationError::Relationship(
            CandidateJudgeJoinCompilationRelationship::TriageReport,
        ),
        CandidateJudgeJoinCompilationError::Portable(
            rewrite_model::GenerationQualificationContractError::InvalidEncoding,
        ),
    ];
    for error in errors {
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(debug.starts_with("CandidateJudgeJoinCompilationError"));
        for forbidden in ["source content", "candidate content", "prompt", "rationale"] {
            assert!(!debug.contains(forbidden));
            assert!(!display.contains(forbidden));
        }
        std::hint::black_box(std::error::Error::source(&error));
    }
}
