use std::fmt;

use rewrite_model::GenerationCaseRequestProfileV1;
use rewrite_types::Digest;

use super::{GenerationQualificationRequestBuildError, digest};

/// Internally derived generation-system bindings owned by the V1 request builder.
///
/// Validator and adapter digests remain reviewed equality bindings. This type
/// does not claim that either component's semantics have been independently proven.
#[derive(Clone, Eq, PartialEq)]
#[expect(
    clippy::struct_field_names,
    reason = "the suffix distinguishes exact generation-system policy bindings"
)]
pub struct GenerationQualificationRequestBuilderBindingsV1 {
    pub(super) strategy_digest: Digest,
    pub(super) planner_digest: Digest,
    pub(super) prompt_digest: Digest,
    pub(super) output_schema_digest: Digest,
    pub(super) request_policy_digest: Digest,
    pub(super) language_digest: Digest,
    pub(super) mode_digest: Digest,
    pub(super) format_digest: Digest,
}

impl GenerationQualificationRequestBuilderBindingsV1 {
    /// Derives all bindings from the typed profile and reviewed equality bindings.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for an invalid canonical profile preimage.
    pub fn derive(
        profile: &GenerationCaseRequestProfileV1,
        validator_equality_binding: &Digest,
        adapter_equality_binding: &Digest,
    ) -> Result<Self, GenerationQualificationRequestBuildError> {
        digest::builder_bindings(
            profile,
            validator_equality_binding,
            adapter_equality_binding,
        )
    }

    /// Returns the complete grounded-strategy digest.
    #[must_use]
    pub const fn strategy_digest(&self) -> &Digest {
        &self.strategy_digest
    }
    /// Returns the typed whole-document planner digest.
    #[must_use]
    pub const fn planner_digest(&self) -> &Digest {
        &self.planner_digest
    }
    /// Returns the exact prompt-construction contract digest.
    #[must_use]
    pub const fn prompt_digest(&self) -> &Digest {
        &self.prompt_digest
    }
    /// Returns the exact candidate output-schema digest.
    #[must_use]
    pub const fn output_schema_digest(&self) -> &Digest {
        &self.output_schema_digest
    }
    /// Returns the fixed provider-neutral request-policy digest.
    #[must_use]
    pub const fn request_policy_digest(&self) -> &Digest {
        &self.request_policy_digest
    }
    /// Returns the canonical English selector digest.
    #[must_use]
    pub const fn language_digest(&self) -> &Digest {
        &self.language_digest
    }
    /// Returns the explicit typed rewrite-mode digest.
    #[must_use]
    pub const fn mode_digest(&self) -> &Digest {
        &self.mode_digest
    }
    /// Returns the canonical plain UTF-8 selector digest.
    #[must_use]
    pub const fn format_digest(&self) -> &Digest {
        &self.format_digest
    }
}

impl fmt::Debug for GenerationQualificationRequestBuilderBindingsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationRequestBuilderBindingsV1")
            .finish_non_exhaustive()
    }
}
