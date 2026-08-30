use std::fmt;

use rewrite_inference::{
    CandidateOutputPolicy, StructuredCompletionRequest, parse_candidate_output,
};
use rewrite_model::{
    CandidateArtifactEntryV1, CandidateGenerationAttemptPrecursorV1,
    CandidateGenerationAttemptRecordV1, CandidateGenerationCleanupRecordV1,
    CandidateGenerationEvidenceBundleEntryV1, CandidateGenerationEvidenceBundleRoleV1,
    CandidateGenerationReceiptV1, CandidateGenerationReceiptV1Relations,
    CandidateGenerationUsageObservationV1, EffectivePackageEvidenceV2, GenerationCaseManifestV1,
    GenerationClusterRecordV1, GenerationQualificationContractError, GenerationQualificationPlanV1,
    GenerationRepetitionRecordV1, GenerationSuiteManifestV1, GenerationSystemRecordV1,
    ManagedOllamaCandidateGenerationEvidenceV2, ManagedOllamaCandidateGenerationEvidenceV2Input,
    ManagedOllamaCandidateGenerationEvidenceV2Relations, PlannedCandidateAttemptV1,
    PlannedCandidateAttemptV1Relations,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    CandidateGenerationEvidenceBundleError, CandidateGenerationEvidenceBundleReadbackLease,
    RetainedStructuredResponseArtifactError, RetainedStructuredResponseArtifactV1,
};

/// Exact typed relationships and consuming readback lease for one receipt compilation.
pub struct CandidateGenerationReceiptCompilationInput<'a> {
    /// Qualification plan that selected the attempt.
    pub qualification_plan: &'a GenerationQualificationPlanV1,
    /// Suite selected by the qualification plan.
    pub suite: &'a GenerationSuiteManifestV1,
    /// Exact generation case.
    pub case: &'a GenerationCaseManifestV1,
    /// Cluster containing the case.
    pub cluster: &'a GenerationClusterRecordV1,
    /// Exact suite repetition.
    pub repetition: &'a GenerationRepetitionRecordV1,
    /// Attempt selected before execution.
    pub planned_attempt: &'a PlannedCandidateAttemptV1,
    /// Stable generation system selected by the attempt.
    pub generation_system: &'a GenerationSystemRecordV1,
    /// Cleanup-gated effective package used by managed evidence.
    pub effective_package: &'a EffectivePackageEvidenceV2,
    /// Exact prelaunch attempt precursor.
    pub precursor: &'a CandidateGenerationAttemptPrecursorV1,
    /// Managed bracket evidence produced by execution.
    pub managed_evidence: &'a ManagedOllamaCandidateGenerationEvidenceV2,
    /// Successful cleanup and final revalidation record.
    pub cleanup: &'a CandidateGenerationCleanupRecordV1,
    /// Exact structured request used to reconstruct the response.
    pub structured_request: &'a StructuredCompletionRequest,
    /// Consuming lease over the freshly verified published evidence tree.
    pub readback_lease: CandidateGenerationEvidenceBundleReadbackLease,
}

/// Noncloneable inert material reloaded from one cleanup-and-readback-gated bundle.
///
/// This value proves only exact portable records and retained bytes. It is not live
/// execution authority and cannot by itself create a verified candidate batch.
pub struct CandidateGenerationReceiptCompilation {
    lease: CandidateGenerationEvidenceBundleReadbackLease,
    receipt: CandidateGenerationReceiptV1,
    attempt_record: CandidateGenerationAttemptRecordV1,
}

impl CandidateGenerationReceiptCompilation {
    /// Returns the retained exact evidence-tree lease.
    #[must_use]
    pub const fn lease(&self) -> &CandidateGenerationEvidenceBundleReadbackLease {
        &self.lease
    }

    /// Returns the response-derived durable receipt.
    #[must_use]
    pub const fn receipt(&self) -> &CandidateGenerationReceiptV1 {
        &self.receipt
    }

    /// Returns the completed attempt record derived from the receipt.
    #[must_use]
    pub const fn attempt_record(&self) -> &CandidateGenerationAttemptRecordV1 {
        &self.attempt_record
    }

