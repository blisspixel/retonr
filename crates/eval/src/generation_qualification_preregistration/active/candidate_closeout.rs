//! Active-owned publication, receipt compilation, and candidate settlement.

mod error;
mod failure;
mod run;

use rewrite_app::{
    CandidateGenerationEvidenceBundleAuxiliaryArtifact, CandidateGenerationEvidenceBundleLimits,
    CandidateGenerationEvidenceRepository, VerifiedGenerationQualificationResourcePolicy,
};
use rewrite_model::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureV1Input,
    CandidateGenerationAttemptRecordV1, CandidateGenerationEvidenceBundleManifestV1,
    GenerationCaseManifestV1, GenerationClusterRecordV1, GenerationRepetitionRecordV1,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
};
use rewrite_model_store::{
    CandidateGenerationExecutionV1Input, StoredCandidateGenerationExecutionV1,
};
use rewrite_types::CancellationToken;

pub use error::{
    ActiveGenerationQualificationCandidateCloseoutError,
    ActiveGenerationQualificationCandidateCloseoutErrorKind,
};

use self::{
    failure::{
        CloseoutPrimaryFailure, apply_terminal_precedence, map_prepared_closeout_error,
        settlement_failure, settlement_finalization_failed,
    },
    run::{CandidateCloseoutProduct, run_closeout},
};
use super::interruption::CandidateCloseoutInterruptionDisposition;
use super::{ActiveGenerationQualificationOperation, ensure_traffic_eligible};
use crate::{
    GenerationQualificationPreparationError, GenerationQualificationPreregistrationRepository,
    VerifiedCandidateBatch,
};

