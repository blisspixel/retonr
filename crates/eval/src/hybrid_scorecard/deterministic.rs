use rewrite_types::{Digest, RewriteStatus};

use crate::{
    EVALUATION_SCHEMA_VERSION, EvaluationReport, EvaluationSuite, ExpectedOutput,
    MAX_EVALUATION_SUITE_BYTES, ReferenceJudgment, TransformationCoverage, parse_suite, run_suite,
};

use super::{HybridScorecardError, HybridScorecardPlan, JudgeObservationBatch};

const SUITE_PAIR_DOMAIN: &[u8] = b"retonr:hybrid-scorecard-suite-pair:v1\0";
const POLICY_DOMAIN: &[u8] = b"retonr:hybrid-scorecard-deterministic-policy:v1\0";
const PLAN_DOMAIN: &[u8] = b"retonr:hybrid-scorecard-plan:v1\0";
const REPORT_PAIR_DOMAIN: &[u8] = b"retonr:hybrid-scorecard-report-pair:v1\0";
const OBSERVATION_BATCH_DOMAIN: &[u8] = b"retonr:hybrid-scorecard-observation-batch:v1\0";

pub(super) struct DeterministicGateReceipt {
    pub(super) report_digest: Digest,
    pub(super) total: usize,
    pub(super) passed: usize,
    pub(super) transformation_coverage: TransformationCoverage,
    pub(super) success: bool,
}

pub(super) fn policy_digest() -> Digest {
    Digest::sha256(POLICY_DOMAIN)
}

pub(super) fn plan_digest(plan: &HybridScorecardPlan) -> Result<Digest, HybridScorecardError> {
    let bytes = serde_json::to_vec(plan).map_err(|_| HybridScorecardError::InvalidPlan)?;
    Ok(domain_digest(PLAN_DOMAIN, &[&bytes]))
}

pub(super) fn observation_batch_digest(
    batch: &JudgeObservationBatch,
) -> Result<Digest, HybridScorecardError> {
    let bytes = serde_json::to_vec(batch).map_err(|_| HybridScorecardError::InvalidObservations)?;
    Ok(domain_digest(OBSERVATION_BATCH_DOMAIN, &[&bytes]))
}

