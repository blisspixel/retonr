use std::{error::Error, fmt};

use rewrite_model::{
    CandidateJudgeJoinRecordV1, CandidateJudgeObservationBatchV1,
    CandidateJudgeResponseAggregateV1, GenerationSystemRecordV1, ManagedLocalJudgeReceiptRecordV1,
};
use rewrite_types::CancellationToken;

use crate::active_generation_qualification_subject::{
    ActiveGenerationQualificationBinding, ActiveGenerationQualificationSubject,
};

use super::managed_local_judge_receipt::{ManagedLocalJudgeReceipt, ManagedLocalJudgeReceiptView};
use super::managed_schedule_runner::{
    ManagedJudgeScheduleAuthorityFailures, ManagedJudgeScheduleExecutionAuthorityError,
};
use crate::candidate_judge_preparation::{
    CandidateJudgeJoinCompilationError, CandidateJudgeTriageCompilationError,
    CompiledCandidateJudgeTriage,
};
use crate::{CandidateJudgeRunnerHandoff, HybridScorecardReport};

mod settlement;
mod validation;
pub(crate) use settlement::JudgeSettlementView;
#[cfg(test)]
pub(crate) use settlement::synthetic_join;

#[cfg(test)]
use validation::validate_join_parts;
pub(super) use validation::{VerifiedCandidateJudgeJoinRelationshipError, validate_join_view};

/// Content-redacted primary failure while compiling an opaque candidate-judge join.
pub(in crate::local_ollama_managed_preflight::generation) enum CandidateJudgeJoinPrimaryError {
    /// Exact compatibility triage compilation failed.
    Triage(CandidateJudgeTriageCompilationError),
    /// Portable candidate-to-judge relationship compilation failed.
    Join(CandidateJudgeJoinCompilationError),
}

/// Public category for a failed fresh opaque-join revalidation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifiedCandidateJudgeJoinRevalidationErrorKind {
    /// Initial retained-authority validation failed.
    InitialAuthority,
    /// Initial and independent terminal retained-authority validation failed.
    InitialAndFinalAuthority,
    /// The retained inert join closure no longer matched the live view.
    Relationship,
    /// Independent terminal retained-authority validation failed.
    FinalAuthority,
    /// Relationship and independent terminal authority validation both failed.
    RelationshipAndFinalAuthority,
}

/// Content-redacted failure from fresh opaque-join revalidation.
pub struct VerifiedCandidateJudgeJoinRevalidationError {
    kind: VerifiedCandidateJudgeJoinRevalidationErrorKind,
    detail: VerifiedCandidateJudgeJoinRevalidationErrorDetail,
}

enum VerifiedCandidateJudgeJoinRevalidationErrorDetail {
    Initial(ManagedJudgeScheduleAuthorityFailures),
    InitialAndFinal {
        initial: ManagedJudgeScheduleAuthorityFailures,
        final_validation: ManagedJudgeScheduleAuthorityFailures,
    },
    Relationship,
    Final(ManagedJudgeScheduleAuthorityFailures),
    RelationshipAndFinal(ManagedJudgeScheduleAuthorityFailures),
}

impl VerifiedCandidateJudgeJoinRevalidationError {
    /// Returns the stable categorical failure without exposing live authority detail.
    #[must_use]
    pub const fn kind(&self) -> VerifiedCandidateJudgeJoinRevalidationErrorKind {
        self.kind
    }
}

impl fmt::Display for VerifiedCandidateJudgeJoinRevalidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "candidate judge join revalidation failed: {:?}",
            self.kind
        )
    }
}

impl fmt::Debug for VerifiedCandidateJudgeJoinRevalidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("VerifiedCandidateJudgeJoinRevalidationError");
        debug.field("kind", &self.kind);
        match &self.detail {
            VerifiedCandidateJudgeJoinRevalidationErrorDetail::Initial(failures)
            | VerifiedCandidateJudgeJoinRevalidationErrorDetail::Final(failures)
            | VerifiedCandidateJudgeJoinRevalidationErrorDetail::RelationshipAndFinal(failures) => {
                debug.field("authority_failures", failures);
            }
            VerifiedCandidateJudgeJoinRevalidationErrorDetail::InitialAndFinal {
                initial,
                final_validation,
            } => {
                debug
                    .field("initial", initial)
                    .field("final_validation", final_validation);
            }
            VerifiedCandidateJudgeJoinRevalidationErrorDetail::Relationship => {}
        }
        debug.finish_non_exhaustive()
    }
}

impl Error for VerifiedCandidateJudgeJoinRevalidationError {}

impl fmt::Display for CandidateJudgeJoinPrimaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Triage(_) => formatter.write_str("candidate judge triage compilation failed"),
            Self::Join(_) => formatter.write_str("candidate judge join compilation failed"),
        }
    }
}

