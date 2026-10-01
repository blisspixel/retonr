//! Authoritative compilation of verified candidate batches into deterministic records.

use std::fmt;

use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationRecordV1Input,
    CandidateDeterministicEvaluationStatusV1, CandidateDeterministicReportRelationshipV1,
    CandidateGenerationReceiptSetV1, CandidateReceiptPairSetId,
    GenerationQualificationContractError, candidate_deterministic_policy_digest,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::generation_case_material::{
    GenerationDeterministicCaseContractError, VerifiedGenerationCaseMaterial,
    VerifiedGenerationCaseMaterialError,
};
use crate::hybrid_scorecard::{HybridScorecardError, deterministic};
use crate::{
    MAX_EVALUATION_SUITE_BYTES, VerifiedCandidateBatchSet, VerifiedCandidateBatchSetError,
};

mod identity;
pub(crate) use identity::derive_case_key_permutation;
use identity::derive_suite_pair_digest;
mod projection;
pub(crate) use projection::{project_case_pair, validate_projection_subset_bound};
use projection::{project_suites, validate_material_closure, validate_projection_bound};
mod report;
use report::{summarize, validate_aggregate};

/// Maximum cases accepted by one deterministic candidate compiler operation.
pub const MAX_CANDIDATE_DETERMINISTIC_COMPILER_CASES: usize =
    rewrite_model::MAX_GENERATION_SUITE_CASES;
/// Maximum retained projected payload for either compatibility suite.
pub const MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES: usize = MAX_EVALUATION_SUITE_BYTES;

/// Candidate position in the identity-significant ordered pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateDeterministicCompilerSide {
    /// First candidate batch set.
    CandidateA,
    /// Second candidate batch set.
    CandidateB,
}

/// Exact relationship rejected by the deterministic compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateDeterministicCompilerRelationship {
    /// The two receipt sets do not form one compatible distinct-system pair.
    ReceiptPair,
    /// A receipt set does not name the exact verified material suite.
    SuiteClosure,
    /// Semantic-order case identities do not close one-to-one.
    CaseClosure,
    /// A live selected candidate does not match its receipt-set selection.
    SelectionClosure,
    /// Case keys do not form a complete checked lexicographic permutation.
    CasePermutation,
    /// Eval and model code do not name the same frozen deterministic policy.
    DeterministicPolicy,
    /// Exact reports, summaries, or compatibility report identity disagreed.
    ReportClosure,
    /// The portable deterministic record rejected the checked inputs.
    EvaluationRecord,
}

/// Failure to compile two live verified candidate sets into one inert record.
#[derive(Error)]
pub enum CandidateDeterministicCompilerError {
    /// Cooperative cancellation was observed.
    #[error("candidate deterministic compilation was cancelled")]
    Cancelled,
    /// One retained candidate authority failed before or after compilation.
    #[error("candidate deterministic compiler batch authority failed for {side:?}")]
    CandidateBatchSet {
        /// Identity-significant candidate side.
        side: CandidateDeterministicCompilerSide,
        /// Exact typed authority failure, omitted from debug output.
        #[source]
        source: VerifiedCandidateBatchSetError,
    },
    /// One read-only authority bracket found one or more independent failures.
    #[error("candidate deterministic authority validation bracket failed")]
    AuthorityValidation {
        /// Candidate A failure, when observed before cancellation stopped checks.
        candidate_a: Option<Box<VerifiedCandidateBatchSetError>>,
        /// Candidate B failure, when observed before cancellation stopped checks.
        candidate_b: Option<Box<VerifiedCandidateBatchSetError>>,
        /// Case-material failure, when observed before cancellation stopped checks.
        case_material: Option<Box<VerifiedGenerationCaseMaterialError>>,
        /// Whether cancellation stopped remaining read-only checks.
        cancelled: bool,
    },
    /// The retained exact case-material authority failed.
    #[error("candidate deterministic compiler case material failed")]
    CaseMaterial {
        /// Exact typed material failure, omitted from debug output.
        #[source]
        source: VerifiedGenerationCaseMaterialError,
    },
    /// A fixed compiler relationship did not close.
    #[error(
        "candidate deterministic compiler relationship does not match at index {semantic_index}"
    )]
    Relationship {
        /// Semantic suite index, or zero for a set-level relationship.
        semantic_index: usize,
        /// Closed relationship category.
        relationship: CandidateDeterministicCompilerRelationship,
    },
    /// Projected suite payload would exceed a fixed compatibility ceiling.
    #[error("candidate deterministic projected suite byte limit exceeded")]
    ProjectedSuiteTooLarge,
    /// Exact source material could not be projected through its case contract.
    #[error("candidate deterministic case projection failed at index {semantic_index}")]
    CaseProjection {
        /// Semantic suite index of the rejected source or contract.
        semantic_index: usize,
        /// Typed projection failure, which carries no source bytes.
        #[source]
        source: GenerationDeterministicCaseContractError,
    },
    /// The frozen compatibility kernel rejected constructed suites or reports.
    #[error("candidate deterministic compatibility kernel failed")]
    CompatibilityKernel {
        /// Typed redacted compatibility failure.
        #[source]
        source: HybridScorecardError,
    },
    /// A checked count could not fit the portable version 1 contract.
    #[error("candidate deterministic report count exceeds the portable bound")]
    ReportCountOverflow,
    /// The portable model contract rejected compiler-derived values.
    #[error("candidate deterministic portable contract failed")]
    PortableContract {
        /// Typed portable contract failure, omitted from debug output.
        #[source]
        source: GenerationQualificationContractError,
    },
    /// Compilation and the mandatory final authority bracket both failed.
    #[error("candidate deterministic compilation and final authority validation both failed")]
    PrimaryAndFinalValidation {
        /// Primary compiler failure, retained without raw content in debug output.
        primary: Box<CandidateDeterministicCompilerError>,
        /// Final validation failure, retained without raw content in debug output.
        final_validation: Box<CandidateDeterministicCompilerError>,
    },
}