pub(super) fn suite_pair_digest(
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<Digest, HybridScorecardError> {
    let (primary_bytes, alternate_bytes) = validate_suites(candidate_a, candidate_b)?;
    Ok(domain_digest(
        SUITE_PAIR_DOMAIN,
        &[&primary_bytes, &alternate_bytes],
    ))
}

pub(super) fn run_deterministic_gates(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<DeterministicGateReceipt, HybridScorecardError> {
    let (primary_bytes, alternate_bytes) = validate_suites(candidate_a, candidate_b)?;
    if plan.deterministic_policy_digest != policy_digest() {
        return Err(HybridScorecardError::DeterministicPolicyMismatch);
    }
    if plan.corpus_digest != domain_digest(SUITE_PAIR_DOMAIN, &[&primary_bytes, &alternate_bytes]) {
        return Err(HybridScorecardError::CorpusMismatch);
    }
    validate_plan_relationships(plan, candidate_a, candidate_b)?;

    let report_a = run_suite(candidate_a);
    let report_b = run_suite(candidate_b);
    validate_report(&report_a, candidate_a.cases.len())?;
    validate_report(&report_b, candidate_b.cases.len())?;
    let primary_report_bytes = serde_json::to_vec(&report_a)
        .map_err(|_| HybridScorecardError::InvalidDeterministicReport)?;
    let alternate_report_bytes = serde_json::to_vec(&report_b)
        .map_err(|_| HybridScorecardError::InvalidDeterministicReport)?;

    Ok(DeterministicGateReceipt {
        report_digest: domain_digest(
            REPORT_PAIR_DOMAIN,
            &[&primary_report_bytes, &alternate_report_bytes],
        ),
        total: checked_add(report_a.total, report_b.total)?,
        passed: checked_add(report_a.passed, report_b.passed)?,
        transformation_coverage: TransformationCoverage {
            acceptable: checked_add(
                report_a.transformation_coverage.acceptable,
                report_b.transformation_coverage.acceptable,
            )?,
            rewritten: checked_add(
                report_a.transformation_coverage.rewritten,
                report_b.transformation_coverage.rewritten,
            )?,
        },
        success: report_a.is_success() && report_b.is_success(),
    })
}

fn validate_suites(
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<(Vec<u8>, Vec<u8>), HybridScorecardError> {
    let primary_bytes = canonical_suite_bytes(candidate_a)?;
    let alternate_bytes = canonical_suite_bytes(candidate_b)?;
    if candidate_a.cases.is_empty()
        || candidate_a.cases.len() != candidate_b.cases.len()
        || !strictly_ordered(candidate_a)
        || !strictly_ordered(candidate_b)
    {
        return Err(HybridScorecardError::InvalidDeterministicSuites);
    }
    for (case_a, case_b) in candidate_a.cases.iter().zip(&candidate_b.cases) {
        if case_a.id != case_b.id
            || case_a.category != case_b.category
            || case_a.source != case_b.source
            || case_a.protected_terms != case_b.protected_terms
            || case_a.reference_judgment != case_b.reference_judgment
        {
            return Err(HybridScorecardError::InvalidDeterministicSuites);
        }
    }
    Ok((primary_bytes, alternate_bytes))
}

fn canonical_suite_bytes(suite: &EvaluationSuite) -> Result<Vec<u8>, HybridScorecardError> {
    let bytes =
        serde_json::to_vec(suite).map_err(|_| HybridScorecardError::InvalidDeterministicSuites)?;
    if bytes.len() > MAX_EVALUATION_SUITE_BYTES {
        return Err(HybridScorecardError::InvalidDeterministicSuites);
    }
    let encoded = std::str::from_utf8(&bytes)
        .map_err(|_| HybridScorecardError::InvalidDeterministicSuites)?;
    let parsed =
        parse_suite(encoded).map_err(|_| HybridScorecardError::InvalidDeterministicSuites)?;
    if &parsed != suite {
        return Err(HybridScorecardError::InvalidDeterministicSuites);
    }
    Ok(bytes)
}

fn strictly_ordered(suite: &EvaluationSuite) -> bool {
    suite.cases.windows(2).all(|pair| pair[0].id < pair[1].id)
}

fn validate_plan_relationships(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<(), HybridScorecardError> {
    if plan.cases.len() != candidate_a.cases.len() {
        return Err(HybridScorecardError::CorpusMismatch);
    }
    for ((planned, case_a), case_b) in plan
        .cases
        .iter()
        .zip(&candidate_a.cases)
        .zip(&candidate_b.cases)
    {
        if planned.id != case_a.id
            || planned.id != case_b.id
            || planned.source_digest != Digest::sha256(case_a.source.as_bytes())
            || planned.candidate_a_digest != Digest::sha256(case_a.candidate.as_bytes())
            || planned.candidate_b_digest != Digest::sha256(case_b.candidate.as_bytes())
            || !judge_eligible(case_a)
            || !judge_eligible(case_b)
        {
            return Err(HybridScorecardError::CorpusMismatch);
        }
    }
    Ok(())
}

fn judge_eligible(case: &crate::EvaluationCase) -> bool {
    case.reference_judgment == ReferenceJudgment::Acceptable
        && case.expected_status == RewriteStatus::Rewritten
        && case.expected_reason.is_none()
        && case.expected_output == ExpectedOutput::Candidate
}

fn validate_report(
    report: &EvaluationReport,
    expected_total: usize,
) -> Result<(), HybridScorecardError> {
    let mut category_total = 0_usize;
    let mut category_passed = 0_usize;
    for category in &report.categories {
        if category.passed > category.total {
            return Err(HybridScorecardError::InvalidDeterministicReport);
        }
        category_total = checked_add(category_total, category.total)?;
        category_passed = checked_add(category_passed, category.passed)?;
    }
    if report.schema_version != EVALUATION_SCHEMA_VERSION
        || report.total != expected_total
        || report.passed > report.total
        || report.failures.len() != report.total.saturating_sub(report.passed)
        || report.transformation_coverage.rewritten > report.transformation_coverage.acceptable
        || category_total != report.total
        || category_passed != report.passed
    {
        return Err(HybridScorecardError::InvalidDeterministicReport);
    }
    Ok(())
}

fn checked_add(left: usize, right: usize) -> Result<usize, HybridScorecardError> {
    left.checked_add(right)
        .ok_or(HybridScorecardError::InvalidDeterministicReport)
}

fn domain_digest(domain: &[u8], fields: &[&[u8]]) -> Digest {
    let mut bytes = Vec::with_capacity(
        domain.len()
            + fields
                .iter()
                .map(|field| 8_usize.saturating_add(field.len()))
                .sum::<usize>(),
    );
    bytes.extend_from_slice(domain);
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field);
    }
    Digest::sha256(&bytes)
}

#[cfg(test)]
mod tests {
    use crate::{
        CategoryResult, EVALUATION_SCHEMA_VERSION, EvaluationReport, TransformationCoverage,
    };

    use super::{HybridScorecardError, policy_digest, validate_report};

    fn successful_report(schema_version: u32, total: usize) -> EvaluationReport {
        EvaluationReport {
            schema_version,
            total,
            passed: total,
            categories: if total == 0 {
                Vec::new()
            } else {
                vec![CategoryResult {
                    category: "fixture".to_owned(),
                    total,
                    passed: total,
                }]
            },
            transformation_coverage: TransformationCoverage {
                acceptable: 0,
                rewritten: 0,
            },
            failures: Vec::new(),
        }
    }

    #[test]
    fn rejects_empty_success_and_wrong_schema_receipts() {
        assert_eq!(
            validate_report(&successful_report(EVALUATION_SCHEMA_VERSION, 0), 1),
            Err(HybridScorecardError::InvalidDeterministicReport)
        );
        assert_eq!(
            validate_report(&successful_report(EVALUATION_SCHEMA_VERSION + 1, 1), 1),
            Err(HybridScorecardError::InvalidDeterministicReport)
        );
    }

    #[test]
    fn deterministic_policy_identity_is_frozen() {
        assert_eq!(
            policy_digest(),
            rewrite_types::Digest::from_sha256_hex(
                "761e147db1918db3712cb203cfd77e8a002efbcae5479be99f4364d758a938da"
            )
            .expect("golden policy digest")
        );
    }
}