impl fmt::Debug for CandidateJudgeJoinPrimaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeJoinPrimaryError")
            .field(
                "kind",
                &match self {
                    Self::Triage(_) => "triage",
                    Self::Join(_) => "join",
                },
            )
            .finish_non_exhaustive()
    }
}

impl Error for CandidateJudgeJoinPrimaryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Triage(source) => Some(source),
            Self::Join(source) => Some(source),
        }
    }
}

/// Content-redacted failure from the full opaque candidate-judge join bracket.
pub(in crate::local_ollama_managed_preflight::generation) enum VerifiedCandidateJudgeJoinCompilerError
{
    /// Initial retained-authority validation failed before compilation.
    InitialAuthority(ManagedJudgeScheduleAuthorityFailures),
    /// Initial and independent terminal authority validation both failed.
    InitialAndFinalAuthority {
        /// Initial retained-authority failure set.
        initial: ManagedJudgeScheduleAuthorityFailures,
        /// Independent terminal retained-authority failure set.
        final_validation: ManagedJudgeScheduleAuthorityFailures,
    },
    /// Triage or portable join compilation failed.
    Compilation(CandidateJudgeJoinPrimaryError),
    /// Mandatory independent terminal authority validation failed.
    FinalAuthority(ManagedJudgeScheduleAuthorityFailures),
    /// Compilation and independent terminal authority validation both failed.
    CompilationAndFinalAuthority {
        /// Primary compilation failure.
        compilation: Box<CandidateJudgeJoinPrimaryError>,
        /// Independent terminal retained-authority failure set.
        final_validation: ManagedJudgeScheduleAuthorityFailures,
    },
}

impl fmt::Display for VerifiedCandidateJudgeJoinCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitialAuthority(_) => {
                formatter.write_str("candidate judge join initial authority failed")
            }
            Self::InitialAndFinalAuthority { .. } => formatter
                .write_str("candidate judge join initial and final authority validation failed"),
            Self::Compilation(_) => formatter.write_str("candidate judge join compilation failed"),
            Self::FinalAuthority(_) => {
                formatter.write_str("candidate judge join final authority failed")
            }
            Self::CompilationAndFinalAuthority { .. } => formatter.write_str(
                "candidate judge join compilation and final authority validation failed",
            ),
        }
    }
}

impl fmt::Debug for VerifiedCandidateJudgeJoinCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("VerifiedCandidateJudgeJoinCompilerError");
        match self {
            Self::InitialAuthority(failures) => debug
                .field("kind", &"initial_authority")
                .field("failures", failures),
            Self::InitialAndFinalAuthority {
                initial,
                final_validation,
            } => debug
                .field("kind", &"initial_and_final_authority")
                .field("initial", initial)
                .field("final_validation", final_validation),
            Self::Compilation(_) => debug.field("kind", &"compilation"),
            Self::FinalAuthority(failures) => debug
                .field("kind", &"final_authority")
                .field("failures", failures),
            Self::CompilationAndFinalAuthority {
                final_validation, ..
            } => debug
                .field("kind", &"compilation_and_final_authority")
                .field("final_validation", final_validation),
        };
        debug.finish_non_exhaustive()
    }
}

impl Error for VerifiedCandidateJudgeJoinCompilerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Compilation(source) => Some(source),
            Self::CompilationAndFinalAuthority { compilation, .. } => Some(compilation.as_ref()),
            Self::InitialAuthority(_)
            | Self::InitialAndFinalAuthority { .. }
            | Self::FinalAuthority(_) => None,
        }
    }
}

/// Opaque eval-owned capability for one exact candidate-to-managed-judge join.
///
/// The value owns the durable receipt authority and content-free triage. Its inert
/// portable record cannot recreate either held live authority.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCandidateJudgeJoin;
///
/// fn clone_capability(value: VerifiedCandidateJudgeJoin<'_, '_, '_, '_>) {
///     let _forged = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCandidateJudgeJoin;
///
/// fn serialize_capability(value: &VerifiedCandidateJudgeJoin<'_, '_, '_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCandidateJudgeJoin;
///
/// fn construct_capability() {
///     let _forged = VerifiedCandidateJudgeJoin {
///         receipt: todo!(),
///         triage: todo!(),
///         record: todo!(),
///     };
/// }
/// ```
pub struct VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime> {
    active_binding: Option<ActiveGenerationQualificationBinding>,
    receipt: ManagedLocalJudgeReceipt<'store, 'records, 'model, 'runtime>,
    triage: CompiledCandidateJudgeTriage,
    record: CandidateJudgeJoinRecordV1,
}

pub(in crate::local_ollama_managed_preflight::generation) struct VerifiedCandidateJudgeJoinView<
    'borrow,
    'store,