impl fmt::Debug for CandidateDeterministicCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CandidateDeterministicCompilerError");
        match self {
            Self::Cancelled => debug.field("kind", &"cancelled"),
            Self::CandidateBatchSet { side, .. } => debug
                .field("kind", &"candidate_batch_set")
                .field("side", side),
            Self::AuthorityValidation {
                candidate_a,
                candidate_b,
                case_material,
                cancelled,
            } => debug
                .field("kind", &"authority_validation")
                .field("candidate_a_failed", &candidate_a.is_some())
                .field("candidate_b_failed", &candidate_b.is_some())
                .field("case_material_failed", &case_material.is_some())
                .field("cancelled", cancelled),
            Self::CaseMaterial { .. } => debug.field("kind", &"case_material"),
            Self::Relationship {
                semantic_index,
                relationship,
            } => debug
                .field("kind", &"relationship")
                .field("semantic_index", semantic_index)
                .field("relationship", relationship),
            Self::ProjectedSuiteTooLarge => debug.field("kind", &"projected_suite_too_large"),
            Self::CaseProjection { semantic_index, .. } => debug
                .field("kind", &"case_projection")
                .field("semantic_index", semantic_index),
            Self::CompatibilityKernel { .. } => debug.field("kind", &"compatibility_kernel"),
            Self::ReportCountOverflow => debug.field("kind", &"report_count_overflow"),
            Self::PortableContract { .. } => debug.field("kind", &"portable_contract"),
            Self::PrimaryAndFinalValidation { .. } => {
                debug.field("kind", &"primary_and_final_validation")
            }
        };
        debug.finish_non_exhaustive()
    }
}

/// Compiles two live, ordered candidate authorities against exact case material.
///
/// The compiler revalidates all three authorities before and after use. It copies
/// only each policy-selected candidate text, reads each exact source once, runs the
/// frozen deterministic compatibility kernel once, and returns only an inert
/// portable record. A record whose status is `Failed` remains a valid result; it
/// does not authorize later judge execution.
///
/// # Errors
///
/// Returns [`CandidateDeterministicCompilerError`] for cancellation, authority
/// drift, incompatible receipts, incomplete closure, excessive projected bytes,
/// invalid source material, kernel failure, or portable-contract disagreement.
pub fn compile_candidate_deterministic_evaluation(
    candidate_a: &VerifiedCandidateBatchSet,
    candidate_b: &VerifiedCandidateBatchSet,
    material: &VerifiedGenerationCaseMaterial<'_>,
    cancellation: &CancellationToken,
) -> Result<CandidateDeterministicEvaluationRecordV1, CandidateDeterministicCompilerError> {
    compile_candidate_deterministic_evidence(candidate_a, candidate_b, material, cancellation)
        .map(|evidence| evidence.record)
}

pub(crate) struct CompiledCandidateDeterministicEvidence {
    pub(crate) record: CandidateDeterministicEvaluationRecordV1,
    pub(crate) reports: CandidateDeterministicReportRelationshipV1,
}