/// Exact caller-selected storage layout and portable relationships for closeout.
///
/// Active owns every state transition and derives all execution and failure facts.
/// The caller selects only the durable destination, bounded bundle layout, and the
/// exact records that the app and model relationship validators independently reload.
pub struct ActiveGenerationQualificationCandidateCloseoutInput<'input> {
    /// Exact case named by the pending planned attempt.
    pub case: &'input GenerationCaseManifestV1,
    /// Exact cluster containing the case.
    pub cluster: &'input GenerationClusterRecordV1,
    /// Exact preregistered repetition named by the pending attempt.
    pub repetition: &'input GenerationRepetitionRecordV1,
    /// Ordered exact auxiliary bundle members.
    pub auxiliary_artifacts: &'input [CandidateGenerationEvidenceBundleAuxiliaryArtifact<'input>],
    /// Exact model-owned evidence-bundle manifest.
    pub manifest: &'input CandidateGenerationEvidenceBundleManifestV1,
    /// Hard publication and readback ceilings.
    pub limits: CandidateGenerationEvidenceBundleLimits,
    /// Application-owned root for canonical no-replace publication and reacquisition.
    pub evidence_repository: &'input CandidateGenerationEvidenceRepository,
    /// Required for target closeout and forbidden for baseline closeout.
    pub resource_policy: Option<&'input VerifiedGenerationQualificationResourcePolicy>,
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Publishes and settles the exact pending completed candidate in one owner.
    ///
    /// Active compiles the retained response artifact, validates the exact bundle
    /// plan, publishes without replacement, consumes fresh readback into the receipt,
    /// verifies the target or baseline batch, and settles the durable ordinal. Any
    /// failure consumes the pending ordinal without retry, terminalizes the owner,
    /// and derives the exact failed-attempt record with deadline precedence.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for missing state, cross-Active replay, exact-scope
    /// mismatch, publication or readback failure, receipt or batch failure, deadline,
    /// cancellation, authority drift, or mandatory finalization failure.
    pub fn close_pending_candidate(
        &mut self,
        state_repository: &mut GenerationQualificationPreregistrationRepository,
        input: &ActiveGenerationQualificationCandidateCloseoutInput<'_>,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedCandidateBatch, ActiveGenerationQualificationCandidateCloseoutError> {
        if self.terminal {
            return Err(ActiveGenerationQualificationCandidateCloseoutError::simple(
                ActiveGenerationQualificationCandidateCloseoutErrorKind::OperationTerminated,
            ));
        }
        let Some(pending) = self.pending_completed_candidate.as_ref() else {
            return Err(ActiveGenerationQualificationCandidateCloseoutError::simple(
                ActiveGenerationQualificationCandidateCloseoutErrorKind::NoPendingCandidate,
            ));
        };
        let Some(completed_attempt) = pending.completed_attempt.as_ref() else {
            self.terminal = true;
            return Err(ActiveGenerationQualificationCandidateCloseoutError::simple(
                ActiveGenerationQualificationCandidateCloseoutErrorKind::NoPendingCandidate,
            ));
        };
        let deadline = self.prepared.operation_deadline();
        if !completed_attempt.matches_active_subject(&self.subject)
            || completed_attempt.planned_attempt() != &pending.scope.planned_attempt
            || completed_attempt.precursor() != &pending.scope.precursor
        {
            let mandatory_failed = self
                .prepared
                .revalidate_for_mandatory_finalization()
                .is_err();
            return Err(self.fail_pending_closeout(
                state_repository,
                apply_terminal_precedence(
                    CloseoutPrimaryFailure::operation_scope(),
                    deadline,
                    cancellation,
                ),
                mandatory_failed,
            ));
        }
        let pending_scope = pending.scope.clone();
        let completed_attempt = self
            .pending_completed_candidate
            .as_mut()
            .and_then(|pending| pending.completed_attempt.take())
            .ok_or_else(|| {
                ActiveGenerationQualificationCandidateCloseoutError::simple(
                    ActiveGenerationQualificationCandidateCloseoutErrorKind::NoPendingCandidate,
                )
            })?;

        let primary = self
            .prepared
            .with_validated_view(cancellation, |view| {
                ensure_traffic_eligible(view.disposition)
                    .map_err(|error| CloseoutPrimaryFailure::active_authority(error, false))?;
                run_closeout(
                    input,
                    completed_attempt,
                    &pending_scope,
                    &view,
                    cancellation,
                )
            })
            .map_err(map_prepared_closeout_error);

        self.finish_candidate_closeout(
            state_repository,
            input.evidence_repository,
            primary,
            deadline,
            cancellation,
        )
    }

    fn finish_candidate_closeout(
        &mut self,
        state_repository: &mut GenerationQualificationPreregistrationRepository,
        evidence_repository: &CandidateGenerationEvidenceRepository,
        primary: Result<CandidateCloseoutProduct, CloseoutPrimaryFailure>,
        deadline: std::time::Instant,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedCandidateBatch, ActiveGenerationQualificationCandidateCloseoutError> {
        match primary {
            Ok(product) => match self
                .prepare_completed_candidate_settlement(&product.batch, cancellation)
            {
                Ok(record) => {
                    if let Err(error) = self.persist_completed_closeout(
                        state_repository,
                        evidence_repository,
                        &product,
                        &record,
                    ) {
                        self.terminal = true;
                        return Err(
                            ActiveGenerationQualificationCandidateCloseoutError::durability(error),
                        );
                    }
                    let receipt = product.batch.receipt().clone();
                    self.commit_completed_candidate_settlement(receipt, record);
                    Ok(product.batch)
                }
                Err(error) => {
                    let mandatory_failed = settlement_finalization_failed(&error);
                    Err(self.fail_pending_closeout(
                        state_repository,
                        apply_terminal_precedence(
                            settlement_failure(error),
                            deadline,
                            cancellation,
                        ),
                        mandatory_failed,
                    ))
                }
            },
            Err(failure) => {
                let mandatory_failed = self
                    .prepared
                    .revalidate_for_mandatory_finalization()
                    .is_err();
                Err(self.fail_pending_closeout(
                    state_repository,
                    apply_terminal_precedence(failure, deadline, cancellation),
                    mandatory_failed,
                ))
            }
        }
    }

    fn fail_pending_closeout(
        &mut self,
        state_repository: &mut GenerationQualificationPreregistrationRepository,
        failure: CloseoutPrimaryFailure,
        mandatory_finalization_failed: bool,
    ) -> ActiveGenerationQualificationCandidateCloseoutError {
        let Some(scope) = self
            .pending_completed_candidate
            .as_ref()
            .map(|pending| pending.scope.clone())
        else {
            self.terminal = true;
            return ActiveGenerationQualificationCandidateCloseoutError::simple(
                ActiveGenerationQualificationCandidateCloseoutErrorKind::NoPendingCandidate,
            );
        };
        let record = match CandidateGenerationAttemptRecordV1::failed(
            &scope.planned_attempt,
            Some(&scope.precursor),
            CandidateGenerationAttemptFailureV1Input {
                failure_phase: failure.phase,
                failure_category: failure.category,
                traffic_observed: true,
                output_observed: true,
                cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
            },
        ) {
            Ok(record) => record,
            Err(error) => {
                self.terminal = true;
                return ActiveGenerationQualificationCandidateCloseoutError::record_failure(
                    error,
                    mandatory_finalization_failed,
                );
            }
        };
        if let Err(error) = state_repository.persist_candidate_execution(
            &CandidateGenerationExecutionV1Input::Failed {
                preregistration: self.prepared.preregistration_read_input(),
                precursor: Some(&scope.precursor),
                attempt: &record,
            },
            |_| Ok(()),
        ) {
            self.terminal = true;
            return ActiveGenerationQualificationCandidateCloseoutError::durability(error);
        }
        let Some(pending) = self.pending_completed_candidate.take() else {
            unreachable!("failed closeout was persisted from pending Active state")
        };
        self.next_candidate_attempt += 1;
        self.terminal = true;
        if pending.scope.target {
            self.target_attempt_records.push(record.clone());
        }
        let disposition = if mandatory_finalization_failed {
            CandidateCloseoutInterruptionDisposition::MandatoryFinalizationFailed
        } else {
            self.compile_candidate_closeout_interruption(
                failure
                    .interruption_facts(pending.scope.planned_attempt.planned_attempt_id().clone()),
            )
        };
        ActiveGenerationQualificationCandidateCloseoutError::closed(failure, record, disposition)
    }

    fn persist_completed_closeout(
        &mut self,
        state_repository: &mut GenerationQualificationPreregistrationRepository,
        evidence_repository: &CandidateGenerationEvidenceRepository,
        product: &CandidateCloseoutProduct,
        record: &CandidateGenerationAttemptRecordV1,
    ) -> Result<(), GenerationQualificationPreparationError> {
        let fresh = CancellationToken::new();
        let completed = product
            .batch
            .completed_attempt(&fresh)
            .map_err(|_| GenerationQualificationPreparationError::ReadbackMismatch)?;
        let managed = completed.managed_evidence();
        let managed_input = ManagedOllamaCandidateGenerationEvidenceV2Input {
            bracket_observation_v1_id: managed.bracket_observation_v1_id().clone(),
            effective_runtime_state_join_id: managed.effective_runtime_state_join_id().clone(),
            response_id: managed.response_id().clone(),
        };
        state_repository.persist_candidate_execution(
            &CandidateGenerationExecutionV1Input::Completed {
                preregistration: self.prepared.preregistration_read_input(),
                precursor: completed.precursor(),
                managed_evidence: managed,
                managed_evidence_input: &managed_input,
                cleanup: completed.cleanup(),
                bundle: product.batch.evidence_bundle_manifest(),
                storage: &product.storage,
                readback: product.batch.evidence_bundle_readback(),
                receipt: product.batch.receipt(),
                attempt: record,
            },
            |readback| {
                let StoredCandidateGenerationExecutionV1::Completed {
                    bundle,
                    storage,
                    readback: stored_readback,
                    receipt,
                    attempt,
                    ..
                } = readback.execution()
                else {
                    return Err(GenerationQualificationPreparationError::ReadbackMismatch);
                };
                if bundle != product.batch.evidence_bundle_manifest()
                    || storage != &product.storage
                    || stored_readback != product.batch.evidence_bundle_readback()
                    || receipt != product.batch.receipt()
                    || attempt != record
                {
                    return Err(GenerationQualificationPreparationError::ReadbackMismatch);
                }
                let reacquired = evidence_repository
                    .reacquire_bundle(storage.clone(), bundle, stored_readback, &fresh)
                    .map_err(|_| GenerationQualificationPreparationError::ReadbackMismatch)?;
                if reacquired.storage() != storage
                    || reacquired.manifest() != bundle
                    || reacquired.readback() != stored_readback
                {
                    return Err(GenerationQualificationPreparationError::ReadbackMismatch);
                }
                product
                    .batch
                    .revalidate(&fresh)
                    .map_err(|_| GenerationQualificationPreparationError::ReadbackMismatch)
            },
        )?;
        Ok(())
    }
}
