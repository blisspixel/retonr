use std::{error::Error, fmt};

use rewrite_app::{
    MANAGED_JUDGE_PREFLIGHT_OBSERVER_BINDING_DOMAIN, ReleasedManagedJudgeEffectivePackageV2,
    effective_runtime_state_observation::MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT,
};
use rewrite_model::{
    CandidateJudgeObservationBatchV1, CandidateJudgeResponseAggregateV1,
    GenerationQualificationContractError, GenerationSystemRecordV1,
    ManagedLocalJudgeReceiptRecordV1, ManagedLocalJudgeReceiptRecordV1Input,
    ManagedLocalJudgeReceiptRecordV1Relations,
};
use rewrite_runtime_attestor::RetainedTcpConnectionEvidence;
use rewrite_types::{CancellationToken, Digest};

use super::managed_schedule_runner::{
    ManagedJudgeScheduleAuthorityFailures, ManagedJudgeScheduleExecutionAuthorityError,
    ManagedJudgeScheduleExecutionView, VerifiedManagedJudgeScheduleExecution,
};
use crate::{
    CandidateJudgeRunnerHandoff, LocalOllamaManagedPreflightOutcome,
    LocalOllamaManagedPreflightReport,
};

#[cfg(test)]
mod offline_test_authority;
#[cfg(test)]
use offline_test_authority::ExactOfflineReceiptAuthority;

const PREFLIGHT_CONNECTION_SPAN_DOMAIN: &[u8] = b"retonr:managed-judge-connection-span:v1";

/// Closed relationship rejected by the managed local-judge receipt compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_ollama_managed_preflight::generation) enum ManagedLocalJudgeReceiptRelationship
{
    /// The app-owned and eval-owned portable judge closures differed.
    PortableJudge,
    /// Schedule, response, observation, or app authority counts differed.
    AttemptCount,
    /// The full managed preflight and app-owned preflight observer did not close.
    ManagedPreflight,
}

/// Content-redacted failure from durable managed local-judge receipt compilation.
pub(in crate::local_ollama_managed_preflight::generation) enum ManagedLocalJudgeReceiptCompilerError
{
    /// Initial retained-authority validation failed before compilation.
    InitialAuthority(ManagedJudgeScheduleAuthorityFailures),
    /// Initial and mandatory terminal authority validation both failed.
    InitialAndFinalAuthority {
        /// Initial retained-authority failure set.
        initial: ManagedJudgeScheduleAuthorityFailures,
        /// Independent terminal retained-authority failure set.
        final_validation: ManagedJudgeScheduleAuthorityFailures,
    },
    /// One exact compiler relationship differed.
    Relationship(ManagedLocalJudgeReceiptRelationship),
    /// The portable record contract rejected owner-derived fields.
    Portable(GenerationQualificationContractError),
    /// Mandatory terminal retained-authority validation failed.
    FinalAuthority(ManagedJudgeScheduleAuthorityFailures),
    /// Compilation and mandatory terminal validation both failed.
    CompilationAndFinalAuthority {
        /// Primary compilation failure.
        compilation: Box<ManagedLocalJudgeReceiptCompilerError>,
        /// Independent terminal authority-validation failure.
        final_validation: ManagedJudgeScheduleAuthorityFailures,
    },
}

impl fmt::Display for ManagedLocalJudgeReceiptCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitialAuthority(_) => {
                formatter.write_str("managed local-judge receipt initial authority failed")
            }
            Self::InitialAndFinalAuthority { .. } => formatter.write_str(
                "managed local-judge receipt initial and final authority validation failed",
            ),
            Self::Relationship(relationship) => write!(
                formatter,
                "managed local-judge receipt relationship is invalid: {relationship:?}"
            ),
            Self::Portable(_) => {
                formatter.write_str("managed local-judge receipt portable contract failed")
            }
            Self::FinalAuthority(_) => {
                formatter.write_str("managed local-judge receipt final authority failed")
            }
            Self::CompilationAndFinalAuthority { .. } => formatter.write_str(
                "managed local-judge receipt compilation and final authority validation failed",
            ),
        }
    }
}

impl fmt::Debug for ManagedLocalJudgeReceiptCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("ManagedLocalJudgeReceiptCompilerError");
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
            Self::Relationship(relationship) => debug
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::Portable(_) => debug.field("kind", &"portable"),
            Self::FinalAuthority(failures) => debug
                .field("kind", &"final_authority")
                .field("failures", failures),
            Self::CompilationAndFinalAuthority {
                compilation,
                final_validation,
            } => {
                let _ = compilation;
                debug
                    .field("kind", &"compilation_and_final_authority")
                    .field("final_validation", final_validation)
            }
        };
        debug.finish_non_exhaustive()
    }
}

