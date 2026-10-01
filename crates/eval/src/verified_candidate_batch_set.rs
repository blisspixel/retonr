use std::fmt;

use rewrite_inference::GenerationCandidate;
use rewrite_model::{
    CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptSetV1,
    CandidateGenerationReceiptSetV1Relations, CandidateGenerationReceiptV1,
    CandidateSelectionPolicyV1, GenerationQualificationContractError,
    GenerationQualificationPlanV1, GenerationRepetitionRecordV1,
    GenerationResourceAttemptResultRecordV1, GenerationSuiteManifestV1, GenerationSystemRecordV1,
    MAX_GENERATION_SUITE_CASES, MAX_PLANNED_GENERATION_ATTEMPTS, PlannedCandidateAttemptV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::active_generation_qualification_subject::{
    ActiveGenerationQualificationBinding, ActiveGenerationQualificationSubject,
};
use crate::{VerifiedCandidateBatch, VerifiedCandidateBatchError};

mod authority;
mod resource;
mod scope;
mod validation;
pub(crate) use scope::CandidateBatchSetScope;

use authority::{CandidateBatchSetAuthority, RetainedCandidateBatch, erase_core};
use resource::CandidateBatchSetResourceMode;
use validation::validate_fixed_closure;

/// Exact inert records consumed while creating one verified candidate-batch set.
///
/// The planned attempts must contain every plan attempt in exact plan order. The
/// batches supplied separately to [`VerifiedCandidateBatchSet::verify`] must be
/// in semantic suite order.
pub struct VerifiedCandidateBatchSetInput {
    /// Exact qualification plan.
    pub qualification_plan: GenerationQualificationPlanV1,
    /// Exact suite in semantic case order.
    pub suite: GenerationSuiteManifestV1,
    /// Exact repetition selected for this set.
    pub repetition: GenerationRepetitionRecordV1,
    /// Exact generation system selected for this set.
    pub generation_system: GenerationSystemRecordV1,
    /// Exact pre-output candidate-selection policy.
    pub selection_policy: CandidateSelectionPolicyV1,
    /// Every planned attempt in exact plan order.
    pub planned_attempts: Vec<PlannedCandidateAttemptV1>,
}

/// Opaque authority for one complete suite-ordered set of verified candidate batches.
///
/// The set retains every individual batch authority. Its portable receipt set and
/// selected candidates are available only after all retained evidence trees and
/// their complete portable closure have been revalidated.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCandidateBatchSet;
///
/// fn clone_capability(value: &VerifiedCandidateBatchSet) {
///     let _forged: VerifiedCandidateBatchSet = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCandidateBatchSet;
///
/// fn serialize_capability(value: &VerifiedCandidateBatchSet) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct VerifiedCandidateBatchSet {
    authority: Box<dyn CandidateBatchSetAuthority>,
}

impl VerifiedCandidateBatchSet {
    pub(crate) fn managed_evidence_inputs(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<
        Vec<(
            rewrite_model::PlannedCandidateAttemptId,
            rewrite_model::ManagedOllamaCandidateGenerationEvidenceV2Input,
        )>,
        VerifiedCandidateBatchSetError,
    > {
        self.authority.managed_evidence_inputs(cancellation)
    }

    pub(crate) fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding> {
        self.authority.active_binding()
    }

    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        self.active_binding()
            .is_some_and(|binding| subject.accepts(binding))
    }

    /// Joins one exact plan closure to one verified batch per suite case.
    ///
    /// Every batch is revalidated before portable records are read and again
    /// after the receipt set and live selections have been checked. Both the
    /// inert records and individual batch authorities are consumed.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchSetError`] for cancellation, fixed-bound
    /// violations, evidence-tree drift, or any missing, extra, substituted,
    /// reordered, failed, or incorrectly selected attempt.
    pub fn verify(
        input: VerifiedCandidateBatchSetInput,
        batches: Vec<VerifiedCandidateBatch>,
        cancellation: &CancellationToken,
    ) -> Result<Self, VerifiedCandidateBatchSetError> {
        VerifiedCandidateBatchSetCore::verify(input, batches, cancellation)
            .map(|core| erase_core(core, map_core_error))
            .map_err(map_core_error)
    }

