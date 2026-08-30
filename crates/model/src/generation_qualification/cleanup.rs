use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::codec::{append_count, append_digest, append_u32, validate_canonical_json};
use super::{
    CANDIDATE_GENERATION_CLEANUP_ID_DOMAIN, CandidateGenerationAttemptPrecursorId,
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationCleanupId,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationContractError,
    ManagedOllamaCandidateGenerationEvidenceV2, ManagedOllamaCandidateGenerationEvidenceV2Id,
};
use rewrite_types::Digest;

mod wire;

use wire::CleanupWire;

/// Maximum JSON bytes accepted for one candidate-generation cleanup record.
pub const MAX_CANDIDATE_GENERATION_CLEANUP_JSON_BYTES: usize = 16_384;
const MAX_CANDIDATE_GENERATION_CLEANUP_CANONICAL_BYTES: usize = 4_096;
const MAX_CANDIDATE_GENERATION_CLEANUP_FAILURE_CATEGORIES: usize = 3;

/// Closed status for managed process and retained-connection cleanup.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationProcessCleanupStatusV1 {
    /// The managed process, connection, and every retained child closed.
    Succeeded,
    /// At least one required process cleanup postcondition failed.
    Failed,
}

/// Closed status for final exact package-lease revalidation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationPackageRevalidationStatusV1 {
    /// The exact retained package lease was freshly revalidated.
    Verified,
    /// Fresh revalidation observed changed retained package bytes or identity.
    Changed,
    /// Fresh revalidation could not complete successfully.
    Failed,
}

/// Closed diagnostic category for one failed cleanup postcondition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationCleanupFailureCategoryV1 {
    /// Managed process or retained-connection cleanup failed.
    ProcessCleanupFailed,
    /// Final runtime-package revalidation observed changed bytes or identity.
    RuntimePackageChanged,
    /// Final runtime-package revalidation failed to complete.
    RuntimePackageRevalidationFailed,
    /// Final model-package revalidation observed changed bytes or identity.
    ModelPackageChanged,
    /// Final model-package revalidation failed to complete.
    ModelPackageRevalidationFailed,
}

/// Closed cleanup and package-revalidation observations for one attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateGenerationCleanupRecordV1Input {
    /// Managed process and retained-connection cleanup status.
    pub process_cleanup_status: CandidateGenerationProcessCleanupStatusV1,
    /// Final runtime-package revalidation status.
    pub runtime_package_revalidation_status: CandidateGenerationPackageRevalidationStatusV1,
    /// Final model-package revalidation status.
    pub model_package_revalidation_status: CandidateGenerationPackageRevalidationStatusV1,
}

/// Inert diagnostic record for cleanup and final package revalidation.
///
/// A failed record preserves every applicable closed failure category. Even an
/// all-pass record grants no receipt, qualification, or live-use authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationCleanupRecordV1 {
    schema_version: u32,
    precursor_id: CandidateGenerationAttemptPrecursorId,
    managed_evidence_id: ManagedOllamaCandidateGenerationEvidenceV2Id,
    process_cleanup_status: CandidateGenerationProcessCleanupStatusV1,
    runtime_package_revalidation_status: CandidateGenerationPackageRevalidationStatusV1,
    model_package_revalidation_status: CandidateGenerationPackageRevalidationStatusV1,
    failure_categories: Vec<CandidateGenerationCleanupFailureCategoryV1>,
    #[serde(skip)]
    id: CandidateGenerationCleanupId,
}