impl Error for ManagedLocalJudgeReceiptCompilerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Portable(source) => Some(source),
            Self::CompilationAndFinalAuthority { compilation, .. } => Some(compilation.as_ref()),
            Self::InitialAuthority(_)
            | Self::InitialAndFinalAuthority { .. }
            | Self::Relationship(_)
            | Self::FinalAuthority(_) => None,
        }
    }
}

/// Eval-owned durable receipt capability over one exact successful execution.
///
/// The owned execution retains every live authority needed for later revalidation.
/// The portable record alone cannot recreate this value.
pub(in crate::local_ollama_managed_preflight::generation) struct ManagedLocalJudgeReceipt<
    'store,
    'records,
    'model,
    'runtime,
> {
    authority: ManagedLocalJudgeReceiptAuthority<'store, 'records, 'model, 'runtime>,
    record: ManagedLocalJudgeReceiptRecordV1,
}

enum ManagedLocalJudgeReceiptAuthority<'store, 'records, 'model, 'runtime> {
    Managed(Box<VerifiedManagedJudgeScheduleExecution<'store, 'records, 'model, 'runtime>>),
    #[cfg(test)]
    ExactOffline(Box<ExactOfflineReceiptAuthority<'store>>),
}

pub(in crate::local_ollama_managed_preflight::generation) struct ManagedLocalJudgeReceiptView<
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
    pub(in crate::local_ollama_managed_preflight::generation) input:
        ManagedLocalJudgeReceiptRecordV1Input,
    pub(in crate::local_ollama_managed_preflight::generation) record:
        &'borrow ManagedLocalJudgeReceiptRecordV1,
}

impl ManagedLocalJudgeReceipt<'_, '_, '_, '_> {
    pub(in crate::local_ollama_managed_preflight::generation) fn with_revalidated_authorities<
        T,
        E,
    >(
        &mut self,
        cancellation: &CancellationToken,
        use_authorities: impl FnOnce(ManagedLocalJudgeReceiptView<'_, '_>) -> Result<T, E>,
    ) -> Result<T, ManagedJudgeScheduleExecutionAuthorityError<E>> {
        let record = &self.record;
        match &mut self.authority {
            ManagedLocalJudgeReceiptAuthority::Managed(execution) => execution
                .with_revalidated_authorities(cancellation, |execution| {
                    use_authorities(ManagedLocalJudgeReceiptView {
                        eval: execution.eval,
                        judge_system: execution.released_package.judge_system(),
                        response_aggregate: execution.response_aggregate,
                        observation_batch: execution.observation_batch,
                        input: execution.released_package.receipt_input(
                            execution.managed_preflight.report().binding_digest.clone(),
                        ),
                        record,
                    })
                }),
            #[cfg(test)]
            ManagedLocalJudgeReceiptAuthority::ExactOffline(authority) => {
                authority.with_revalidated_authorities(cancellation, record, use_authorities)
            }
        }
    }

    #[cfg(test)]
    pub(in crate::local_ollama_managed_preflight::generation) fn from_portable_closure_for_test(
        eval: CandidateJudgeRunnerHandoff<'_>,
        response_aggregate: CandidateJudgeResponseAggregateV1,
        observation_batch: CandidateJudgeObservationBatchV1,
        record: ManagedLocalJudgeReceiptRecordV1,
    ) -> ManagedLocalJudgeReceipt<'_, 'static, 'static, 'static> {
        ManagedLocalJudgeReceipt {
            authority: ManagedLocalJudgeReceiptAuthority::ExactOffline(Box::new(
                ExactOfflineReceiptAuthority::new(eval, response_aggregate, observation_batch),
            )),
            record,
        }
    }
}

impl fmt::Debug for ManagedLocalJudgeReceipt<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedLocalJudgeReceipt")
            .field(
                "managed_local_judge_receipt_id",
                self.record.managed_local_judge_receipt_id(),
            )
            .field("attempt_count", &self.record.attempt_count())
            .finish_non_exhaustive()
    }
}

/// Compiler for one nonforgeable managed local-judge receipt capability.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::local_ollama_managed_preflight::generation) struct ManagedLocalJudgeReceiptCompiler;

