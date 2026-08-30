use std::time::{Duration, Instant};
use std::{cell::RefCell, future::ready};

use super::*;
use crate::candidate_judge_preparation::CandidateJudgeJoinCompilationError;
use crate::local_ollama_managed_preflight::generation::managed_local_judge_receipt::ManagedLocalJudgeReceiptRelationship;
use crate::local_ollama_managed_preflight::generation::managed_schedule_runner::remaining_before_deadline_at;
use crate::local_ollama_managed_preflight::generation::verified_candidate_judge_join::CandidateJudgeJoinPrimaryError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailStage {
    Configuration,
    ScheduleExecution,
    ReceiptCompilation,
    JoinCompilation,
}

#[test]
fn configuration_time_consumes_the_single_joined_run_deadline() {
    let started = Instant::now();
    let deadline = joined_run_deadline(started, 10).expect("representable deadline");
    let after_configuration = started + Duration::from_millis(4);

    assert_eq!(
        remaining_before_deadline_at(deadline, after_configuration),
        Ok(Duration::from_millis(6))
    );
    assert!(remaining_before_deadline_at(deadline, started + Duration::from_millis(10)).is_err());
    assert!(remaining_before_deadline_at(deadline, started + Duration::from_millis(11)).is_err());
    assert_ne!(
        deadline,
        after_configuration + Duration::from_millis(10),
        "schedule execution must not reset the deadline after configuration"
    );
}

#[tokio::test]
async fn pipeline_calls_each_consuming_stage_once_and_stops_at_first_failure() {
    for fail_on in [
        None,
        Some(FailStage::Configuration),
        Some(FailStage::ScheduleExecution),
        Some(FailStage::ReceiptCompilation),
        Some(FailStage::JoinCompilation),
    ] {
        let calls = RefCell::new(Vec::new());
        let result = run_pipeline(
            || {
                calls.borrow_mut().push(FailStage::Configuration);
                if fail_on == Some(FailStage::Configuration) {
                    Err("configuration")
                } else {
                    Ok(1_u8)
                }
            },
            |configuration| {
                assert_eq!(configuration, 1);
                calls.borrow_mut().push(FailStage::ScheduleExecution);
                ready(if fail_on == Some(FailStage::ScheduleExecution) {
                    Err("schedule")
                } else {
                    Ok(2_u8)
                })
            },
            |execution| {
                assert_eq!(execution, 2);
                calls.borrow_mut().push(FailStage::ReceiptCompilation);
                ready(if fail_on == Some(FailStage::ReceiptCompilation) {
                    Err("receipt")
                } else {
                    Ok(3_u8)
                })
            },
            |receipt| {
                assert_eq!(receipt, 3);
                calls.borrow_mut().push(FailStage::JoinCompilation);
                ready(if fail_on == Some(FailStage::JoinCompilation) {
                    Err("join")
                } else {
                    Ok(4_u8)
                })
            },
        )
        .await;

        let expected_calls = match fail_on {
            None | Some(FailStage::JoinCompilation) => vec![
                FailStage::Configuration,
                FailStage::ScheduleExecution,
                FailStage::ReceiptCompilation,
                FailStage::JoinCompilation,
            ],
            Some(FailStage::Configuration) => vec![FailStage::Configuration],
            Some(FailStage::ScheduleExecution) => {
                vec![FailStage::Configuration, FailStage::ScheduleExecution]
            }
            Some(FailStage::ReceiptCompilation) => vec![
                FailStage::Configuration,
                FailStage::ScheduleExecution,
                FailStage::ReceiptCompilation,
            ],
        };
        assert_eq!(*calls.borrow(), expected_calls);
        match fail_on {
            None => assert!(matches!(result, Ok(4))),
            Some(expected) => assert_eq!(pipeline_error_stage(result), expected),
        }
    }
}

#[tokio::test]
async fn deadline_pipeline_checks_every_boundary_and_stops_after_terminal_failure() {
    for fail_on in [
        FailStage::Configuration,
        FailStage::ScheduleExecution,
        FailStage::ReceiptCompilation,
        FailStage::JoinCompilation,
    ] {
        let cancellation = CancellationToken::new();
        let calls = RefCell::new(Vec::new());
        let result = run_pipeline_until(
            &cancellation,
            Instant::now() + Duration::from_secs(30),
            || {
                calls.borrow_mut().push(FailStage::Configuration);
                if fail_on == FailStage::Configuration {
                    cancellation.cancel();
                    Err("configuration")
                } else {
                    Ok(1_u8)
                }
            },
            |configuration| {
                assert_eq!(configuration, 1);
                calls.borrow_mut().push(FailStage::ScheduleExecution);
                if fail_on == FailStage::ScheduleExecution {
                    cancellation.cancel();
                }
                ready(if fail_on == FailStage::ScheduleExecution {
                    Err("schedule")
                } else {
                    Ok(2_u8)
                })
            },
            |execution| {
                assert_eq!(execution, 2);
                calls.borrow_mut().push(FailStage::ReceiptCompilation);
                if fail_on == FailStage::ReceiptCompilation {
                    cancellation.cancel();
                }
                ready(if fail_on == FailStage::ReceiptCompilation {
                    Err("receipt")
                } else {
                    Ok(3_u8)
                })
            },
            |receipt| {
                assert_eq!(receipt, 3);
                calls.borrow_mut().push(FailStage::JoinCompilation);
                if fail_on == FailStage::JoinCompilation {
                    cancellation.cancel();
                }
                ready(if fail_on == FailStage::JoinCompilation {
                    Err("join")
                } else {
                    Ok(4_u8)
                })
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(ManagedCandidateJudgePipelineError::Terminal(
                OperationGateFailure::Cancelled
            ))
        ));
        let expected_len = match fail_on {
            FailStage::Configuration => 1,
            FailStage::ScheduleExecution => 2,
            FailStage::ReceiptCompilation => 3,
            FailStage::JoinCompilation => 4,
        };
        assert_eq!(calls.borrow().len(), expected_len);
    }
}

