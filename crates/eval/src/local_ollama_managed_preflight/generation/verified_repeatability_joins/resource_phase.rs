//! Policy-derived resource phase over one complete Passed repeatability closure.

use std::fmt;

use rewrite_app::{
    GenerationQualificationResourcePolicyLimitsV1, VerifiedGenerationQualificationResourcePolicy,
};
use rewrite_model::{
    GenerationQualificationOperationPolicyV1, GenerationQualificationPhaseEvidenceError,
    GenerationQualificationPhaseScopeV1, GenerationQualificationPhaseStatusV1,
    GenerationResourceAttemptResultRecordV1, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations, GenerationResourceExceededLimitV1,
    MAX_GENERATION_QUALIFICATION_PHASE_ITEMS,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    VerifiedCompletePassedRepeatabilityJoins, VerifiedCompletePassedRepeatabilityJoinsError,
};

#[path = "resource_phase/authority.rs"]
mod authority;
#[path = "resource_phase/evidence.rs"]
mod evidence;

use authority::{ResourceAuthorityBindingView, validate_resource_authority_bindings};
use evidence::FrozenResourcePhaseEvidence;

/// Stateless compiler for one complete resource phase.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationQualificationResourcePhaseCompiler;

impl GenerationQualificationResourcePhaseCompiler {
    /// Consumes complete Passed repeatability and its exact Approved resource policy.
    ///
    /// Policy exceedance is a valid `Failed` phase outcome. Missing observations,
    /// cancellation, authority failure, or an incomplete closure return an error
    /// and never produce a manifest.
    ///
    /// # Errors
    ///
    /// Returns a content-free bracketed error for any initial, derivation, or
    /// mandatory independent final validation failure.
    pub fn compile<'store, 'records, 'model, 'runtime>(
        mut repeatability: VerifiedCompletePassedRepeatabilityJoins<
            'store,
            'records,
            'model,
            'runtime,
        >,
        resource_policy: VerifiedGenerationQualificationResourcePolicy,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedGenerationQualificationResourcePhase<'store, 'records, 'model, 'runtime>,
        GenerationQualificationResourcePhaseCompilationError,
    > {
        let operation_policy = repeatability.operation_policy().clone();
        let derived = with_revalidated_snapshot(
            &mut repeatability,
            &resource_policy,
            &operation_policy,
            cancellation,
            |authority, results| derive_phase(authority, &resource_policy, results, cancellation),
        )?;
        Ok(VerifiedGenerationQualificationResourcePhase {
            repeatability,
            resource_policy,
            evidence: FrozenResourcePhaseEvidence::from_derived(derived),
        })
    }
}

/// Nonforgeable live authority over one complete policy-derived resource phase.
///
/// This value retains the complete repeatability join tree and the exact app-owned
/// Approved policy. It grants no qualification, activation, launch, or traffic.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedGenerationQualificationResourcePhase;
///
/// fn clone_authority(value: VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>) {
///     let _forged = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedGenerationQualificationResourcePhase;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualificationResourcePhase<'store, 'records, 'model, 'runtime> {
    repeatability: VerifiedCompletePassedRepeatabilityJoins<'store, 'records, 'model, 'runtime>,
    resource_policy: VerifiedGenerationQualificationResourcePolicy,
    evidence: FrozenResourcePhaseEvidence,
}

impl VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_> {
    /// Returns every exact resource result in repetition then suite-case order.
    #[must_use]
    pub fn resource_results(&self) -> &[GenerationResourceAttemptResultRecordV1] {
        self.evidence.resource_results()
    }

    /// Returns the exact policy-derived resource manifest.
    #[must_use]
    pub const fn resource_manifest(&self) -> &GenerationResourceEvidenceManifestV1 {
        self.evidence.resource_manifest()
    }

    /// Returns the retained complete Passed repeatability manifest.
    #[must_use]
    pub const fn repeatability_manifest(
        &self,
    ) -> &rewrite_model::GenerationRepeatabilityEvidenceManifestV1 {
        self.repeatability.repeatability_manifest()
    }