    /// Joins one exact plan closure to one resource-observed batch per suite case.
    ///
    /// Every batch must retain one exact compiled resource result. Missing,
    /// mixed, or compatibility-only batches fail closed rather than being
    /// omitted from later phase evidence.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchSetError`] for cancellation, fixed-bound
    /// violations, evidence-tree drift, or any missing, mixed, substituted,
    /// reordered, failed, or incorrectly selected attempt or resource result.
    pub fn verify_resource_observed(
        input: VerifiedCandidateBatchSetInput,
        batches: Vec<VerifiedCandidateBatch>,
        cancellation: &CancellationToken,
    ) -> Result<Self, VerifiedCandidateBatchSetError> {
        VerifiedCandidateBatchSetCore::verify_resource_observed(input, batches, cancellation)
            .map(|core| erase_core(core, map_core_error))
            .map_err(map_core_error)
    }

    /// Revalidates every retained tree and the complete portable receipt closure.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchSetError`] for cancellation, tree drift,
    /// or a stale portable relationship.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCandidateBatchSetError> {
        self.authority.revalidate(cancellation)
    }

    /// Returns the portable receipt set after complete before-and-after revalidation.
    ///
    /// The returned record is inert and does not carry this capability.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchSetError`] for cancellation, tree drift,
    /// or a stale portable relationship.
    pub fn receipt_set(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<&CandidateGenerationReceiptSetV1, VerifiedCandidateBatchSetError> {
        self.authority.receipt_set(cancellation)
    }

    #[cfg(test)]
    pub(crate) fn attempt_records_for_test(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<CandidateGenerationAttemptRecordV1>, VerifiedCandidateBatchSetError> {
        self.authority.attempt_records_for_test(cancellation)
    }

    /// Returns the policy-selected live candidates in semantic suite order.
    ///
    /// Candidate text remains untrusted input to deterministic evaluation. The
    /// returned references do not copy the authority held by this set.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedCandidateBatchSetError`] for cancellation, tree drift,
    /// or any missing or substituted live selection.
    pub fn selected_candidates(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<&GenerationCandidate>, VerifiedCandidateBatchSetError> {
        self.authority.selected_candidates(cancellation)
    }

    /// Returns the exact semantic-order resource results for a strict set.
    ///
    /// Ordinary compatibility sets return `None`. Strict sets return one result
    /// per suite case only after complete before-and-after revalidation.
    pub(crate) fn resource_results(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Option<Vec<&GenerationResourceAttemptResultRecordV1>>, VerifiedCandidateBatchSetError>
    {
        self.authority.resource_results(cancellation)
    }
}

impl fmt::Debug for VerifiedCandidateBatchSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCandidateBatchSet")
            .field("batch_count", &self.authority.batch_count())
            .finish_non_exhaustive()
    }
}

/// Exact set-level relationship rejected while joining verified batches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifiedCandidateBatchSetRelationship {
    /// A supplied array exceeded its fixed protocol bound.
    FixedBound,
    /// Planned attempts were missing, extra, substituted, or reordered.
    PlannedAttemptClosure,
    /// The batch count did not equal the suite case count.
    BatchCount,
    /// A batch was not in exact semantic suite order.
    SemanticCaseOrder,
    /// A record named a different qualification plan or suite.
    QualificationClosure,
    /// A record named a different repetition.
    RepetitionClosure,
    /// A record named a different generation system.
    GenerationSystemClosure,
    /// A completed attempt or receipt named a different selected plan attempt.
    SelectedAttemptClosure,
    /// A policy selection was reordered, absent, or outside the live candidate set.
    SelectionClosure,
    /// Stored portable records no longer matched their retained batch authorities.
    PortableRecordClosure,
    /// An ordinary set contained one or more resource-result authorities.
    UnexpectedResourceResult,
    /// A strict set omitted one or more required resource-result authorities.
    MissingResourceResult,
    /// Ordered resource records no longer matched the retained batch closure.
    ResourceResultClosure,
    /// Candidate batches did not share one process-local Active operation subject.
    ActiveSubjectClosure,
}