impl CandidateGenerationCleanupRecordV1 {
    /// Creates one cleanup record with its complete derived failure-category closure.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless the managed evidence
    /// names the supplied exact precursor.
    pub fn new(
        precursor: &CandidateGenerationAttemptPrecursorV1,
        managed_evidence: &ManagedOllamaCandidateGenerationEvidenceV2,
        input: CandidateGenerationCleanupRecordV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            precursor,
            managed_evidence,
            input,
            None,
        )
    }

    fn build(
        schema_version: u32,
        precursor: &CandidateGenerationAttemptPrecursorV1,
        managed_evidence: &ManagedOllamaCandidateGenerationEvidenceV2,
        input: CandidateGenerationCleanupRecordV1Input,
        wire: Option<&CleanupWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        if managed_evidence.precursor_id() != precursor.precursor_id() {
            return Err(GenerationQualificationContractError::CleanupRelationshipMismatch);
        }
        if wire.is_some_and(|value| !value.matches(precursor, managed_evidence)) {
            return Err(GenerationQualificationContractError::CleanupRelationshipMismatch);
        }
        let failure_categories = failure_categories(input);
        if failure_categories.len() > MAX_CANDIDATE_GENERATION_CLEANUP_FAILURE_CATEGORIES
            || wire.is_some_and(|value| value.failure_categories() != failure_categories)
        {
            return Err(GenerationQualificationContractError::InvalidCleanupStatusClosure);
        }
        let mut record = Self {
            schema_version,
            precursor_id: precursor.precursor_id().clone(),
            managed_evidence_id: managed_evidence.managed_evidence_v2_id().clone(),
            process_cleanup_status: input.process_cleanup_status,
            runtime_package_revalidation_status: input.runtime_package_revalidation_status,
            model_package_revalidation_status: input.model_package_revalidation_status,
            failure_categories,
            id: CandidateGenerationCleanupId(Digest::sha256(b"uninitialized cleanup record")),
        };
        let canonical = record.canonical_bytes()?;
        if canonical.len() > MAX_CANDIDATE_GENERATION_CLEANUP_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        record.id = CandidateGenerationCleanupId(Digest::sha256(&canonical));
        Ok(record)
    }

    /// Parses canonical bounded JSON and reloads the exact precursor-evidence join.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// unsupported, noncanonical, substituted, or incomplete status closure.
    pub fn from_json_bytes(
        bytes: &[u8],
        precursor: &CandidateGenerationAttemptPrecursorV1,
        managed_evidence: &ManagedOllamaCandidateGenerationEvidenceV2,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_GENERATION_CLEANUP_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: CleanupWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version() != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version(),
            ));
        }
        let record = Self::build(
            wire.schema_version(),
            precursor,
            managed_evidence,
            wire.input(),
            Some(&wire),
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Revalidates this record against its exact precursor and managed evidence.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if any relationship,
    /// status, failure category, or content-derived identity differs.
    pub fn validate_against(
        &self,
        precursor: &CandidateGenerationAttemptPrecursorV1,
        managed_evidence: &ManagedOllamaCandidateGenerationEvidenceV2,
    ) -> Result<(), GenerationQualificationContractError> {
        let expected = Self::new(
            precursor,
            managed_evidence,
            CandidateGenerationCleanupRecordV1Input {
                process_cleanup_status: self.process_cleanup_status,
                runtime_package_revalidation_status: self.runtime_package_revalidation_status,
                model_package_revalidation_status: self.model_package_revalidation_status,
            },
        )?;
        if &expected == self {
            Ok(())
        } else {
            Err(GenerationQualificationContractError::InvalidCleanupStatusClosure)
        }
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact precursor identity.
    #[must_use]
    pub const fn precursor_id(&self) -> &CandidateGenerationAttemptPrecursorId {
        &self.precursor_id
    }
    /// Returns the exact managed-evidence identity.
    #[must_use]
    pub const fn managed_evidence_id(&self) -> &ManagedOllamaCandidateGenerationEvidenceV2Id {
        &self.managed_evidence_id
    }
    /// Returns the managed process cleanup status.
    #[must_use]
    pub const fn process_cleanup_status(&self) -> CandidateGenerationProcessCleanupStatusV1 {
        self.process_cleanup_status
    }
    /// Returns the final runtime-package revalidation status.
    #[must_use]
    pub const fn runtime_package_revalidation_status(
        &self,
    ) -> CandidateGenerationPackageRevalidationStatusV1 {
        self.runtime_package_revalidation_status
    }
    /// Returns the final model-package revalidation status.
    #[must_use]
    pub const fn model_package_revalidation_status(
        &self,
    ) -> CandidateGenerationPackageRevalidationStatusV1 {
        self.model_package_revalidation_status
    }
    /// Returns every applicable failure category in canonical status order.
    #[must_use]
    pub fn failure_categories(&self) -> &[CandidateGenerationCleanupFailureCategoryV1] {
        &self.failure_categories
    }
    /// Returns the content-derived cleanup identity.
    #[must_use]
    pub const fn cleanup_id(&self) -> &CandidateGenerationCleanupId {
        &self.id
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, GenerationQualificationContractError> {
        let mut output = CANDIDATE_GENERATION_CLEANUP_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.precursor_id.digest());
        append_digest(&mut output, self.managed_evidence_id.digest());
        output.push(process_cleanup_status_tag(self.process_cleanup_status));
        output.push(package_revalidation_status_tag(
            self.runtime_package_revalidation_status,
        ));
        output.push(package_revalidation_status_tag(
            self.model_package_revalidation_status,
        ));
        append_count(&mut output, self.failure_categories.len())?;
        output.extend(
            self.failure_categories
                .iter()
                .copied()
                .map(failure_category_tag),
        );
        Ok(output)
    }
}

impl fmt::Debug for CandidateGenerationCleanupRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationCleanupRecordV1")
            .field("schema_version", &self.schema_version)
            .field("cleanup_id", &self.id)
            .field("process_cleanup_status", &self.process_cleanup_status)
            .field(
                "runtime_package_revalidation_status",
                &self.runtime_package_revalidation_status,
            )
            .field(
                "model_package_revalidation_status",
                &self.model_package_revalidation_status,
            )
            .field("failure_category_count", &self.failure_categories.len())
            .finish_non_exhaustive()
    }
}