> {
    pub(in crate::local_ollama_managed_preflight::generation) eval:
        &'borrow CandidateJudgeRunnerHandoff<'store>,
    pub(in crate::local_ollama_managed_preflight::generation) judge_system:
        &'borrow GenerationSystemRecordV1,
    pub(in crate::local_ollama_managed_preflight::generation) response_aggregate:
        &'borrow CandidateJudgeResponseAggregateV1,
    pub(in crate::local_ollama_managed_preflight::generation) observation_batch:
        &'borrow CandidateJudgeObservationBatchV1,
    pub(in crate::local_ollama_managed_preflight::generation) managed_receipt:
        &'borrow ManagedLocalJudgeReceiptRecordV1,
    pub(in crate::local_ollama_managed_preflight::generation) triage:
        &'borrow CompiledCandidateJudgeTriage,
    pub(in crate::local_ollama_managed_preflight::generation) record:
        &'borrow CandidateJudgeJoinRecordV1,
}

impl VerifiedCandidateJudgeJoin<'_, '_, '_, '_> {
    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        self.active_binding
            .as_ref()
            .is_some_and(|binding| subject.accepts(binding))
    }

    /// Returns the content-free portable candidate-judge join record.
    #[must_use]
    pub const fn record(&self) -> &CandidateJudgeJoinRecordV1 {
        &self.record
    }

    /// Returns the canonical non-authoritative compatibility triage report.
    #[must_use]
    pub const fn triage_report(&self) -> &HybridScorecardReport {
        self.triage.report()
    }

    /// Freshly revalidates every retained eval, app, preflight, model, and runtime
    /// authority plus the complete inert join closure.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted categorical error when any initial, relationship,
    /// cancellation, or mandatory independent terminal validation fails.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCandidateJudgeJoinRevalidationError> {
        self.with_revalidated_authorities(cancellation, |view| {
            validate_join_view(&view, cancellation)
        })
        .map_err(map_revalidation_error)
    }

    /// Revalidates the owned receipt authority around one scoped downstream use.
    pub(in crate::local_ollama_managed_preflight::generation) fn with_revalidated_authorities<
        T,
        E,
    >(
        &mut self,
        cancellation: &CancellationToken,
        use_authorities: impl FnOnce(VerifiedCandidateJudgeJoinView<'_, '_>) -> Result<T, E>,
    ) -> Result<T, ManagedJudgeScheduleExecutionAuthorityError<E>> {
        let triage = &self.triage;
        let join_record = &self.record;
        self.receipt
            .with_revalidated_authorities(cancellation, |receipt| {
                use_authorities(VerifiedCandidateJudgeJoinView {
                    eval: receipt.eval,
                    judge_system: receipt.judge_system,
                    response_aggregate: receipt.response_aggregate,
                    observation_batch: receipt.observation_batch,
                    managed_receipt: receipt.record,
                    triage,
                    record: join_record,
                })
            })
    }
}

impl fmt::Debug for VerifiedCandidateJudgeJoin<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCandidateJudgeJoin")
            .field(
                "candidate_judge_join_id",
                self.record.candidate_judge_join_id(),
            )
            .field("triage_report_digest", self.triage.relationship().digest())
            .finish_non_exhaustive()
    }
}

/// Compiler for one nonforgeable opaque candidate-judge join capability.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::local_ollama_managed_preflight::generation) struct VerifiedCandidateJudgeJoinCompiler;