/// Failure to create or revalidate a verified candidate-batch set.
#[derive(Error)]
pub enum VerifiedCandidateBatchSetError {
    /// Cooperative cancellation was observed.
    #[error("verified candidate batch-set operation was cancelled")]
    Cancelled,
    /// One batch evidence tree failed revalidation or selected-candidate access.
    #[error("verified candidate batch-set retained batch failed at index {index}")]
    Batch {
        /// Semantic suite index of the failing batch.
        index: usize,
        /// Exact typed batch failure.
        #[source]
        source: Box<VerifiedCandidateBatchError>,
    },
    /// One fixed set-level relationship differed.
    #[error("verified candidate batch-set relationship does not match: {0:?}")]
    Relationship(VerifiedCandidateBatchSetRelationship),
    /// The portable model contract rejected the exact set closure.
    #[error("verified candidate batch-set portable contract failed")]
    Contract(#[source] Box<GenerationQualificationContractError>),
}

impl fmt::Debug for VerifiedCandidateBatchSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("VerifiedCandidateBatchSetError");
        match self {
            Self::Cancelled => debug.field("kind", &"cancelled"),
            Self::Batch { index, .. } => debug.field("kind", &"batch").field("batch_index", index),
            Self::Relationship(relationship) => debug
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::Contract(_) => debug.field("kind", &"contract"),
        };
        debug.finish_non_exhaustive()
    }
}

struct VerifiedCandidateBatchSetCore<B> {
    active_binding: Option<ActiveGenerationQualificationBinding>,
    qualification_plan: GenerationQualificationPlanV1,
    suite: GenerationSuiteManifestV1,
    repetition: GenerationRepetitionRecordV1,
    generation_system: GenerationSystemRecordV1,
    selection_policy: CandidateSelectionPolicyV1,
    planned_attempts: Vec<PlannedCandidateAttemptV1>,
    attempt_records: Vec<CandidateGenerationAttemptRecordV1>,
    receipts: Vec<CandidateGenerationReceiptV1>,
    receipt_set: CandidateGenerationReceiptSetV1,
    resource_mode: CandidateBatchSetResourceMode,
    batches: Vec<B>,
}

#[derive(Debug)]
enum CoreError<E> {
    Cancelled,
    Batch { index: usize, source: E },
    Relationship(VerifiedCandidateBatchSetRelationship),
    Contract(GenerationQualificationContractError),
}

impl<B: RetainedCandidateBatch> VerifiedCandidateBatchSetCore<B> {
    fn verify(
        input: VerifiedCandidateBatchSetInput,
        batches: Vec<B>,
        cancellation: &CancellationToken,
    ) -> Result<Self, CoreError<B::Error>> {
        Self::verify_with_resource_mode(
            input,
            batches,
            CandidateBatchSetResourceMode::Ordinary,
            cancellation,
        )
    }

    fn verify_resource_observed(
        input: VerifiedCandidateBatchSetInput,
        batches: Vec<B>,
        cancellation: &CancellationToken,
    ) -> Result<Self, CoreError<B::Error>> {
        Self::verify_with_resource_mode(
            input,
            batches,
            CandidateBatchSetResourceMode::ResourceObserved,
            cancellation,
        )
    }

    fn verify_with_resource_mode(
        input: VerifiedCandidateBatchSetInput,
        batches: Vec<B>,
        resource_mode: CandidateBatchSetResourceMode,
        cancellation: &CancellationToken,
    ) -> Result<Self, CoreError<B::Error>> {
        ensure_active(cancellation)?;
        validate_bounds(&input, batches.len())?;
        revalidate_batches(&batches, cancellation)?;
        resource::validate_mode(resource_mode, &batches).map_err(CoreError::Relationship)?;
        validate_fixed_closure(&input, &batches).map_err(CoreError::Relationship)?;
        let active_binding = derive_active_binding(&batches).map_err(CoreError::Relationship)?;

        let attempt_records = batches
            .iter()
            .map(|batch| batch.attempt_record().clone())
            .collect::<Vec<_>>();
        let receipts = batches
            .iter()
            .map(|batch| batch.receipt().clone())
            .collect::<Vec<_>>();
        let receipt_set =
            derive_receipt_set(&input, &attempt_records, &receipts).map_err(CoreError::Contract)?;
        validate_live_selections(&input.selection_policy, &batches, cancellation)?;
        revalidate_batches(&batches, cancellation)?;
        ensure_active(cancellation)?;

        let VerifiedCandidateBatchSetInput {
            qualification_plan,
            suite,
            repetition,
            generation_system,
            selection_policy,
            planned_attempts,
        } = input;
        Ok(Self {
            active_binding,
            qualification_plan,
            suite,
            repetition,
            generation_system,
            selection_policy,
            planned_attempts,
            attempt_records,
            receipts,
            receipt_set,
            resource_mode,
            batches,
        })
    }