impl ManagedLocalJudgeReceiptCompiler {
    /// Consumes one cleanup-gated execution and derives every receipt field inside
    /// its mandatory before-and-after retained-authority bracket.
    pub(in crate::local_ollama_managed_preflight::generation) fn compile<
        'store,
        'records,
        'model,
        'runtime,
    >(
        mut execution: VerifiedManagedJudgeScheduleExecution<'store, 'records, 'model, 'runtime>,
        cancellation: &CancellationToken,
    ) -> Result<
        ManagedLocalJudgeReceipt<'store, 'records, 'model, 'runtime>,
        ManagedLocalJudgeReceiptCompilerError,
    > {
        let record = execution
            .with_revalidated_authorities(cancellation, |view| compile_record(&view))
            .map_err(map_authority_error)?;
        Ok(ManagedLocalJudgeReceipt {
            authority: ManagedLocalJudgeReceiptAuthority::Managed(Box::new(execution)),
            record,
        })
    }
}

trait ReleasedReceiptAuthority {
    fn exact_portable_relationship(&self, eval: &CandidateJudgeRunnerHandoff<'_>) -> bool;
    fn judge_system(&self) -> &GenerationSystemRecordV1;
    fn attempt_count(&self) -> u64;
    fn preflight_observer_binding_digest(&self) -> &Digest;
    fn receipt_input(
        &self,
        managed_preflight_digest: Digest,
    ) -> ManagedLocalJudgeReceiptRecordV1Input;
}

impl ReleasedReceiptAuthority for ReleasedManagedJudgeEffectivePackageV2 {
    fn exact_portable_relationship(&self, eval: &CandidateJudgeRunnerHandoff<'_>) -> bool {
        self.judge_plan_id() == eval.judge_plan().candidate_judge_plan_id()
            && self.judge_schedule_id() == eval.judge_schedule().candidate_judge_schedule_id()
            && self.request_aggregate_id() == eval.request_aggregate().request_aggregate_id()
            && self.judge_system() == eval.judge_system()
    }

    fn judge_system(&self) -> &GenerationSystemRecordV1 {
        self.judge_system()
    }

    fn attempt_count(&self) -> u64 {
        self.attempt_count()
    }

    fn preflight_observer_binding_digest(&self) -> &Digest {
        self.preflight_observer_binding_digest()
    }

    fn receipt_input(
        &self,
        managed_preflight_digest: Digest,
    ) -> ManagedLocalJudgeReceiptRecordV1Input {
        ManagedLocalJudgeReceiptRecordV1Input {
            judge_runtime_installation_generation: self.runtime_installation_generation(),
            judge_model_installation_generation: self.model_installation_generation(),
            managed_preflight_digest,
            retained_session_preflight_digest: self.retained_session_preflight_digest().clone(),
            residency_receipt_aggregate_digest: self.residency_receipt_aggregate_digest().clone(),
            process_observation_aggregate_digest: self
                .process_observation_aggregate_digest()
                .clone(),
            native_load_observation_aggregate_digest: self
                .native_load_observation_aggregate_digest()
                .clone(),
            connection_observation_aggregate_digest: self
                .connection_observation_aggregate_digest()
                .clone(),
            effective_runtime_state_observation_aggregate_digest: self
                .effective_runtime_state_observation_aggregate_digest()
                .clone(),
            judge_effective_runtime_state_join_id: self.effective_runtime_state_join_id().clone(),
            first_response_ordinal: self.first_response_ordinal(),
            last_response_ordinal: self.last_response_ordinal(),
        }
    }
}

fn compile_record(
    view: &ManagedJudgeScheduleExecutionView<'_, '_>,
) -> Result<ManagedLocalJudgeReceiptRecordV1, ManagedLocalJudgeReceiptCompilerError> {
    compile_record_from_authority(
        view.eval,
        view.response_aggregate,
        view.observation_batch,
        view.managed_preflight,
        view.released_package,
    )
}

fn compile_record_from_authority<A: ReleasedReceiptAuthority>(
    eval: &CandidateJudgeRunnerHandoff<'_>,
    response_aggregate: &CandidateJudgeResponseAggregateV1,
    observation_batch: &CandidateJudgeObservationBatchV1,
    managed_preflight: &LocalOllamaManagedPreflightOutcome,
    authority: &A,
) -> Result<ManagedLocalJudgeReceiptRecordV1, ManagedLocalJudgeReceiptCompilerError> {
    if !authority.exact_portable_relationship(eval) {
        return Err(relationship(
            ManagedLocalJudgeReceiptRelationship::PortableJudge,
        ));
    }
    let attempt_count = u64::from(eval.judge_schedule().entry_count());
    if authority.attempt_count() != attempt_count
        || u64::from(response_aggregate.entry_count()) != attempt_count
        || u64::from(observation_batch.entry_count()) != attempt_count
    {
        return Err(relationship(
            ManagedLocalJudgeReceiptRelationship::AttemptCount,
        ));
    }
    let report = managed_preflight.report();
    if managed_preflight
        .build_binding()
        .managed_preflight_binding_digest()
        != &report.binding_digest
        || derive_preflight_observer_binding(eval.judge_schedule(), report).as_ref()
            != Some(authority.preflight_observer_binding_digest())
    {
        return Err(relationship(
            ManagedLocalJudgeReceiptRelationship::ManagedPreflight,
        ));
    }
    ManagedLocalJudgeReceiptRecordV1::new(
        ManagedLocalJudgeReceiptRecordV1Relations {
            plan: eval.judge_plan(),
            schedule: eval.judge_schedule(),
            judge_system: authority.judge_system(),
            request_aggregate: eval.request_aggregate(),
            response_aggregate,
            observation_batch,
        },
        authority.receipt_input(report.binding_digest.clone()),
    )
    .map_err(ManagedLocalJudgeReceiptCompilerError::Portable)
}

fn derive_preflight_observer_binding(
    schedule: &rewrite_model::CandidateJudgeScheduleV1,
    report: &LocalOllamaManagedPreflightReport,
) -> Option<Digest> {
    let (initial_connection, responses) = report.connection_observations.split_first()?;
    let response_count = usize::try_from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT).ok()?;
    if responses.len() != response_count || responses.last()? != &report.connection_witness {
        return None;
    }
    let connection = preflight_connection_span_digest(
        schedule.candidate_judge_schedule_id(),
        initial_connection,
        responses,
    );
    let mut material =
        Vec::with_capacity(MANAGED_JUDGE_PREFLIGHT_OBSERVER_BINDING_DOMAIN.len() + 64 * 6);
    material.extend_from_slice(MANAGED_JUDGE_PREFLIGHT_OBSERVER_BINDING_DOMAIN);
    for digest in [
        schedule.candidate_judge_schedule_id().digest(),
        report.initial_process_witness.evidence_digest(),
        report.post_preflight_process_witness.evidence_digest(),
        report.final_process_witness.evidence_digest(),
        report.native_load.native_load_observation_id().digest(),
        &connection,
    ] {
        material.extend_from_slice(digest.as_str().as_bytes());
    }
    Some(Digest::sha256(&material))
}