    /// Returns the exact response-derived candidate count.
    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.lease.manifest().candidate_artifacts().len()
    }

    /// Returns one exact candidate entry by its canonical ordinal.
    #[must_use]
    pub fn candidate_entry(&self, ordinal: u8) -> Option<&CandidateArtifactEntryV1> {
        self.lease
            .manifest()
            .candidate_artifacts()
            .get(usize::from(ordinal))
            .filter(|entry| entry.ordinal() == ordinal)
    }

    /// Reads one candidate from its retained exact member after full revalidation.
    ///
    /// # Errors
    ///
    /// Returns an error for a foreign ordinal, cancellation, or any tree or byte drift.
    pub fn candidate_bytes(
        &self,
        ordinal: u8,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, CandidateGenerationReceiptCompilationError> {
        let entry = self
            .candidate_entry(ordinal)
            .ok_or(CandidateGenerationReceiptCompilationError::RelationshipMismatch)?;
        self.lease
            .member_bytes(entry.relative_path(), entry.byte_count(), cancellation)
            .map_err(Into::into)
    }

    /// Revalidates the complete held and named evidence tree.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation or any root, manifest, member, or digest drift.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateGenerationReceiptCompilationError> {
        self.lease.revalidate(cancellation).map_err(Into::into)
    }

    /// Consumes the capability into its lease and portable completed records.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        CandidateGenerationEvidenceBundleReadbackLease,
        CandidateGenerationReceiptV1,
        CandidateGenerationAttemptRecordV1,
    ) {
        (self.lease, self.receipt, self.attempt_record)
    }
}

impl fmt::Debug for CandidateGenerationReceiptCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationReceiptCompilation")
            .field("bundle_id", self.lease.bundle_id())
            .field("receipt_id", self.receipt.receipt_id())
            .field("attempt_record_id", self.attempt_record.attempt_record_id())
            .finish_non_exhaustive()
    }
}