    fn revalidate(&self, cancellation: &CancellationToken) -> Result<(), CoreError<B::Error>> {
        ensure_active(cancellation)?;
        revalidate_batches(&self.batches, cancellation)?;
        resource::validate_mode(self.resource_mode, &self.batches)
            .map_err(CoreError::Relationship)?;
        let input = self.input();
        validate_bounds(&input, self.batches.len())?;
        validate_fixed_closure(&input, &self.batches).map_err(CoreError::Relationship)?;
        if self
            .batches
            .iter()
            .zip(&self.attempt_records)
            .zip(&self.receipts)
            .any(|((batch, attempt), receipt)| {
                batch.attempt_record() != attempt || batch.receipt() != receipt
            })
        {
            return Err(CoreError::Relationship(
                VerifiedCandidateBatchSetRelationship::PortableRecordClosure,
            ));
        }
        validate_active_binding(self.active_binding.as_ref(), &self.batches)
            .map_err(CoreError::Relationship)?;
        let rederived = derive_receipt_set(&input, &self.attempt_records, &self.receipts)
            .map_err(CoreError::Contract)?;
        if rederived != self.receipt_set {
            return Err(CoreError::Relationship(
                VerifiedCandidateBatchSetRelationship::PortableRecordClosure,
            ));
        }
        validate_live_selections(&self.selection_policy, &self.batches, cancellation)?;
        revalidate_batches(&self.batches, cancellation)?;
        ensure_active(cancellation)
    }

