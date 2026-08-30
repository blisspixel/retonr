use rewrite_types::{Digest, ReasonCode, RewriteStatus};

use super::GenerationDeterministicCaseContractV1;
use crate::{ArtifactId, ExpectedOutput, ReferenceJudgment};

impl GenerationDeterministicCaseContractV1 {
    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the canonical case machine key.
    #[must_use]
    pub fn case_key(&self) -> &str {
        &self.case_key
    }

    /// Returns the exact source artifact identity.
    #[must_use]
    pub const fn source_artifact_id(&self) -> &ArtifactId {
        &self.source_artifact_id
    }

    /// Returns the exact source digest.
    #[must_use]
    pub const fn source_digest(&self) -> &Digest {
        &self.source_digest
    }

    /// Returns the exact source byte count.
    #[must_use]
    pub const fn source_byte_count(&self) -> u64 {
        self.source_byte_count
    }

    /// Returns the exact language declaration digest.
    #[must_use]
    pub const fn language_digest(&self) -> &Digest {
        &self.language_digest
    }

    /// Returns the exact operation-mode digest.
    #[must_use]
    pub const fn mode_digest(&self) -> &Digest {
        &self.mode_digest
    }

    /// Returns the exact format-contract digest.
    #[must_use]
    pub const fn format_digest(&self) -> &Digest {
        &self.format_digest
    }

    /// Returns the stable deterministic-report category.
    #[must_use]
    pub fn evaluation_category(&self) -> &str {
        &self.evaluation_category
    }

    /// Returns the exact protected terms in canonical order.
    #[must_use]
    pub fn protected_terms(&self) -> &[String] {
        &self.protected_terms
    }

    /// Returns the predeclared human fixture judgment.
    #[must_use]
    pub const fn reference_judgment(&self) -> ReferenceJudgment {
        self.reference_judgment
    }

    /// Returns the exact expected transaction status.
    #[must_use]
    pub const fn expected_status(&self) -> RewriteStatus {
        self.expected_status
    }

    /// Returns the exact optional expected transaction reason.
    #[must_use]
    pub const fn expected_reason(&self) -> Option<ReasonCode> {
        self.expected_reason
    }

    /// Returns the exact expected output identity.
    #[must_use]
    pub const fn expected_output(&self) -> ExpectedOutput {
        self.expected_output
    }

    /// Returns the exact rubric clause identifiers in canonical order.
    #[must_use]
    pub fn rubric_clause_ids(&self) -> &[String] {
        &self.rubric_clause_ids
    }

    /// Returns the domain-separated digest embedded by the case manifest.
    #[must_use]
    pub const fn contract_digest(&self) -> &Digest {
        &self.contract_digest
    }
}
