use rewrite_model::MAX_CANDIDATE_DETERMINISTIC_REPORT_JSON_BYTES;
use rewrite_types::{Digest, RewriteStatus};

use crate::{
    EVALUATION_SCHEMA_VERSION, EvaluationReport, EvaluationSuite, ExpectedOutput,
    MAX_EVALUATION_SUITE_BYTES, ReferenceJudgment, TransformationCoverage, parse_suite, run_suite,
};

use super::{HybridScorecardError, HybridScorecardPlan, JudgeObservationBatch};

mod bounded_json;
use bounded_json::encode_bounded_json;
mod exact_projection;
pub(crate) use exact_projection::{
    exact_projection_plan_digest_if_hard_gates_pass, run_exact_projection_scorecard,
};

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

/// Canonical, comparable pair of suites retained for pure deterministic execution.
pub(crate) struct ValidatedDeterministicSuitePair<'suite> {
    suite_a: &'suite EvaluationSuite,
    suite_b: &'suite EvaluationSuite,
    json_a: Vec<u8>,
    json_b: Vec<u8>,
}

/// Exact reports, canonical JSON, and checked aggregate from one validated pair.
pub(crate) struct DeterministicReportPairExecution {
    report_a: EvaluationReport,
    report_b: EvaluationReport,
    json_a: Vec<u8>,
    json_b: Vec<u8>,
}

/// Checked combined counters and disposition for two deterministic reports.
pub(crate) struct DeterministicReportPairAggregate {
    pub(crate) total: usize,
    pub(crate) passed: usize,
    pub(crate) transformation_coverage: TransformationCoverage,
    pub(crate) success: bool,
}

impl ValidatedDeterministicSuitePair<'_> {
    pub(crate) const fn candidate_a(&self) -> &EvaluationSuite {
        self.suite_a
    }

    pub(crate) const fn candidate_b(&self) -> &EvaluationSuite {
        self.suite_b
    }

    pub(crate) fn candidate_a_json(&self) -> &[u8] {
        &self.json_a
    }

    pub(crate) fn candidate_b_json(&self) -> &[u8] {
        &self.json_b
    }
}

impl DeterministicReportPairExecution {
    pub(crate) const fn candidate_a_report(&self) -> &EvaluationReport {
        &self.report_a
    }

    pub(crate) const fn candidate_b_report(&self) -> &EvaluationReport {
        &self.report_b
    }

    pub(crate) fn candidate_a_report_json(&self) -> &[u8] {
        &self.json_a
    }

    pub(crate) fn candidate_b_report_json(&self) -> &[u8] {
        &self.json_b
    }
}

impl DeterministicReportPairAggregate {
    pub(crate) const fn total(&self) -> usize {
        self.total
    }

    pub(crate) const fn passed(&self) -> usize {
        self.passed
    }

    pub(crate) const fn transformation_coverage(&self) -> TransformationCoverage {
        self.transformation_coverage
    }

    pub(crate) const fn success(&self) -> bool {
        self.success
    }
}

