use rewrite_model::{
    CandidateDeterministicReportSummaryV1, CandidateDeterministicTransformationCoverageV1,
};

use crate::EvaluationReport;
use crate::hybrid_scorecard::deterministic::DeterministicReportPairAggregate;

use super::{
    CandidateDeterministicCompilerError, CandidateDeterministicCompilerRelationship,
    portable_error, relationship_error,
};

pub(super) fn summarize(
    report: &EvaluationReport,
) -> Result<CandidateDeterministicReportSummaryV1, CandidateDeterministicCompilerError> {
    let total = u32::try_from(report.total)
        .map_err(|_| CandidateDeterministicCompilerError::ReportCountOverflow)?;
    let passed = u32::try_from(report.passed)
        .map_err(|_| CandidateDeterministicCompilerError::ReportCountOverflow)?;
    let acceptable = u32::try_from(report.transformation_coverage.acceptable)
        .map_err(|_| CandidateDeterministicCompilerError::ReportCountOverflow)?;
    let rewritten = u32::try_from(report.transformation_coverage.rewritten)
        .map_err(|_| CandidateDeterministicCompilerError::ReportCountOverflow)?;
    let coverage = CandidateDeterministicTransformationCoverageV1::new(acceptable, rewritten)
        .map_err(portable_error)?;
    CandidateDeterministicReportSummaryV1::new(total, passed, coverage).map_err(portable_error)
}

pub(super) fn validate_aggregate(
    aggregate: &DeterministicReportPairAggregate,
    summary_a: CandidateDeterministicReportSummaryV1,
    summary_b: CandidateDeterministicReportSummaryV1,
) -> Result<(), CandidateDeterministicCompilerError> {
    let total = (summary_a.total() as usize).checked_add(summary_b.total() as usize);
    let passed = (summary_a.passed() as usize).checked_add(summary_b.passed() as usize);
    let coverage = aggregate.transformation_coverage();
    let acceptable = (summary_a.transformation_coverage().acceptable() as usize)
        .checked_add(summary_b.transformation_coverage().acceptable() as usize);
    let rewritten = (summary_a.transformation_coverage().rewritten() as usize)
        .checked_add(summary_b.transformation_coverage().rewritten() as usize);
    if total != Some(aggregate.total())
        || passed != Some(aggregate.passed())
        || acceptable != Some(coverage.acceptable)
        || rewritten != Some(coverage.rewritten)
        || aggregate.success() != (aggregate.total() == aggregate.passed())
    {
        return Err(relationship_error(
            0,
            CandidateDeterministicCompilerRelationship::ReportClosure,
        ));
    }
    Ok(())
}
