use std::fmt;

use rewrite_app::{
    CandidateGenerationReceiptCompilation, CandidateGenerationReceiptCompilationError,
    VerifiedGenerationQualificationResourcePolicy,
};
use rewrite_inference::GenerationCandidate;
use rewrite_model::{
    CandidateGenerationAttemptRecordV1, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleReadbackV1, CandidateGenerationReceiptV1,
    GenerationCaseManifestV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationRepetitionRecordV1, GenerationResourceAttemptResultRecordV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::VerifiedCompletedManagedCandidateAttempt;
use crate::active_generation_qualification_subject::{
    ActiveGenerationQualificationBinding, ActiveGenerationQualificationSubject,
};

mod resource;
mod validation;

/// Exact scope and policy relationships for compiling one resource-observed batch.
#[derive(Clone, Copy)]
pub struct VerifiedCandidateBatchResourceInput<'a> {
    /// Exact target system, frozen plan, and suite.
    pub scope: GenerationQualificationPhaseScopeV1<'a>,
    /// Exact operation policy designating the target.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Exact source-approved resource policy used before traffic.
    pub resource_policy: &'a VerifiedGenerationQualificationResourcePolicy,
    /// Exact case selected by the planned attempt.
    pub case: &'a GenerationCaseManifestV1,
    /// Exact repetition selected by the planned attempt.
    pub repetition: &'a GenerationRepetitionRecordV1,
}

/// Opaque authority for one managed, cleanup-complete, and readback-verified candidate batch.
///
/// The batch owns both the live completed attempt and the inert receipt compilation. Portable
/// records, copied candidates, and copied digests remain inert without this capability. Candidate
/// text remains untrusted input to later deterministic evaluation.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCandidateBatch;
///
/// fn clone_capability(value: &VerifiedCandidateBatch) {
///     let _forged: VerifiedCandidateBatch = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCandidateBatch;
///
/// fn serialize_capability(value: &VerifiedCandidateBatch) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct VerifiedCandidateBatch {
    completed_attempt: VerifiedCompletedManagedCandidateAttempt,
    receipt_compilation: CandidateGenerationReceiptCompilation,
    resource_result: Option<GenerationResourceAttemptResultRecordV1>,
}

impl VerifiedCandidateBatch {
    pub(crate) fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding> {
        self.completed_attempt.active_binding()
    }

    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        self.completed_attempt.matches_active_subject(subject)
    }

    /// Joins one live completed attempt to one independently reloaded receipt compilation.
    ///
    /// The held evidence-tree lease is revalidated before any relationship comparison and again
    /// after every candidate has been read and compared. Both input capabilities are consumed.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchError`] for cancellation, evidence-tree drift, or any
    /// substituted planned attempt, system, request, precursor, managed evidence, cleanup,
    /// effective package, response, usage observation, attempt record, or candidate.
    pub fn verify(
        completed_attempt: VerifiedCompletedManagedCandidateAttempt,
        receipt_compilation: CandidateGenerationReceiptCompilation,
        cancellation: &CancellationToken,
    ) -> Result<Self, VerifiedCandidateBatchError> {
        receipt_compilation.revalidate(cancellation)?;
        resource::validate_resource_mode(completed_attempt.resource_closure().is_some(), false)?;
        validation::validate_fixed_relationships(&completed_attempt, &receipt_compilation)?;
        validation::validate_candidates(&completed_attempt, &receipt_compilation, cancellation)?;
        receipt_compilation.revalidate(cancellation)?;
        Ok(Self {
            completed_attempt,
            receipt_compilation,
            resource_result: None,
        })
    }

    /// Joins and compiles one complete resource-observed managed attempt.
    ///
    /// The app observation and retained receipt tree are revalidated before and
    /// after portable record construction. Both input authorities are consumed.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchError`] for any cancellation, drift,
    /// omitted resource closure, source denial, or substituted relationship.
    pub fn verify_resource_observed(
        completed_attempt: VerifiedCompletedManagedCandidateAttempt,
        receipt_compilation: CandidateGenerationReceiptCompilation,
        input: VerifiedCandidateBatchResourceInput<'_>,
        cancellation: &CancellationToken,
    ) -> Result<Self, VerifiedCandidateBatchError> {
        receipt_compilation.revalidate(cancellation)?;
        validation::validate_fixed_relationships(&completed_attempt, &receipt_compilation)?;
        validation::validate_candidates(&completed_attempt, &receipt_compilation, cancellation)?;
        resource::validate_resource_mode(completed_attempt.resource_closure().is_some(), true)?;
        let closure = completed_attempt.resource_closure().ok_or(
            VerifiedCandidateBatchError::Relationship(
                VerifiedCandidateBatchRelationship::MissingResourceObservation,
            ),
        )?;
        let resource_result = resource::compile_resource_result(
            closure,
            &completed_attempt,
            &receipt_compilation,
            &input,
        )?;
        receipt_compilation.revalidate(cancellation)?;
        resource::validate_retained_resource(
            closure,
            &resource_result,
            &completed_attempt,
            receipt_compilation.receipt(),
        )?;
        Ok(Self {
            completed_attempt,
            receipt_compilation,
            resource_result: Some(resource_result),
        })
    }

    /// Returns the inert durable receipt retained by this authority.
    #[must_use]
    pub const fn receipt(&self) -> &CandidateGenerationReceiptV1 {
        self.receipt_compilation.receipt()
    }

    /// Returns the inert completed attempt record retained by this authority.
    #[must_use]
    pub const fn attempt_record(&self) -> &CandidateGenerationAttemptRecordV1 {
        self.receipt_compilation.attempt_record()
    }

    pub(crate) const fn evidence_bundle_manifest(
        &self,
    ) -> &CandidateGenerationEvidenceBundleManifestV1 {
        self.receipt_compilation.lease().manifest()
    }

    pub(crate) const fn evidence_bundle_readback(
        &self,
    ) -> &CandidateGenerationEvidenceBundleReadbackV1 {
        self.receipt_compilation.lease().readback()
    }

    /// Returns the compiled portable resource result when this is a strict batch.
    #[must_use]
    pub const fn resource_result(&self) -> Option<&GenerationResourceAttemptResultRecordV1> {
        self.resource_result.as_ref()
    }

    /// Returns the number of verified candidates.
    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.completed_attempt.candidates().len()
    }

    /// Revalidates the evidence tree, then returns the retained live completed attempt.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchError`] for cancellation or any evidence-tree drift.
    pub fn completed_attempt(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<&VerifiedCompletedManagedCandidateAttempt, VerifiedCandidateBatchError> {
        self.revalidate(cancellation)?;
        Ok(&self.completed_attempt)
    }

    /// Revalidates the evidence tree, then returns all ordered candidates.
    ///
    /// The returned content is exact but remains untrusted input. Copying it does not copy the
    /// authority of this batch.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchError`] for cancellation or any evidence-tree drift.
    pub fn candidates(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<&[GenerationCandidate], VerifiedCandidateBatchError> {
        self.revalidate(cancellation)?;
        Ok(self.completed_attempt.candidates())
    }

    /// Revalidates the evidence tree, then returns one candidate by its canonical ordinal.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchError`] for cancellation or any evidence-tree drift.
    pub fn candidate(
        &self,
        ordinal: u8,
        cancellation: &CancellationToken,
    ) -> Result<Option<&GenerationCandidate>, VerifiedCandidateBatchError> {
        self.revalidate(cancellation)?;
        Ok(self
            .completed_attempt
            .candidates()
            .get(usize::from(ordinal))
            .filter(|candidate| candidate.ordinal == ordinal))
    }

    /// Revalidates the complete held and named evidence tree.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchError`] for cancellation or any root, manifest, member,
    /// metadata, size, or digest drift.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCandidateBatchError> {
        self.receipt_compilation.revalidate(cancellation)?;
        match (
            self.completed_attempt.resource_closure(),
            self.resource_result.as_ref(),
        ) {
            (None, None) => Ok(()),
            (Some(closure), Some(result)) => resource::validate_retained_resource(
                closure,
                result,
                &self.completed_attempt,
                self.receipt_compilation.receipt(),
            ),
            (None, Some(_)) | (Some(_), None) => Err(VerifiedCandidateBatchError::Relationship(
                VerifiedCandidateBatchRelationship::ResourceResult,
            )),
        }
    }
}