/// Failure while reloading a published bundle and deriving its completed receipt.
#[derive(Debug, Error)]
pub enum CandidateGenerationReceiptCompilationError {
    /// Published bundle verification or retained readback failed.
    #[error("candidate receipt bundle readback failed")]
    Bundle(#[from] CandidateGenerationEvidenceBundleError),
    /// A model-owned portable record or relationship was invalid.
    #[error("candidate receipt portable contract failed")]
    Contract(#[from] GenerationQualificationContractError),
    /// The structured response artifact could not be reconstructed exactly.
    #[error("candidate receipt response artifact failed")]
    ResponseArtifact(#[from] RetainedStructuredResponseArtifactError),
    /// The provider-neutral candidate envelope was invalid.
    #[error("candidate receipt response candidates failed validation")]
    CandidateOutput(#[from] rewrite_inference::CandidateOutputError),
    /// Independently supplied and reloaded evidence did not name one chain.
    #[error("candidate receipt evidence relationship does not match")]
    RelationshipMismatch,
}

/// Compiler for one cleanup-and-readback-gated completed receipt.
#[derive(Clone, Copy, Debug, Default)]
pub struct CandidateGenerationReceiptCompiler;

impl CandidateGenerationReceiptCompiler {
    /// Reloads every fixed record and candidate before deriving a completed receipt.
    ///
    /// Usage is taken only from the reconstructed retained response. The input
    /// readback lease is consumed and retained by the successful result.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, tree drift, invalid canonical records,
    /// substituted relationships, response reconstruction, or candidate mismatch.
    pub fn compile(
        input: CandidateGenerationReceiptCompilationInput<'_>,
        cancellation: &CancellationToken,
    ) -> Result<CandidateGenerationReceiptCompilation, CandidateGenerationReceiptCompilationError>
    {
        let CandidateGenerationReceiptCompilationInput {
            qualification_plan,
            suite,
            case,
            cluster,
            repetition,
            planned_attempt,
            generation_system,
            effective_package,
            precursor,
            managed_evidence,
            cleanup,
            structured_request,
            readback_lease,
        } = input;
        readback_lease.revalidate(cancellation)?;
        let records = reload_records(
            &readback_lease,
            &RecordRelations {
                qualification_plan,
                suite,
                case,
                cluster,
                repetition,
                generation_system,
                effective_package,
                structured_request,
                expected_planned_attempt: planned_attempt,
                expected_precursor: precursor,
                expected_managed_evidence: managed_evidence,
                expected_cleanup: cleanup,
            },
            cancellation,
        )?;
        verify_candidates(&readback_lease, &records, case, cancellation)?;
        let usage = records.response.usage();
        let usage = CandidateGenerationUsageObservationV1::new(
            usage.input_tokens,
            usage.output_tokens,
            usage.generation_micros,
        );
        let receipt = CandidateGenerationReceiptV1::new(
            CandidateGenerationReceiptV1Relations {
                qualification_plan,
                suite,
                case,
                cluster,
                repetition,
                planned_attempt: &records.planned_attempt,
                precursor: &records.precursor,
                generation_system,
                managed_evidence: &records.managed_evidence,
                cleanup: &records.cleanup,
                bundle: readback_lease.manifest(),
                readback: readback_lease.readback(),
            },
            usage,
        )?;
        let attempt_record = CandidateGenerationAttemptRecordV1::completed(
            &records.planned_attempt,
            &records.precursor,
            &receipt,
        )?;
        readback_lease.revalidate(cancellation)?;
        Ok(CandidateGenerationReceiptCompilation {
            lease: readback_lease,
            receipt,
            attempt_record,
        })
    }
}

struct RecordRelations<'a> {
    qualification_plan: &'a GenerationQualificationPlanV1,
    suite: &'a GenerationSuiteManifestV1,
    case: &'a GenerationCaseManifestV1,
    cluster: &'a GenerationClusterRecordV1,
    repetition: &'a GenerationRepetitionRecordV1,
    generation_system: &'a GenerationSystemRecordV1,
    effective_package: &'a EffectivePackageEvidenceV2,
    structured_request: &'a StructuredCompletionRequest,
    expected_planned_attempt: &'a PlannedCandidateAttemptV1,
    expected_precursor: &'a CandidateGenerationAttemptPrecursorV1,
    expected_managed_evidence: &'a ManagedOllamaCandidateGenerationEvidenceV2,
    expected_cleanup: &'a CandidateGenerationCleanupRecordV1,
}

struct ReloadedRecords {
    planned_attempt: PlannedCandidateAttemptV1,
    precursor: CandidateGenerationAttemptPrecursorV1,
    managed_evidence: ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: CandidateGenerationCleanupRecordV1,
    response: RetainedStructuredResponseArtifactV1,
}

fn reload_records(
    lease: &CandidateGenerationEvidenceBundleReadbackLease,
    relations: &RecordRelations<'_>,
    cancellation: &CancellationToken,
) -> Result<ReloadedRecords, CandidateGenerationReceiptCompilationError> {
    let planned_attempt = PlannedCandidateAttemptV1::from_json_bytes(
        &read_role(
            lease,
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            cancellation,
        )?,
        PlannedCandidateAttemptV1Relations {
            suite: relations.suite,
            case: relations.case,
            cluster: relations.cluster,
            repetition: relations.repetition,
            generation_system: relations.generation_system,
        },
    )?;
    require_equal(&planned_attempt, relations.expected_planned_attempt)?;
    let precursor = CandidateGenerationAttemptPrecursorV1::from_json_bytes(
        &read_role(
            lease,
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            cancellation,
        )?,
        relations.qualification_plan,
        &planned_attempt,
        relations.generation_system,
    )?;
    require_equal(&precursor, relations.expected_precursor)?;
    let managed_input = ManagedOllamaCandidateGenerationEvidenceV2Input {
        bracket_observation_v1_id: relations
            .expected_managed_evidence
            .bracket_observation_v1_id()
            .clone(),
        effective_runtime_state_join_id: relations
            .expected_managed_evidence
            .effective_runtime_state_join_id()
            .clone(),
        response_id: relations.expected_managed_evidence.response_id().clone(),
    };
    let managed_evidence = ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
        &read_role(
            lease,
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            cancellation,
        )?,
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: &planned_attempt,
            generation_system: relations.generation_system,
            effective_package_evidence_v2: relations.effective_package,
        },
        &managed_input,
    )?;
    require_equal(&managed_evidence, relations.expected_managed_evidence)?;
    let cleanup = CandidateGenerationCleanupRecordV1::from_json_bytes(
        &read_role(
            lease,
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            cancellation,
        )?,
        &precursor,
        &managed_evidence,
    )?;
    require_equal(&cleanup, relations.expected_cleanup)?;
    let response = RetainedStructuredResponseArtifactV1::from_json_bytes(
        &read_role(
            lease,
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            cancellation,
        )?,
        relations.structured_request,
        managed_evidence.response_id(),
    )?;
    Ok(ReloadedRecords {
        planned_attempt,
        precursor,
        managed_evidence,
        cleanup,
        response,
    })
}

fn verify_candidates(
    lease: &CandidateGenerationEvidenceBundleReadbackLease,
    records: &ReloadedRecords,
    case: &GenerationCaseManifestV1,
    cancellation: &CancellationToken,
) -> Result<(), CandidateGenerationReceiptCompilationError> {
    let ceilings = records.planned_attempt.output_ceilings();
    let policy = CandidateOutputPolicy::new(
        ceilings.candidate_count(),
        ceilings.maximum_candidate_bytes(),
        ceilings.maximum_aggregate_candidate_bytes(),
    )?;
    if policy.maximum_envelope_bytes() != ceilings.maximum_envelope_bytes() {
        return Err(CandidateGenerationReceiptCompilationError::RelationshipMismatch);
    }
    let candidates = parse_candidate_output(records.response.output_json().as_bytes(), policy)?;
    let expected = lease.manifest().candidate_artifacts();
    if candidates.len() != expected.len() {
        return Err(CandidateGenerationReceiptCompilationError::RelationshipMismatch);
    }
    for (candidate, expected) in candidates.iter().zip(expected) {
        let bytes = lease.member_bytes_inside_revalidation(
            expected.relative_path(),
            expected.byte_count(),
            cancellation,
        )?;
        let derived = CandidateArtifactEntryV1::new(
            &records.precursor,
            &records.planned_attempt,
            case,
            candidate.ordinal,
            expected.relative_path().clone(),
            &bytes,
        )?;
        if bytes != candidate.text.as_bytes() || &derived != expected {
            return Err(CandidateGenerationReceiptCompilationError::RelationshipMismatch);
        }
    }
    Ok(())
}

fn read_role(
    lease: &CandidateGenerationEvidenceBundleReadbackLease,
    role: CandidateGenerationEvidenceBundleRoleV1,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, CandidateGenerationReceiptCompilationError> {
    let entry = unique_role_entry(lease, role)?;
    lease
        .member_bytes_inside_revalidation(entry.relative_path(), entry.byte_size(), cancellation)
        .map_err(Into::into)
}

fn unique_role_entry(
    lease: &CandidateGenerationEvidenceBundleReadbackLease,
    role: CandidateGenerationEvidenceBundleRoleV1,
) -> Result<&CandidateGenerationEvidenceBundleEntryV1, CandidateGenerationReceiptCompilationError> {
    let mut matches = lease
        .manifest()
        .entries()
        .iter()
        .filter(|entry| entry.role() == role);
    let entry = matches
        .next()
        .ok_or(CandidateGenerationReceiptCompilationError::RelationshipMismatch)?;
    if matches.next().is_some() {
        return Err(CandidateGenerationReceiptCompilationError::RelationshipMismatch);
    }
    Ok(entry)
}

fn require_equal<T: PartialEq>(
    observed: &T,
    expected: &T,
) -> Result<(), CandidateGenerationReceiptCompilationError> {
    if observed == expected {
        Ok(())
    } else {
        Err(CandidateGenerationReceiptCompilationError::RelationshipMismatch)
    }
}