pub(crate) fn compile_candidate_deterministic_evidence(
    candidate_a: &VerifiedCandidateBatchSet,
    candidate_b: &VerifiedCandidateBatchSet,
    material: &VerifiedGenerationCaseMaterial<'_>,
    cancellation: &CancellationToken,
) -> Result<CompiledCandidateDeterministicEvidence, CandidateDeterministicCompilerError> {
    revalidate_all(candidate_a, candidate_b, material, cancellation)?;
    let primary =
        compile_after_initial_validation(candidate_a, candidate_b, material, cancellation);
    let final_validation = revalidate_all(candidate_a, candidate_b, material, cancellation);
    match (primary, final_validation) {
        (Ok(record), Ok(())) => Ok(record),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(_), Err(final_validation)) => Err(final_validation),
        (Err(primary), Err(final_validation)) => Err(
            CandidateDeterministicCompilerError::PrimaryAndFinalValidation {
                primary: Box::new(primary),
                final_validation: Box::new(final_validation),
            },
        ),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the compiler keeps one explicit linear authority-to-record transaction"
)]
fn compile_after_initial_validation(
    candidate_a: &VerifiedCandidateBatchSet,
    candidate_b: &VerifiedCandidateBatchSet,
    material: &VerifiedGenerationCaseMaterial<'_>,
    cancellation: &CancellationToken,
) -> Result<CompiledCandidateDeterministicEvidence, CandidateDeterministicCompilerError> {
    ensure_active(cancellation)?;
    if deterministic::policy_digest() != candidate_deterministic_policy_digest() {
        return Err(relationship_error(
            0,
            CandidateDeterministicCompilerRelationship::DeterministicPolicy,
        ));
    }

    let receipt_a = receipt_set(
        candidate_a,
        CandidateDeterministicCompilerSide::CandidateA,
        cancellation,
    )?;
    let receipt_b = receipt_set(
        candidate_b,
        CandidateDeterministicCompilerSide::CandidateB,
        cancellation,
    )?;
    let pair_id =
        CandidateReceiptPairSetId::from_receipt_sets(receipt_a, receipt_b).map_err(|_| {
            relationship_error(0, CandidateDeterministicCompilerRelationship::ReceiptPair)
        })?;
    validate_material_closure(receipt_a, receipt_b, material)?;

    let selected_a = selected_candidates(
        candidate_a,
        receipt_a,
        CandidateDeterministicCompilerSide::CandidateA,
        cancellation,
    )?;
    let selected_b = selected_candidates(
        candidate_b,
        receipt_b,
        CandidateDeterministicCompilerSide::CandidateB,
        cancellation,
    )?;
    validate_projection_bound(material, &selected_a, &selected_b)?;
    let (suite_a, suite_b, permutation) =
        project_suites(material, &selected_a, &selected_b, cancellation)?;
    ensure_active(cancellation)?;

    let suites = deterministic::validate_suite_pair(&suite_a, &suite_b).map_err(kernel_error)?;
    let suite_pair_digest = derive_suite_pair_digest(
        material.suite().suite_manifest_id().digest(),
        material.case_material_set_digest(),
        &permutation,
        suites.candidate_a_json(),
        suites.candidate_b_json(),
    )?;
    let execution = deterministic::execute_report_pair(&suites).map_err(kernel_error)?;
    let aggregate =
        deterministic::checked_report_pair_aggregate(&execution).map_err(kernel_error)?;
    let summary_a = summarize(execution.candidate_a_report())?;
    let summary_b = summarize(execution.candidate_b_report())?;
    validate_aggregate(&aggregate, summary_a, summary_b)?;
    let reports = CandidateDeterministicReportRelationshipV1::new(
        execution.candidate_a_report_json(),
        execution.candidate_b_report_json(),
        summary_a,
        summary_b,
    )
    .map_err(portable_error)?;
    if reports.report_pair_digest()
        != &deterministic::legacy_report_pair_digest(
            execution.candidate_a_report_json(),
            execution.candidate_b_report_json(),
        )
    {
        return Err(relationship_error(
            0,
            CandidateDeterministicCompilerRelationship::ReportClosure,
        ));
    }
    let record = CandidateDeterministicEvaluationRecordV1::new(
        receipt_a,
        receipt_b,
        CandidateDeterministicEvaluationRecordV1Input {
            case_material_set_digest: material.case_material_set_digest(),
            suite_pair_digest: &suite_pair_digest,
            report_relationship: &reports,
        },
    )
    .map_err(portable_error)?;
    if record.candidate_receipt_pair_set_id() != &pair_id
        || record.deterministic_policy_digest() != &deterministic::policy_digest()
        || usize::try_from(record.total()) != Ok(aggregate.total())
        || usize::try_from(record.passed()) != Ok(aggregate.passed())
        || usize::try_from(record.transformation_coverage().acceptable())
            != Ok(aggregate.transformation_coverage().acceptable)
        || usize::try_from(record.transformation_coverage().rewritten())
            != Ok(aggregate.transformation_coverage().rewritten)
        || record.status()
            != if aggregate.success() {
                CandidateDeterministicEvaluationStatusV1::Passed
            } else {
                CandidateDeterministicEvaluationStatusV1::Failed
            }
    {
        return Err(relationship_error(
            0,
            CandidateDeterministicCompilerRelationship::EvaluationRecord,
        ));
    }
    ensure_active(cancellation)?;
    Ok(CompiledCandidateDeterministicEvidence { record, reports })
}