impl fmt::Debug for VerifiedCandidateBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCandidateBatch")
            .field("candidate_count", &self.candidate_count())
            .finish_non_exhaustive()
    }
}

/// Exact relationship rejected while joining a live attempt and inert receipt compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifiedCandidateBatchRelationship {
    /// The receipt does not name the exact live planned attempt.
    PlannedAttempt,
    /// The receipt does not name the exact live generation system.
    GenerationSystem,
    /// The provider-neutral request does not match the planned request relationship.
    GenerationRequest,
    /// The structured request does not match the precursor and receipt relationship.
    StructuredRequest,
    /// The receipt does not name the exact live precursor and its carried relationships.
    Precursor,
    /// The receipt does not name the exact live managed evidence and its carried relationships.
    ManagedEvidence,
    /// The receipt does not name the exact live successful cleanup record.
    Cleanup,
    /// The receipt does not name the exact live cleanup-gated effective package.
    EffectivePackage,
    /// The live structured response does not match the receipt and retained execution receipt.
    Response,
    /// The live response usage does not match the receipt usage observation.
    ResponseUsage,
    /// The compiled attempt record does not close over the live attempt and receipt.
    AttemptRecord,
    /// The receipt does not close over the compilation's retained lease and readback.
    ReceiptCompilation,
    /// The live and inert candidate counts differ.
    CandidateCount,
    /// A candidate is not in its exact canonical ordinal position.
    CandidateOrdinal,
    /// A live candidate differs from the exact retained candidate bytes.
    CandidateBytes,
    /// A candidate's inert artifact relationship differs from its retained bytes.
    CandidateArtifact,
    /// A resource-observed closure was passed to ordinary verification.
    UncompiledResourceObservation,
    /// Strict verification did not receive a resource-observed closure.
    MissingResourceObservation,
    /// App-owned resource observations or their exact policy bindings differed.
    ResourceObservation,
    /// The retained portable resource result no longer matches its authority.
    ResourceResult,
}

/// Failure to create or revalidate a verified candidate batch.
#[derive(Debug, Error)]
pub enum VerifiedCandidateBatchError {
    /// The retained receipt compilation could not be read or revalidated exactly.
    #[error("verified candidate batch receipt compilation failed")]
    ReceiptCompilation(#[from] CandidateGenerationReceiptCompilationError),
    /// Portable resource-result construction rejected an exact relationship.
    #[error("verified candidate batch resource result construction failed")]
    ResourceResult(#[source] GenerationQualificationPhaseEvidenceError),
    /// One live and inert relationship differed.
    #[error("verified candidate batch relationship does not match: {0:?}")]
    Relationship(VerifiedCandidateBatchRelationship),
}