impl VerifiedCandidateJudgeJoinCompiler {
    /// Consumes a durable receipt and derives triage plus the portable join inside
    /// its mandatory before-and-after retained-authority bracket.
    pub(in crate::local_ollama_managed_preflight::generation) fn compile<
        'store,
        'records,
        'model,
        'runtime,
    >(
        mut receipt: ManagedLocalJudgeReceipt<'store, 'records, 'model, 'runtime>,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>,
        VerifiedCandidateJudgeJoinCompilerError,
    > {
        let compiled = receipt
            .with_revalidated_authorities(cancellation, |view| {
                compile_from_view(&view, cancellation)
            })
            .map_err(map_authority_error)?;
        Ok(VerifiedCandidateJudgeJoin {
            active_binding: compiled.active_binding,
            receipt,
            triage: compiled.triage,
            record: compiled.record,
        })
    }
}

struct CompiledJoin {
    active_binding: Option<ActiveGenerationQualificationBinding>,
    triage: CompiledCandidateJudgeTriage,
    record: CandidateJudgeJoinRecordV1,
}

fn compile_from_view(
    view: &ManagedLocalJudgeReceiptView<'_, '_>,
    cancellation: &CancellationToken,
) -> Result<CompiledJoin, CandidateJudgeJoinPrimaryError> {
    compile_from_parts(
        view.eval,
        view.judge_system,
        view.response_aggregate,
        view.observation_batch,
        view.record,
        cancellation,
    )
}

fn compile_from_parts(
    eval: &CandidateJudgeRunnerHandoff<'_>,
    judge_system: &GenerationSystemRecordV1,
    response_aggregate: &CandidateJudgeResponseAggregateV1,
    observation_batch: &CandidateJudgeObservationBatchV1,
    managed_receipt: &ManagedLocalJudgeReceiptRecordV1,
    cancellation: &CancellationToken,
) -> Result<CompiledJoin, CandidateJudgeJoinPrimaryError> {
    compile_from_parts_with_post_triage(
        eval,
        judge_system,
        response_aggregate,
        observation_batch,
        managed_receipt,
        cancellation,
        || {},
    )
}

fn compile_from_parts_with_post_triage(
    eval: &CandidateJudgeRunnerHandoff<'_>,
    judge_system: &GenerationSystemRecordV1,
    response_aggregate: &CandidateJudgeResponseAggregateV1,
    observation_batch: &CandidateJudgeObservationBatchV1,
    managed_receipt: &ManagedLocalJudgeReceiptRecordV1,
    cancellation: &CancellationToken,
    post_triage: impl FnOnce(),
) -> Result<CompiledJoin, CandidateJudgeJoinPrimaryError> {
    let triage = eval
        .compile_compatibility_triage(observation_batch, cancellation)
        .map_err(CandidateJudgeJoinPrimaryError::Triage)?;
    post_triage();
    let record = eval
        .compile_join_record(
            judge_system,
            response_aggregate,
            observation_batch,
            managed_receipt,
            &triage,
            cancellation,
        )
        .map_err(CandidateJudgeJoinPrimaryError::Join)?;
    Ok(CompiledJoin {
        active_binding: eval.active_binding().cloned(),
        triage,
        record,
    })
}

fn map_authority_error(
    error: ManagedJudgeScheduleExecutionAuthorityError<CandidateJudgeJoinPrimaryError>,
) -> VerifiedCandidateJudgeJoinCompilerError {
    match error {
        ManagedJudgeScheduleExecutionAuthorityError::Initial(failures) => {
            VerifiedCandidateJudgeJoinCompilerError::InitialAuthority(failures)
        }
        ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
            initial,
            final_validation,
        } => VerifiedCandidateJudgeJoinCompilerError::InitialAndFinalAuthority {
            initial,
            final_validation,
        },
        ManagedJudgeScheduleExecutionAuthorityError::Callback(error) => {
            VerifiedCandidateJudgeJoinCompilerError::Compilation(error)
        }
        ManagedJudgeScheduleExecutionAuthorityError::Final(failures) => {
            VerifiedCandidateJudgeJoinCompilerError::FinalAuthority(failures)
        }
        ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
            callback,
            final_validation,
        } => VerifiedCandidateJudgeJoinCompilerError::CompilationAndFinalAuthority {
            compilation: Box::new(callback),
            final_validation,
        },
    }
}

fn map_revalidation_error(
    error: ManagedJudgeScheduleExecutionAuthorityError<VerifiedCandidateJudgeJoinRelationshipError>,
) -> VerifiedCandidateJudgeJoinRevalidationError {
    let (kind, detail) = match error {
        ManagedJudgeScheduleExecutionAuthorityError::Initial(failures) => (
            VerifiedCandidateJudgeJoinRevalidationErrorKind::InitialAuthority,
            VerifiedCandidateJudgeJoinRevalidationErrorDetail::Initial(failures),
        ),
        ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
            initial,
            final_validation,
        } => (
            VerifiedCandidateJudgeJoinRevalidationErrorKind::InitialAndFinalAuthority,
            VerifiedCandidateJudgeJoinRevalidationErrorDetail::InitialAndFinal {
                initial,
                final_validation,
            },
        ),
        ManagedJudgeScheduleExecutionAuthorityError::Callback(_error) => (
            VerifiedCandidateJudgeJoinRevalidationErrorKind::Relationship,
            VerifiedCandidateJudgeJoinRevalidationErrorDetail::Relationship,
        ),
        ManagedJudgeScheduleExecutionAuthorityError::Final(failures) => (
            VerifiedCandidateJudgeJoinRevalidationErrorKind::FinalAuthority,
            VerifiedCandidateJudgeJoinRevalidationErrorDetail::Final(failures),
        ),
        ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
            callback: _error,
            final_validation,
        } => (
            VerifiedCandidateJudgeJoinRevalidationErrorKind::RelationshipAndFinalAuthority,
            VerifiedCandidateJudgeJoinRevalidationErrorDetail::RelationshipAndFinal(
                final_validation,
            ),
        ),
    };
    VerifiedCandidateJudgeJoinRevalidationError { kind, detail }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(super) use tests::support as test_support;