    /// Freshly revalidates both live authorities and the complete derived manifest.
    ///
    /// # Errors
    ///
    /// Returns the same bracketed content-free errors as initial compilation.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationResourcePhaseCompilationError> {
        let operation_policy = self.repeatability.operation_policy().clone();
        let resource_policy = &self.resource_policy;
        let expected_evidence = &self.evidence;
        with_revalidated_snapshot(
            &mut self.repeatability,
            resource_policy,
            &operation_policy,
            cancellation,
            |authority, results| {
                let derived = derive_phase(authority, resource_policy, results, cancellation)?;
                if expected_evidence.matches(&derived) {
                    Ok(())
                } else {
                    Err(GenerationQualificationResourcePhaseDerivationError::Relationship)
                }
            },
        )
    }
}

impl fmt::Debug for VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationResourcePhase")
            .field(
                "resource_result_count",
                &self.evidence.resource_results().len(),
            )
            .field("status", &self.evidence.resource_manifest().status())
            .finish_non_exhaustive()
    }
}

/// One initial or final live-authority failure in the resource-phase bracket.
#[derive(Debug, Error)]
pub enum GenerationQualificationResourcePhaseAuthorityError {
    /// Cooperative cancellation was observed.
    #[error("generation qualification resource phase was cancelled")]
    Cancelled,
    /// The exact resource policy was not source-approved.
    #[error("generation qualification resource policy is denied")]
    PolicyDenied,
    /// The app policy did not name the exact operation resource-policy digest.
    #[error("generation qualification resource policy binding does not match")]
    PolicyBinding,
    /// The operation did not name the complete authority's target, plan, or suite.
    #[error("generation qualification resource operation scope does not match")]
    OperationScope,
    /// The complete repeatability or nested resource authority failed.
    #[error("generation qualification resource repeatability authority failed")]
    Repeatability(#[source] VerifiedCompletePassedRepeatabilityJoinsError),
    /// Independent final collection differed from the initial exact snapshot.
    #[error("generation qualification resource snapshot changed")]
    SnapshotChanged,
}

/// Pure resource closure or manifest derivation failure.
#[derive(Debug, Error)]
pub enum GenerationQualificationResourcePhaseDerivationError {
    /// Cooperative cancellation was observed during bounded derivation.
    #[error("generation qualification resource derivation was cancelled")]
    Cancelled,
    /// The complete target resource-result count is invalid.
    #[error("generation qualification resource result count is invalid")]
    InvalidCount,
    /// A target result was missing, extra, reordered, duplicated, or substituted.
    #[error("generation qualification resource result relationship does not match")]
    Relationship,
    /// A result's exceeded-limit list differed from the exact retained policy.
    #[error("generation qualification resource limit classification does not match")]
    LimitClassification,
    /// The model manifest contract rejected the derived exact result roots.
    #[error("generation qualification resource manifest construction failed")]
    Manifest(#[source] GenerationQualificationPhaseEvidenceError),
}

/// Complete before, callback, and mandatory independent after failure matrix.
#[derive(Debug, Error)]
pub enum GenerationQualificationResourcePhaseCompilationError {
    /// Initial live-authority validation failed.
    #[error("generation qualification resource phase initial authority failed")]
    InitialAuthority(#[source] GenerationQualificationResourcePhaseAuthorityError),
    /// Initial and independent final live-authority validation both failed.
    #[error("generation qualification resource phase initial and final authorities failed")]
    InitialAndFinalAuthority {
        /// Initial authority failure.
        initial: GenerationQualificationResourcePhaseAuthorityError,
        /// Independent final authority failure.
        final_validation: GenerationQualificationResourcePhaseAuthorityError,
    },
    /// Pure complete resource derivation failed.
    #[error("generation qualification resource phase derivation failed")]
    Compilation(#[source] GenerationQualificationResourcePhaseDerivationError),
    /// Mandatory independent final live-authority validation failed.
    #[error("generation qualification resource phase final authority failed")]
    FinalAuthority(#[source] GenerationQualificationResourcePhaseAuthorityError),
    /// Derivation and independent final authority validation both failed.
    #[error("generation qualification resource phase derivation and final authority failed")]
    CompilationAndFinalAuthority {
        /// Primary pure derivation failure.
        compilation: GenerationQualificationResourcePhaseDerivationError,
        /// Independent final authority failure.
        final_validation: GenerationQualificationResourcePhaseAuthorityError,
    },
}

struct DerivedResourcePhase {
    results: Vec<GenerationResourceAttemptResultRecordV1>,
    manifest: GenerationResourceEvidenceManifestV1,
}

#[derive(Clone)]
struct ResourceEvidenceView {
    relationships_match: bool,
    facts: ResourceFacts,
    observed_exceeded: Vec<GenerationResourceExceededLimitV1>,
    digest: rewrite_types::Digest,
}

#[derive(Clone, Copy)]
struct ResourceManifestDerivation<'a> {
    scope: GenerationQualificationPhaseScopeV1<'a>,
    policy_digest: &'a rewrite_types::Digest,
    limits: GenerationQualificationResourcePolicyLimitsV1,
    expected_count: usize,
    evidence: &'a [ResourceEvidenceView],
}

fn with_revalidated_snapshot<T>(
    repeatability: &mut VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    operation_policy: &GenerationQualificationOperationPolicyV1,
    cancellation: &CancellationToken,
    use_snapshot: impl FnOnce(
        &VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>,
        &[GenerationResourceAttemptResultRecordV1],
    ) -> Result<T, GenerationQualificationResourcePhaseDerivationError>,
) -> Result<T, GenerationQualificationResourcePhaseCompilationError> {
    with_snapshot_bracket(
        repeatability,
        |authority| {
            validate_and_collect(authority, resource_policy, operation_policy, cancellation)
        },
        |authority, results: &Vec<GenerationResourceAttemptResultRecordV1>| {
            use_snapshot(authority, results)
        },
    )
}

fn with_snapshot_bracket<C, I: PartialEq, T>(
    context: &mut C,
    mut validate: impl FnMut(&mut C) -> Result<I, GenerationQualificationResourcePhaseAuthorityError>,
    use_snapshot: impl FnOnce(&C, &I) -> Result<T, GenerationQualificationResourcePhaseDerivationError>,
) -> Result<T, GenerationQualificationResourcePhaseCompilationError> {
    let initial = validate(context);
    let callback = initial
        .as_ref()
        .ok()
        .map(|snapshot| use_snapshot(context, snapshot));
    let mut final_validation = validate(context);
    if initial
        .as_ref()
        .ok()
        .zip(final_validation.as_ref().ok())
        .is_some_and(|(before, after)| before != after)
    {
        final_validation = Err(GenerationQualificationResourcePhaseAuthorityError::SnapshotChanged);
    }
    finish_bracket(initial, callback, final_validation)
}

fn finish_bracket<T, I: PartialEq>(
    initial: Result<I, GenerationQualificationResourcePhaseAuthorityError>,
    callback: Option<Result<T, GenerationQualificationResourcePhaseDerivationError>>,
    final_validation: Result<I, GenerationQualificationResourcePhaseAuthorityError>,
) -> Result<T, GenerationQualificationResourcePhaseCompilationError> {
    match (initial, callback, final_validation) {
        (Err(initial), None, Err(final_validation)) => Err(
            GenerationQualificationResourcePhaseCompilationError::InitialAndFinalAuthority {
                initial,
                final_validation,
            },
        ),
        (Err(initial), None, Ok(_)) => {
            Err(GenerationQualificationResourcePhaseCompilationError::InitialAuthority(initial))
        }
        (Ok(_), Some(Err(compilation)), Err(final_validation)) => Err(
            GenerationQualificationResourcePhaseCompilationError::CompilationAndFinalAuthority {
                compilation,
                final_validation,
            },
        ),
        (Ok(_), Some(Err(compilation)), Ok(_)) => {
            Err(GenerationQualificationResourcePhaseCompilationError::Compilation(compilation))
        }
        (Ok(_), Some(Ok(value)), Err(final_validation)) => {
            drop(value);
            Err(
                GenerationQualificationResourcePhaseCompilationError::FinalAuthority(
                    final_validation,
                ),
            )
        }
        (Ok(_), Some(Ok(value)), Ok(_)) => Ok(value),
        (Err(_), Some(_), _) | (Ok(_), None, _) => {
            unreachable!("callback presence is determined by initial validation")
        }
    }
}

fn validate_and_collect(
    repeatability: &mut VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    operation_policy: &GenerationQualificationOperationPolicyV1,
    cancellation: &CancellationToken,
) -> Result<
    Vec<GenerationResourceAttemptResultRecordV1>,
    GenerationQualificationResourcePhaseAuthorityError,
> {
    check_active(cancellation)?;
    validate_resource_authority_bindings(ResourceAuthorityBindingView {
        source_disposition: resource_policy.source_disposition(),
        policy_binding_matches: resource_policy
            .revalidate_operation_policy(operation_policy)
            .is_ok(),
        operation_scope_matches: operation_scope_matches(repeatability, operation_policy),
    })?;
    let results = repeatability
        .collect_resource_results(
            operation_policy.baseline_generation_system_id(),
            cancellation,
        )
        .map_err(GenerationQualificationResourcePhaseAuthorityError::Repeatability)?;
    check_active(cancellation)?;
    Ok(results)
}

fn operation_scope_matches(
    repeatability: &VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>,
    operation_policy: &GenerationQualificationOperationPolicyV1,
) -> bool {
    operation_policy.target_generation_system_id()
        == repeatability
            .target_generation_system()
            .generation_system_id()
        && operation_policy.baseline_generation_system_id()
            != operation_policy.target_generation_system_id()
        && operation_policy.generation_qualification_plan_id()
            == repeatability.qualification_plan().qualification_plan_id()
        && operation_policy.suite_manifest_id() == repeatability.suite().suite_manifest_id()
        && repeatability.repeatability_manifest().phase_policy_digest()
            == operation_policy.repeatability_policy_digest()
}

fn derive_phase(
    authority: &VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    results: &[GenerationResourceAttemptResultRecordV1],
    cancellation: &CancellationToken,
) -> Result<DerivedResourcePhase, GenerationQualificationResourcePhaseDerivationError> {
    check_derivation_active(cancellation)?;
    let expected_count = authority
        .preregistered_repetitions()
        .len()
        .checked_mul(authority.suite().case_ids().len())
        .filter(|count| *count > 0 && *count <= MAX_GENERATION_QUALIFICATION_PHASE_ITEMS)
        .ok_or(GenerationQualificationResourcePhaseDerivationError::InvalidCount)?;
    if results.len() != expected_count {
        return Err(GenerationQualificationResourcePhaseDerivationError::InvalidCount);
    }

    let target_id = authority.target_generation_system().generation_system_id();
    let policy_digest = resource_policy.policy_digest();
    let limits = resource_policy.limits();
    let mut cursor = 0;
    let mut evidence = Vec::with_capacity(results.len());
    for repetition in authority.preregistered_repetitions() {
        for case_id in authority.suite().case_ids() {
            check_derivation_active(cancellation)?;
            let matching = authority
                .planned_attempts()
                .iter()
                .filter(|attempt| {
                    attempt.generation_system_id() == target_id
                        && attempt.repetition_id() == repetition.repetition_id()
                        && attempt.case_id() == case_id
                })
                .collect::<Vec<_>>();
            let [planned_attempt] = matching.as_slice() else {
                return Err(GenerationQualificationResourcePhaseDerivationError::Relationship);
            };
            let result = results
                .get(cursor)
                .ok_or(GenerationQualificationResourcePhaseDerivationError::InvalidCount)?;
            let relationships_match = result.generation_system_id() == target_id
                && result.generation_qualification_plan_id()
                    == authority.qualification_plan().qualification_plan_id()
                && result.suite_manifest_id() == authority.suite().suite_manifest_id()
                && result.repetition_id() == repetition.repetition_id()
                && result.case_id() == case_id
                && result.planned_attempt_id() == planned_attempt.planned_attempt_id()
                && result.resource_policy_digest() == policy_digest;
            evidence.push(ResourceEvidenceView {
                relationships_match,
                facts: resource_facts(result),
                observed_exceeded: result.exceeded_limits().to_vec(),
                digest: result.resource_attempt_result_id().digest().clone(),
            });
            cursor += 1;
        }
    }
    if cursor != results.len() {
        return Err(GenerationQualificationResourcePhaseDerivationError::InvalidCount);
    }
    let manifest = derive_resource_manifest(
        ResourceManifestDerivation {
            scope: GenerationQualificationPhaseScopeV1 {
                generation_system: authority.target_generation_system(),
                qualification_plan: authority.qualification_plan(),
                suite: authority.suite(),
            },
            policy_digest,
            limits,
            expected_count,
            evidence: &evidence,
        },
        cancellation,
    )?;
    Ok(DerivedResourcePhase {
        results: results.to_vec(),
        manifest,
    })
}

fn derive_resource_manifest(
    derivation: ResourceManifestDerivation<'_>,
    cancellation: &CancellationToken,
) -> Result<GenerationResourceEvidenceManifestV1, GenerationQualificationResourcePhaseDerivationError>
{
    check_derivation_active(cancellation)?;
    if derivation.expected_count == 0
        || derivation.expected_count > MAX_GENERATION_QUALIFICATION_PHASE_ITEMS
        || derivation.evidence.len() != derivation.expected_count
    {
        return Err(GenerationQualificationResourcePhaseDerivationError::InvalidCount);
    }
    let mut any_exceeded = false;
    for evidence in derivation.evidence {
        check_derivation_active(cancellation)?;
        if !evidence.relationships_match {
            return Err(GenerationQualificationResourcePhaseDerivationError::Relationship);
        }
        let expected_exceeded = exceeded_limits(evidence.facts, derivation.limits);
        if evidence.observed_exceeded != expected_exceeded {
            return Err(GenerationQualificationResourcePhaseDerivationError::LimitClassification);
        }
        any_exceeded |= !expected_exceeded.is_empty();
    }
    check_derivation_active(cancellation)?;
    let digests = derivation
        .evidence
        .iter()
        .map(|evidence| evidence.digest.clone())
        .collect::<Vec<_>>();
    GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
        scope: derivation.scope,
        phase_policy_digest: derivation.policy_digest,
        evidence_record_digests: &digests,
        status: phase_status(any_exceeded),
    })
    .map_err(GenerationQualificationResourcePhaseDerivationError::Manifest)
}

#[derive(Clone, Copy)]
struct ResourceFacts {
    attempt_elapsed_nanoseconds: u64,
    first_response_elapsed_nanoseconds: u64,
    cleanup_elapsed_nanoseconds: u64,
    worker_high_water_resident_bytes: u64,
    installed_footprint_bytes: u64,
}

fn resource_facts(result: &GenerationResourceAttemptResultRecordV1) -> ResourceFacts {
    ResourceFacts {
        attempt_elapsed_nanoseconds: result.attempt_elapsed_nanoseconds(),
        first_response_elapsed_nanoseconds: result.first_response_elapsed_nanoseconds(),
        cleanup_elapsed_nanoseconds: result.cleanup_elapsed_nanoseconds(),
        worker_high_water_resident_bytes: result.worker_high_water_resident_bytes(),
        installed_footprint_bytes: result.installed_footprint_bytes(),
    }
}

fn exceeded_limits(
    facts: ResourceFacts,
    limits: GenerationQualificationResourcePolicyLimitsV1,
) -> Vec<GenerationResourceExceededLimitV1> {
    [
        (
            facts.attempt_elapsed_nanoseconds > limits.maximum_attempt_elapsed_nanoseconds,
            GenerationResourceExceededLimitV1::AttemptElapsed,
        ),
        (
            facts.first_response_elapsed_nanoseconds > limits.maximum_first_response_nanoseconds,
            GenerationResourceExceededLimitV1::FirstResponse,
        ),
        (
            facts.cleanup_elapsed_nanoseconds > limits.maximum_cleanup_nanoseconds,
            GenerationResourceExceededLimitV1::Cleanup,
        ),
        (
            facts.worker_high_water_resident_bytes
                > limits.maximum_worker_high_water_resident_bytes,
            GenerationResourceExceededLimitV1::WorkerHighWaterResident,
        ),
        (
            facts.installed_footprint_bytes > limits.maximum_installed_footprint_bytes,
            GenerationResourceExceededLimitV1::InstalledFootprint,
        ),
    ]
    .into_iter()
    .filter_map(|(exceeded, limit)| exceeded.then_some(limit))
    .collect()
}

const fn phase_status(any_exceeded: bool) -> GenerationQualificationPhaseStatusV1 {
    if any_exceeded {
        GenerationQualificationPhaseStatusV1::Failed
    } else {
        GenerationQualificationPhaseStatusV1::Passed
    }
}

fn check_active(
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationResourcePhaseAuthorityError> {
    if cancellation.is_cancelled() {
        Err(GenerationQualificationResourcePhaseAuthorityError::Cancelled)
    } else {
        Ok(())
    }
}

fn check_derivation_active(
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationResourcePhaseDerivationError> {
    if cancellation.is_cancelled() {
        Err(GenerationQualificationResourcePhaseDerivationError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "resource_phase/tests.rs"]
mod tests;