    fn receipt_set(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<&CandidateGenerationReceiptSetV1, CoreError<B::Error>> {
        self.revalidate(cancellation)?;
        Ok(&self.receipt_set)
    }

    fn selected_candidates(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<&GenerationCandidate>, CoreError<B::Error>> {
        self.revalidate(cancellation)?;
        let selected =
            collect_live_selections(&self.selection_policy, &self.batches, cancellation)?;
        self.revalidate(cancellation)?;
        Ok(selected)
    }

    fn resource_results(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Option<Vec<&GenerationResourceAttemptResultRecordV1>>, CoreError<B::Error>> {
        self.revalidate(cancellation)?;
        let results = resource::collect_results(self.resource_mode, &self.batches)
            .map_err(CoreError::Relationship)?;
        self.revalidate(cancellation)?;
        ensure_active(cancellation)?;
        Ok(results)
    }

    fn input(&self) -> VerifiedCandidateBatchSetInput {
        VerifiedCandidateBatchSetInput {
            qualification_plan: self.qualification_plan.clone(),
            suite: self.suite.clone(),
            repetition: self.repetition.clone(),
            generation_system: self.generation_system.clone(),
            selection_policy: self.selection_policy.clone(),
            planned_attempts: self.planned_attempts.clone(),
        }
    }
}

fn derive_active_binding<B: RetainedCandidateBatch>(
    batches: &[B],
) -> Result<Option<ActiveGenerationQualificationBinding>, VerifiedCandidateBatchSetRelationship> {
    let Some(first) = batches.first() else {
        return Ok(None);
    };
    let first = first.active_binding();
    if batches
        .iter()
        .all(|batch| bindings_match(first, batch.active_binding()))
    {
        Ok(first.cloned())
    } else {
        Err(VerifiedCandidateBatchSetRelationship::ActiveSubjectClosure)
    }
}

fn validate_active_binding<B: RetainedCandidateBatch>(
    expected: Option<&ActiveGenerationQualificationBinding>,
    batches: &[B],
) -> Result<(), VerifiedCandidateBatchSetRelationship> {
    if batches
        .iter()
        .all(|batch| bindings_match(expected, batch.active_binding()))
    {
        Ok(())
    } else {
        Err(VerifiedCandidateBatchSetRelationship::ActiveSubjectClosure)
    }
}

fn bindings_match(
    left: Option<&ActiveGenerationQualificationBinding>,
    right: Option<&ActiveGenerationQualificationBinding>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => left.same_subject(right),
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn validate_bounds<E>(
    input: &VerifiedCandidateBatchSetInput,
    batch_count: usize,
) -> Result<(), CoreError<E>> {
    if input.suite.case_ids().len() > MAX_GENERATION_SUITE_CASES
        || batch_count > MAX_GENERATION_SUITE_CASES
        || input.planned_attempts.len() > MAX_PLANNED_GENERATION_ATTEMPTS
    {
        return Err(CoreError::Relationship(
            VerifiedCandidateBatchSetRelationship::FixedBound,
        ));
    }
    Ok(())
}

fn derive_receipt_set(
    input: &VerifiedCandidateBatchSetInput,
    attempt_records: &[CandidateGenerationAttemptRecordV1],
    receipts: &[CandidateGenerationReceiptV1],
) -> Result<CandidateGenerationReceiptSetV1, GenerationQualificationContractError> {
    CandidateGenerationReceiptSetV1::new(CandidateGenerationReceiptSetV1Relations {
        qualification_plan: &input.qualification_plan,
        suite: &input.suite,
        repetition: &input.repetition,
        generation_system: &input.generation_system,
        selection_policy: &input.selection_policy,
        planned_attempts: &input.planned_attempts,
        attempt_records,
        receipts,
    })
}

fn revalidate_batches<B: RetainedCandidateBatch>(
    batches: &[B],
    cancellation: &CancellationToken,
) -> Result<(), CoreError<B::Error>> {
    for (index, batch) in batches.iter().enumerate() {
        ensure_active(cancellation)?;
        batch
            .revalidate(cancellation)
            .map_err(|source| CoreError::Batch { index, source })?;
    }
    Ok(())
}

fn validate_live_selections<B: RetainedCandidateBatch>(
    selection_policy: &CandidateSelectionPolicyV1,
    batches: &[B],
    cancellation: &CancellationToken,
) -> Result<(), CoreError<B::Error>> {
    collect_live_selections(selection_policy, batches, cancellation).map(|_| ())
}

fn collect_live_selections<'a, B: RetainedCandidateBatch>(
    selection_policy: &CandidateSelectionPolicyV1,
    batches: &'a [B],
    cancellation: &CancellationToken,
) -> Result<Vec<&'a GenerationCandidate>, CoreError<B::Error>> {
    let mut selected = Vec::with_capacity(batches.len());
    for (index, (entry, batch)) in selection_policy.entries().iter().zip(batches).enumerate() {
        ensure_active(cancellation)?;
        let candidate = batch
            .candidate(entry.selected_ordinal(), cancellation)
            .map_err(|source| CoreError::Batch { index, source })?
            .ok_or(CoreError::Relationship(
                VerifiedCandidateBatchSetRelationship::SelectionClosure,
            ))?;
        selected.push(candidate);
    }
    Ok(selected)
}

fn ensure_active<E>(cancellation: &CancellationToken) -> Result<(), CoreError<E>> {
    if cancellation.is_cancelled() {
        Err(CoreError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_core_error(error: CoreError<VerifiedCandidateBatchError>) -> VerifiedCandidateBatchSetError {
    match error {
        CoreError::Cancelled => VerifiedCandidateBatchSetError::Cancelled,
        CoreError::Batch { index, source } => VerifiedCandidateBatchSetError::Batch {
            index,
            source: Box::new(source),
        },
        CoreError::Relationship(relationship) => {
            VerifiedCandidateBatchSetError::Relationship(relationship)
        }
        CoreError::Contract(source) => VerifiedCandidateBatchSetError::Contract(Box::new(source)),
    }
}

#[cfg(test)]
#[path = "verified_candidate_batch_set/tests.rs"]
pub(crate) mod tests;