fn preflight_connection_span_digest(
    schedule_id: &rewrite_model::CandidateJudgeScheduleId,
    initial: &RetainedTcpConnectionEvidence,
    responses: &[RetainedTcpConnectionEvidence],
) -> Digest {
    let mut bytes = Vec::with_capacity(256 + responses.len() * 72);
    push_field(&mut bytes, PREFLIGHT_CONNECTION_SPAN_DOMAIN);
    push_field(&mut bytes, schedule_id.digest().as_str().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&1_u64.to_be_bytes());
    bytes.extend_from_slice(&u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT).to_be_bytes());
    bytes.extend_from_slice(&(responses.len() as u64).to_be_bytes());
    push_field(&mut bytes, initial.evidence_digest().as_str().as_bytes());
    for evidence in responses {
        push_field(&mut bytes, evidence.evidence_digest().as_str().as_bytes());
    }
    Digest::sha256(&bytes)
}

fn push_field(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
    bytes.extend_from_slice(field);
}

fn map_authority_error(
    error: ManagedJudgeScheduleExecutionAuthorityError<ManagedLocalJudgeReceiptCompilerError>,
) -> ManagedLocalJudgeReceiptCompilerError {
    match error {
        ManagedJudgeScheduleExecutionAuthorityError::Initial(failures) => {
            ManagedLocalJudgeReceiptCompilerError::InitialAuthority(failures)
        }
        ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
            initial,
            final_validation,
        } => ManagedLocalJudgeReceiptCompilerError::InitialAndFinalAuthority {
            initial,
            final_validation,
        },
        ManagedJudgeScheduleExecutionAuthorityError::Callback(compilation) => compilation,
        ManagedJudgeScheduleExecutionAuthorityError::Final(failures) => {
            ManagedLocalJudgeReceiptCompilerError::FinalAuthority(failures)
        }
        ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
            callback,
            final_validation,
        } => ManagedLocalJudgeReceiptCompilerError::CompilationAndFinalAuthority {
            compilation: Box::new(callback),
            final_validation,
        },
    }
}

const fn relationship(
    relationship: ManagedLocalJudgeReceiptRelationship,
) -> ManagedLocalJudgeReceiptCompilerError {
    ManagedLocalJudgeReceiptCompilerError::Relationship(relationship)
}

#[cfg(test)]
mod tests;
