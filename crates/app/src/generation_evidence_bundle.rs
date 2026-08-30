//! App-owned canonical artifacts and durable storage for generation evidence.

mod contract;
mod plan;
mod publish;
mod receipt_compiler;
mod response_artifact;
mod verify;

pub use contract::{
    CandidateGenerationEvidenceBundleDestination, CandidateGenerationEvidenceBundleError,
    CandidateGenerationEvidenceBundleLimits, CandidateGenerationEvidenceBundleSource,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_STAGING_ROOTS,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TOTAL_BYTES,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TREE_DEPTH,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TREE_ENTRIES,
};
pub use plan::{
    CandidateGenerationEvidenceBundleAuxiliaryArtifact,
    CandidateGenerationEvidenceBundlePublicationPlan,
    CandidateGenerationEvidenceBundlePublicationPlanInput,
};
pub use publish::{
    CandidateGenerationEvidenceBundlePublication, CandidateGenerationEvidenceBundlePublisher,
};
pub use receipt_compiler::{
    CandidateGenerationReceiptCompilation, CandidateGenerationReceiptCompilationError,
    CandidateGenerationReceiptCompilationInput, CandidateGenerationReceiptCompiler,
};

pub use response_artifact::{
    MAX_RETAINED_STRUCTURED_RESPONSE_ARTIFACT_JSON_BYTES,
    RETAINED_STRUCTURED_RESPONSE_ARTIFACT_SCHEMA_VERSION, RetainedStructuredResponseArtifactError,
    RetainedStructuredResponseArtifactV1,
};
pub(crate) use verify::acquire_from_parent;
pub use verify::{
    CandidateGenerationEvidenceBundleReadbackLease, CandidateGenerationEvidenceBundleVerifier,
};

#[cfg(test)]
#[path = "generation_evidence_bundle/tests.rs"]
mod tests;