fn failure_categories(
    input: CandidateGenerationCleanupRecordV1Input,
) -> Vec<CandidateGenerationCleanupFailureCategoryV1> {
    let mut categories = Vec::with_capacity(MAX_CANDIDATE_GENERATION_CLEANUP_FAILURE_CATEGORIES);
    if input.process_cleanup_status == CandidateGenerationProcessCleanupStatusV1::Failed {
        categories.push(CandidateGenerationCleanupFailureCategoryV1::ProcessCleanupFailed);
    }
    if let Some(category) = runtime_failure_category(input.runtime_package_revalidation_status) {
        categories.push(category);
    }
    if let Some(category) = model_failure_category(input.model_package_revalidation_status) {
        categories.push(category);
    }
    categories
}

const fn runtime_failure_category(
    status: CandidateGenerationPackageRevalidationStatusV1,
) -> Option<CandidateGenerationCleanupFailureCategoryV1> {
    match status {
        CandidateGenerationPackageRevalidationStatusV1::Verified => None,
        CandidateGenerationPackageRevalidationStatusV1::Changed => {
            Some(CandidateGenerationCleanupFailureCategoryV1::RuntimePackageChanged)
        }
        CandidateGenerationPackageRevalidationStatusV1::Failed => {
            Some(CandidateGenerationCleanupFailureCategoryV1::RuntimePackageRevalidationFailed)
        }
    }
}

const fn model_failure_category(
    status: CandidateGenerationPackageRevalidationStatusV1,
) -> Option<CandidateGenerationCleanupFailureCategoryV1> {
    match status {
        CandidateGenerationPackageRevalidationStatusV1::Verified => None,
        CandidateGenerationPackageRevalidationStatusV1::Changed => {
            Some(CandidateGenerationCleanupFailureCategoryV1::ModelPackageChanged)
        }
        CandidateGenerationPackageRevalidationStatusV1::Failed => {
            Some(CandidateGenerationCleanupFailureCategoryV1::ModelPackageRevalidationFailed)
        }
    }
}

const fn process_cleanup_status_tag(value: CandidateGenerationProcessCleanupStatusV1) -> u8 {
    match value {
        CandidateGenerationProcessCleanupStatusV1::Succeeded => 0,
        CandidateGenerationProcessCleanupStatusV1::Failed => 1,
    }
}

const fn package_revalidation_status_tag(
    value: CandidateGenerationPackageRevalidationStatusV1,
) -> u8 {
    match value {
        CandidateGenerationPackageRevalidationStatusV1::Verified => 0,
        CandidateGenerationPackageRevalidationStatusV1::Changed => 1,
        CandidateGenerationPackageRevalidationStatusV1::Failed => 2,
    }
}

const fn failure_category_tag(value: CandidateGenerationCleanupFailureCategoryV1) -> u8 {
    match value {
        CandidateGenerationCleanupFailureCategoryV1::ProcessCleanupFailed => 0,
        CandidateGenerationCleanupFailureCategoryV1::RuntimePackageChanged => 1,
        CandidateGenerationCleanupFailureCategoryV1::RuntimePackageRevalidationFailed => 2,
        CandidateGenerationCleanupFailureCategoryV1::ModelPackageChanged => 3,
        CandidateGenerationCleanupFailureCategoryV1::ModelPackageRevalidationFailed => 4,
    }
}
