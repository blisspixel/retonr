//! Bounded CPU concurrency runner for deterministic evaluation suites.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use rewrite_app::{AppError, CandidateCheckRequest, CandidateCheckService};
use rewrite_model::{EvaluationCase, ExpectedOutput, ReferenceJudgment};
use rewrite_types::RewriteStatus;

use super::{
    CategoryResult, EVALUATION_SCHEMA_VERSION, EvaluationFailure, EvaluationReport,
    EvaluationSuite, TransformationCoverage,
};

/// Default worker cap for deterministic evaluation suites (serial execution).
pub const DEFAULT_EVALUATION_WORKER_CAP: usize = 1;

/// Maximum worker threads permitted for concurrent evaluation.
pub const MAX_EVALUATION_WORKER_CAP: usize = 16;

#[derive(Clone, Debug)]
struct CaseExecutionOutcome {
    index: usize,
    category: String,
    acceptable_increment: usize,
    rewritten_increment: usize,
    passed: bool,
    failure: Option<EvaluationFailure>,
}

fn execute_single_case(index: usize, case: &EvaluationCase) -> CaseExecutionOutcome {
    let result = CandidateCheckService::check(CandidateCheckRequest::new(
        case.source.as_bytes().to_vec(),
        case.candidate.clone(),
        case.protected_terms.clone(),
    ));

    let mut acceptable_increment = 0;
    let mut rewritten_increment = 0;

    match result {
        Ok(result) => {
            if case.reference_judgment == ReferenceJudgment::Acceptable
                && case.source.as_bytes() != case.candidate.as_bytes()
            {
                acceptable_increment = 1;
                if result.record.status == RewriteStatus::Rewritten
                    && result.output == case.candidate.as_bytes()
                {
                    rewritten_increment = 1;
                }
            }
            let expected_bytes = match case.expected_output {
                ExpectedOutput::Source => case.source.as_bytes(),
                ExpectedOutput::Candidate => case.candidate.as_bytes(),
            };
            let output_mismatch = result.output != expected_bytes;
            if result.record.status == case.expected_status
                && result.record.reason == case.expected_reason
                && !output_mismatch
            {
                CaseExecutionOutcome {
                    index,
                    category: case.category.clone(),
                    acceptable_increment,
                    rewritten_increment,
                    passed: true,
                    failure: None,
                }
            } else {
                CaseExecutionOutcome {
                    index,
                    category: case.category.clone(),
                    acceptable_increment,
                    rewritten_increment,
                    passed: false,
                    failure: Some(EvaluationFailure {
                        id: case.id.clone(),
                        category: case.category.clone(),
                        expected_status: case.expected_status,
                        actual_status: Some(result.record.status),
                        expected_reason: case.expected_reason,
                        actual_reason: result.record.reason,
                        output_mismatch,
                        error: None,
                    }),
                }
            }
        }
        Err(error) => CaseExecutionOutcome {
            index,
            category: case.category.clone(),
            acceptable_increment: 0,
            rewritten_increment: 0,
            passed: false,
            failure: Some(operational_failure(case, &error)),
        },
    }
}

/// Executes all suite cases serially through the application check service.
#[must_use]
pub fn run_suite(suite: &EvaluationSuite) -> EvaluationReport {
    run_suite_with_concurrency(suite, DEFAULT_EVALUATION_WORKER_CAP)
}