pub(crate) fn policy_digest() -> Digest {
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

pub(crate) fn suite_pair_digest(
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<Digest, HybridScorecardError> {
    let validated = validate_suite_pair(candidate_a, candidate_b)?;
    Ok(domain_digest(
        SUITE_PAIR_DOMAIN,
        &[validated.candidate_a_json(), validated.candidate_b_json()],
    ))
}

pub(super) fn run_deterministic_gates(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<DeterministicGateReceipt, HybridScorecardError> {
    let validated = validate_suite_pair(candidate_a, candidate_b)?;
    if plan.deterministic_policy_digest != policy_digest() {
        return Err(HybridScorecardError::DeterministicPolicyMismatch);
    }
    if plan.corpus_digest
        != domain_digest(
            SUITE_PAIR_DOMAIN,
            &[validated.candidate_a_json(), validated.candidate_b_json()],
        )
    {
        return Err(HybridScorecardError::CorpusMismatch);
    }
    validate_plan_relationships(plan, candidate_a, candidate_b)?;
    let execution = execute_report_pair(&validated)?;
    let aggregate = checked_report_pair_aggregate(&execution)?;
    Ok(DeterministicGateReceipt {
        report_digest: legacy_report_pair_digest(
            execution.candidate_a_report_json(),
            execution.candidate_b_report_json(),
        ),
        total: aggregate.total(),
        passed: aggregate.passed(),
        transformation_coverage: aggregate.transformation_coverage(),
        success: aggregate.success(),
    })
}

/// Validates, canonically encodes, and retains one comparable deterministic pair.
pub(crate) fn validate_suite_pair<'suite>(
    candidate_a: &'suite EvaluationSuite,
    candidate_b: &'suite EvaluationSuite,
) -> Result<ValidatedDeterministicSuitePair<'suite>, HybridScorecardError> {
    let primary_json = canonical_suite_bytes(candidate_a)?;
    let alternate_json = canonical_suite_bytes(candidate_b)?;
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
    Ok(ValidatedDeterministicSuitePair {
        suite_a: candidate_a,
        suite_b: candidate_b,
        json_a: primary_json,
        json_b: alternate_json,
    })
}

/// Encodes and round-trip validates one suite under the compatibility V1 JSON contract.
pub(crate) fn canonical_suite_bytes(
    suite: &EvaluationSuite,
) -> Result<Vec<u8>, HybridScorecardError> {
    let bytes = encode_bounded_json(suite, MAX_EVALUATION_SUITE_BYTES)
        .map_err(|_| HybridScorecardError::InvalidDeterministicSuites)?;
    let encoded = std::str::from_utf8(&bytes)
        .map_err(|_| HybridScorecardError::InvalidDeterministicSuites)?;
    let parsed =
        parse_suite(encoded).map_err(|_| HybridScorecardError::InvalidDeterministicSuites)?;
    if &parsed != suite {
        return Err(HybridScorecardError::InvalidDeterministicSuites);
    }
    Ok(bytes)
}

/// Runs both suites and returns their exact checked reports and aggregate.
pub(crate) fn execute_report_pair(
    suites: &ValidatedDeterministicSuitePair<'_>,
) -> Result<DeterministicReportPairExecution, HybridScorecardError> {
    let primary_report = run_suite(suites.candidate_a());
    let alternate_report = run_suite(suites.candidate_b());
    validate_report_consistency(&primary_report, suites.candidate_a().cases.len())?;
    validate_report_consistency(&alternate_report, suites.candidate_b().cases.len())?;
    let primary_json = encode_report_json(&primary_report)?;
    let alternate_json = encode_report_json(&alternate_report)?;
    Ok(DeterministicReportPairExecution {
        report_a: primary_report,
        report_b: alternate_report,
        json_a: primary_json,
        json_b: alternate_json,
    })
}

/// Combines the reports produced by one exact pair execution with checked arithmetic.
pub(crate) fn checked_report_pair_aggregate(
    execution: &DeterministicReportPairExecution,
) -> Result<DeterministicReportPairAggregate, HybridScorecardError> {
    let primary_report = execution.candidate_a_report();
    let alternate_report = execution.candidate_b_report();
    let total = checked_add(primary_report.total, alternate_report.total)?;
    let passed = checked_add(primary_report.passed, alternate_report.passed)?;
    let transformation_coverage = TransformationCoverage {
        acceptable: checked_add(
            primary_report.transformation_coverage.acceptable,
            alternate_report.transformation_coverage.acceptable,
        )?,
        rewritten: checked_add(
            primary_report.transformation_coverage.rewritten,
            alternate_report.transformation_coverage.rewritten,
        )?,
    };
    let success = primary_report.is_success() && alternate_report.is_success();
    Ok(DeterministicReportPairAggregate {
        total,
        passed,
        transformation_coverage,
        success,
    })
}

/// Encodes one deterministic report under the compatibility V1 JSON contract.
///
/// Encoding is a pure compatibility operation and grants no authority to a
/// caller-supplied report.
pub(crate) fn encode_report_json(
    report: &EvaluationReport,
) -> Result<Vec<u8>, HybridScorecardError> {
    encode_bounded_json(report, MAX_CANDIDATE_DETERMINISTIC_REPORT_JSON_BYTES)
        .map_err(|_| HybridScorecardError::InvalidDeterministicReport)
}

/// Preserves the exact legacy hybrid report-pair digest calculation.
pub(crate) fn legacy_report_pair_digest(
    primary_report_json: &[u8],
    alternate_report_json: &[u8],
) -> Digest {
    domain_digest(
        REPORT_PAIR_DOMAIN,
        &[primary_report_json, alternate_report_json],
    )
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

/// Checks report counters and coverage for internal consistency only.
///
/// Successful validation does not establish how a caller-supplied report was
/// produced and does not grant evaluation authority.
pub(crate) fn validate_report_consistency(
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
        CategoryResult, EVALUATION_SCHEMA_VERSION, EvaluationCase, EvaluationReport,
        EvaluationSuite, ExpectedOutput, ReferenceJudgment, TransformationCoverage,
    };
    use rewrite_types::RewriteStatus;

    use super::{
        DeterministicReportPairExecution, HybridScorecardError, canonical_suite_bytes,
        checked_report_pair_aggregate, execute_report_pair, legacy_report_pair_digest,
        policy_digest, validate_report_consistency, validate_suite_pair,
    };

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

    fn compatibility_suites() -> (EvaluationSuite, EvaluationSuite) {
        let case = EvaluationCase {
            id: "same-content".to_owned(),
            category: "identity".to_owned(),
            source: "No change".to_owned(),
            candidate: "No change".to_owned(),
            protected_terms: Vec::new(),
            reference_judgment: ReferenceJudgment::Identity,
            expected_status: RewriteStatus::UnchangedNoEligibleContent,
            expected_reason: None,
            expected_output: ExpectedOutput::Source,
        };
        let suite = EvaluationSuite {
            schema_version: EVALUATION_SCHEMA_VERSION,
            cases: vec![case],
        };
        (suite.clone(), suite)
    }

    #[test]
    fn rejects_empty_success_and_wrong_schema_receipts() {
        assert_eq!(
            validate_report_consistency(&successful_report(EVALUATION_SCHEMA_VERSION, 0), 1),
            Err(HybridScorecardError::InvalidDeterministicReport)
        );
        assert_eq!(
            validate_report_consistency(&successful_report(EVALUATION_SCHEMA_VERSION + 1, 1), 1,),
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

    #[test]
    fn pure_pair_execution_preserves_every_legacy_compatibility_vector() {
        let (candidate_a, candidate_b) = compatibility_suites();
        let expected_suite_json = concat!(
            r#"{"schema_version":2,"cases":[{"id":"same-content","category":"identity","#,
            r#""source":"No change","candidate":"No change","protected_terms":[],"#,
            r#""reference_judgment":"identity","expected_status":"#,
            r#""unchanged_no_eligible_content","expected_reason":null,"expected_output":"source"}]}"#,
        );
        assert_eq!(
            canonical_suite_bytes(&candidate_a).expect("suite encodes"),
            expected_suite_json.as_bytes()
        );
        let suites = validate_suite_pair(&candidate_a, &candidate_b).expect("pair validates");
        let execution = execute_report_pair(&suites).expect("pure pair executes");
        let expected_report_json = concat!(
            r#"{"schema_version":2,"total":1,"passed":1,"categories":[{"category":"#,
            r#""identity","total":1,"passed":1}],"transformation_coverage":{"#,
            r#""acceptable":0,"rewritten":0},"failures":[]}"#,
        );
        assert_eq!(
            execution.candidate_a_report_json(),
            expected_report_json.as_bytes()
        );
        assert_eq!(
            execution.candidate_b_report_json(),
            expected_report_json.as_bytes()
        );
        assert!(execution.candidate_a_report().is_success());
        assert!(execution.candidate_b_report().is_success());
        let aggregate = checked_report_pair_aggregate(&execution).expect("aggregate is checked");
        assert_eq!(aggregate.total(), 2);
        assert_eq!(aggregate.passed(), 2);
        assert_eq!(aggregate.transformation_coverage().acceptable, 0);
        assert_eq!(aggregate.transformation_coverage().rewritten, 0);
        assert!(aggregate.success());
        assert_eq!(
            super::suite_pair_digest(&candidate_a, &candidate_b)
                .expect("suite pair digest derives")
                .as_str(),
            "880f6e870661bee0c84c21bb355a2e9824a05a8b5af84a91660e8fdebd0df806"
        );
        assert_eq!(
            legacy_report_pair_digest(
                execution.candidate_a_report_json(),
                execution.candidate_b_report_json(),
            )
            .as_str(),
            "604fd19ed127e110babd1d220c2e9dda207dd841d58f2de21d80b53a5f36f3a1"
        );
    }

    #[test]
    fn pure_execution_does_not_require_judge_eligible_cases() {
        let (candidate_a, candidate_b) = compatibility_suites();
        assert_eq!(
            candidate_a.cases[0].reference_judgment,
            ReferenceJudgment::Identity
        );
        let suites = validate_suite_pair(&candidate_a, &candidate_b).expect("pair validates");
        let execution = execute_report_pair(&suites).expect("identity case executes");
        assert!(execution.candidate_a_report().is_success());
        assert!(execution.candidate_b_report().is_success());
    }

    #[test]
    fn report_validation_and_pair_aggregation_reject_inconsistent_or_overflowing_counts() {
        let mut invalid = successful_report(EVALUATION_SCHEMA_VERSION, 1);
        invalid.categories[0].passed = 2;
        assert_eq!(
            validate_report_consistency(&invalid, 1),
            Err(HybridScorecardError::InvalidDeterministicReport)
        );

        let mut invalid = successful_report(EVALUATION_SCHEMA_VERSION, 1);
        invalid.passed = 2;
        assert_eq!(
            validate_report_consistency(&invalid, 1),
            Err(HybridScorecardError::InvalidDeterministicReport)
        );

        let mut invalid = successful_report(EVALUATION_SCHEMA_VERSION, 1);
        invalid.transformation_coverage.rewritten = 1;
        assert_eq!(
            validate_report_consistency(&invalid, 1),
            Err(HybridScorecardError::InvalidDeterministicReport)
        );

        let mut invalid = successful_report(EVALUATION_SCHEMA_VERSION, 1);
        invalid.categories[0].total = 2;
        assert_eq!(
            validate_report_consistency(&invalid, 1),
            Err(HybridScorecardError::InvalidDeterministicReport)
        );

        let maximum = successful_report(EVALUATION_SCHEMA_VERSION, usize::MAX);
        let one = successful_report(EVALUATION_SCHEMA_VERSION, 1);
        validate_report_consistency(&maximum, usize::MAX)
            .expect("maximum report is internally consistent");
        let execution = DeterministicReportPairExecution {
            report_a: maximum,
            report_b: one,
            json_a: Vec::new(),
            json_b: Vec::new(),
        };
        assert!(matches!(
            checked_report_pair_aggregate(&execution),
            Err(HybridScorecardError::InvalidDeterministicReport)
        ));
    }
}