#[tokio::test]
async fn expired_and_cancelled_deadline_suppresses_configuration() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let configured = RefCell::new(false);
    let result = run_pipeline_until(
        &cancellation,
        Instant::now(),
        || {
            configured.replace(true);
            Ok::<_, &'static str>(1_u8)
        },
        |_configuration| ready(Ok::<_, &'static str>(2_u8)),
        |_execution| ready(Ok::<_, &'static str>(3_u8)),
        |_receipt| ready(Ok::<_, &'static str>(4_u8)),
    )
    .await;
    assert!(!configured.into_inner());
    assert!(matches!(
        result,
        Err(ManagedCandidateJudgePipelineError::Terminal(
            OperationGateFailure::DeadlineExceeded
        ))
    ));
}

#[tokio::test]
async fn active_deadline_pipeline_preserves_success_and_each_underlying_stage() {
    for fail_on in [
        None,
        Some(FailStage::Configuration),
        Some(FailStage::ScheduleExecution),
        Some(FailStage::ReceiptCompilation),
        Some(FailStage::JoinCompilation),
    ] {
        let result = run_pipeline_until(
            &CancellationToken::new(),
            Instant::now() + Duration::from_secs(30),
            || stage_result(fail_on, FailStage::Configuration, 1_u8, "configuration"),
            |configuration| {
                assert_eq!(configuration, 1);
                ready(stage_result(
                    fail_on,
                    FailStage::ScheduleExecution,
                    2_u8,
                    "schedule",
                ))
            },
            |execution| {
                assert_eq!(execution, 2);
                ready(stage_result(
                    fail_on,
                    FailStage::ReceiptCompilation,
                    3_u8,
                    "receipt",
                ))
            },
            |receipt| {
                assert_eq!(receipt, 3);
                ready(stage_result(
                    fail_on,
                    FailStage::JoinCompilation,
                    4_u8,
                    "join",
                ))
            },
        )
        .await;
        match fail_on {
            None => assert!(matches!(result, Ok(4))),
            Some(expected) => assert_eq!(pipeline_error_stage(result), expected),
        }
    }
}

fn stage_result(
    fail_on: Option<FailStage>,
    stage: FailStage,
    value: u8,
    error: &'static str,
) -> Result<u8, &'static str> {
    if fail_on == Some(stage) {
        Err(error)
    } else {
        Ok(value)
    }
}

#[test]
fn component_errors_map_to_stable_redacted_public_categories() {
    let errors = [
        map_pipeline_error(ManagedCandidateJudgePipelineError::Configuration(
            ManagedJudgeRunnerConfigurationError::Cancelled,
        )),
        map_pipeline_error(ManagedCandidateJudgePipelineError::ScheduleExecution(
            ManagedJudgeScheduleRunnerError::CancelledBeforeLaunch,
        )),
        map_pipeline_error(ManagedCandidateJudgePipelineError::ReceiptCompilation(
            ManagedLocalJudgeReceiptCompilerError::Relationship(
                ManagedLocalJudgeReceiptRelationship::AttemptCount,
            ),
        )),
        map_pipeline_error(ManagedCandidateJudgePipelineError::JoinCompilation(
            VerifiedCandidateJudgeJoinCompilerError::Compilation(
                CandidateJudgeJoinPrimaryError::Join(CandidateJudgeJoinCompilationError::Cancelled),
            ),
        )),
    ];
    assert_eq!(
        errors
            .iter()
            .map(ManagedCandidateJudgeRunError::kind)
            .collect::<Vec<_>>(),
        [
            ManagedCandidateJudgeRunErrorKind::Configuration,
            ManagedCandidateJudgeRunErrorKind::ScheduleExecution,
            ManagedCandidateJudgeRunErrorKind::ReceiptCompilation,
            ManagedCandidateJudgeRunErrorKind::JoinCompilation,
        ]
    );
    for error in errors {
        let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&error);
        while let Some(current) = source {
            let debug = format!("{current:?}");
            let display = current.to_string();
            for forbidden in [
                "source content",
                "candidate content",
                "prompt content",
                "response content",
                "rationale content",
            ] {
                assert!(!debug.contains(forbidden));
                assert!(!display.contains(forbidden));
            }
            source = current.source();
        }
    }
}

fn pipeline_error_stage(
    result: Result<
        u8,
        ManagedCandidateJudgePipelineError<&'static str, &'static str, &'static str, &'static str>,
    >,
) -> FailStage {
    match result.expect_err("selected stage must fail") {
        ManagedCandidateJudgePipelineError::Terminal(_) => {
            panic!("compatibility pipeline has no terminal deadline")
        }
        ManagedCandidateJudgePipelineError::Configuration(_) => FailStage::Configuration,
        ManagedCandidateJudgePipelineError::ScheduleExecution(_) => FailStage::ScheduleExecution,
        ManagedCandidateJudgePipelineError::ReceiptCompilation(_) => FailStage::ReceiptCompilation,
        ManagedCandidateJudgePipelineError::JoinCompilation(_) => FailStage::JoinCompilation,
    }
}