fn revalidate_all(
    candidate_a: &VerifiedCandidateBatchSet,
    candidate_b: &VerifiedCandidateBatchSet,
    material: &VerifiedGenerationCaseMaterial<'_>,
    cancellation: &CancellationToken,
) -> Result<(), CandidateDeterministicCompilerError> {
    ensure_active(cancellation)?;
    let mut failure_a = candidate_a.revalidate(cancellation).err().map(Box::new);
    if cancellation.is_cancelled() {
        return authority_validation_result(failure_a, None, None, true);
    }
    let failure_b = candidate_b.revalidate(cancellation).err().map(Box::new);
    if cancellation.is_cancelled() {
        return authority_validation_result(failure_a.take(), failure_b, None, true);
    }
    let failure_material = material.revalidate(cancellation).err().map(Box::new);
    authority_validation_result(
        failure_a,
        failure_b,
        failure_material,
        cancellation.is_cancelled(),
    )
}

fn authority_validation_result(
    candidate_a: Option<Box<VerifiedCandidateBatchSetError>>,
    candidate_b: Option<Box<VerifiedCandidateBatchSetError>>,
    case_material: Option<Box<VerifiedGenerationCaseMaterialError>>,
    cancelled: bool,
) -> Result<(), CandidateDeterministicCompilerError> {
    if candidate_a.is_none() && candidate_b.is_none() && case_material.is_none() && !cancelled {
        Ok(())
    } else {
        Err(CandidateDeterministicCompilerError::AuthorityValidation {
            candidate_a,
            candidate_b,
            case_material,
            cancelled,
        })
    }
}

fn receipt_set<'a>(
    set: &'a VerifiedCandidateBatchSet,
    side: CandidateDeterministicCompilerSide,
    cancellation: &CancellationToken,
) -> Result<&'a CandidateGenerationReceiptSetV1, CandidateDeterministicCompilerError> {
    set.receipt_set(cancellation)
        .map_err(|source| batch_error(side, source))
}

fn selected_candidates<'a>(
    set: &'a VerifiedCandidateBatchSet,
    receipts: &CandidateGenerationReceiptSetV1,
    side: CandidateDeterministicCompilerSide,
    cancellation: &CancellationToken,
) -> Result<Vec<&'a rewrite_inference::GenerationCandidate>, CandidateDeterministicCompilerError> {
    let candidates = set
        .selected_candidates(cancellation)
        .map_err(|source| batch_error(side, source))?;
    if candidates.len() != receipts.entries().len() {
        return Err(relationship_error(
            0,
            CandidateDeterministicCompilerRelationship::SelectionClosure,
        ));
    }
    candidates
        .into_iter()
        .zip(receipts.entries())
        .enumerate()
        .map(|(semantic_index, (candidate, entry))| {
            if candidate.ordinal != entry.selected_ordinal() {
                return Err(relationship_error(
                    semantic_index,
                    CandidateDeterministicCompilerRelationship::SelectionClosure,
                ));
            }
            Ok(candidate)
        })
        .collect()
}

pub(super) fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), CandidateDeterministicCompilerError> {
    if cancellation.is_cancelled() {
        Err(CandidateDeterministicCompilerError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn relationship_error(
    semantic_index: usize,
    relationship: CandidateDeterministicCompilerRelationship,
) -> CandidateDeterministicCompilerError {
    CandidateDeterministicCompilerError::Relationship {
        semantic_index,
        relationship,
    }
}

fn batch_error(
    side: CandidateDeterministicCompilerSide,
    source: VerifiedCandidateBatchSetError,
) -> CandidateDeterministicCompilerError {
    CandidateDeterministicCompilerError::CandidateBatchSet { side, source }
}

fn kernel_error(source: HybridScorecardError) -> CandidateDeterministicCompilerError {
    CandidateDeterministicCompilerError::CompatibilityKernel { source }
}

fn portable_error(
    source: GenerationQualificationContractError,
) -> CandidateDeterministicCompilerError {
    CandidateDeterministicCompilerError::PortableContract { source }
}

#[cfg(test)]
#[path = "candidate_deterministic_compiler/tests.rs"]
mod tests;