/// Executes all suite cases with an explicit worker cap.
///
/// When `worker_cap <= 1` or the suite has at most one case, execution runs
/// serially in the calling thread. When `worker_cap > 1`, work is distributed
/// across at most `min(worker_cap, MAX_EVALUATION_WORKER_CAP, suite.cases.len())`
/// threads and joined by stable index and identifier to guarantee bit-for-bit
/// identical results.
#[must_use]
pub fn run_suite_with_concurrency(suite: &EvaluationSuite, worker_cap: usize) -> EvaluationReport {
    let total_cases = suite.cases.len();
    if total_cases == 0 {
        return EvaluationReport {
            schema_version: EVALUATION_SCHEMA_VERSION,
            total: 0,
            passed: 0,
            categories: Vec::new(),
            transformation_coverage: TransformationCoverage {
                acceptable: 0,
                rewritten: 0,
            },
            failures: Vec::new(),
        };
    }

    let effective_workers = worker_cap
        .clamp(1, MAX_EVALUATION_WORKER_CAP)
        .min(total_cases);

    let mut outcomes: Vec<CaseExecutionOutcome> = if effective_workers <= 1 {
        suite
            .cases
            .iter()
            .enumerate()
            .map(|(index, case)| execute_single_case(index, case))
            .collect()
    } else {
        let next_index = AtomicUsize::new(0);
        let mut results: Vec<Option<CaseExecutionOutcome>> =
            (0..total_cases).map(|_| None).collect();

        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(effective_workers);
            for _ in 0..effective_workers {
                handles.push(scope.spawn(|| {
                    let mut worker_results = Vec::new();
                    loop {
                        let index = next_index.fetch_add(1, Ordering::Relaxed);
                        if index >= total_cases {
                            break;
                        }
                        let outcome = execute_single_case(index, &suite.cases[index]);
                        worker_results.push(outcome);
                    }
                    worker_results
                }));
            }

            for handle in handles {
                if let Ok(worker_results) = handle.join() {
                    for outcome in worker_results {
                        let idx = outcome.index;
                        if idx < total_cases {
                            results[idx] = Some(outcome);
                        }
                    }
                }
            }
        });

        results
            .into_iter()
            .enumerate()
            .map(|(index, opt)| {
                opt.unwrap_or_else(|| execute_single_case(index, &suite.cases[index]))
            })
            .collect()
    };

    outcomes.sort_by_key(|outcome| outcome.index);

    let mut categories = BTreeMap::<String, CategoryResult>::new();
    let mut failures = Vec::new();
    let mut transformation_coverage = TransformationCoverage {
        acceptable: 0,
        rewritten: 0,
    };

    for outcome in outcomes {
        let category = categories
            .entry(outcome.category.clone())
            .or_insert_with(|| CategoryResult {
                category: outcome.category,
                total: 0,
                passed: 0,
            });
        category.total += 1;
        if outcome.passed {
            category.passed += 1;
        }

        transformation_coverage.acceptable += outcome.acceptable_increment;
        transformation_coverage.rewritten += outcome.rewritten_increment;

        if let Some(failure) = outcome.failure {
            failures.push(failure);
        }
    }

    let passed = total_cases.saturating_sub(failures.len());

    EvaluationReport {
        schema_version: EVALUATION_SCHEMA_VERSION,
        total: total_cases,
        passed,
        categories: categories.into_values().collect(),
        transformation_coverage,
        failures,
    }
}

fn operational_failure(case: &EvaluationCase, error: &AppError) -> EvaluationFailure {
    let error = match error {
        AppError::CandidateTooLarge { .. } => "candidate_too_large",
        AppError::TextAdapter(_) => "text_adapter",
        AppError::Engine(_) => "engine",
        AppError::Protection(_) => "protection",
        AppError::Grounded(_) => "grounded",
        AppError::GroundedUnavailable => "grounded_unavailable",
        AppError::GroundedSelectionMismatch => "grounded_selection_mismatch",
        AppError::GroundedRuntimeUnavailable => "grounded_runtime_unavailable",
        AppError::GroundedRepository => "grounded_repository",
        AppError::ClaimExtraction(_) => "claim_extraction",
    };
    EvaluationFailure {
        id: case.id.clone(),
        category: case.category.clone(),
        expected_status: case.expected_status,
        actual_status: None,
        expected_reason: case.expected_reason,
        actual_reason: None,
        output_mismatch: false,
        error: Some(error.to_owned()),
    }
}
