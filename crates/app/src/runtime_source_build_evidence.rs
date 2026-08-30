//! Durable evidence closure for one controlled runtime source build.

mod contract;
mod plan_binding;
mod publish;
mod publish_plan;
mod verify;

pub use contract::{
    MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_BYTES,
    MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_DESTINATION_ENTRIES,
    MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_STAGING_ROOTS,
    MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_TREE_ENTRIES,
    RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH, RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH,
    RUNTIME_SOURCE_BUILD_PLAN_BINDING_PATH, RUNTIME_SOURCE_BUILD_REPORT_PATH,
    RuntimeSourceBuildEvidenceBundleDestination, RuntimeSourceBuildEvidenceBundleError,
    RuntimeSourceBuildEvidenceBundleLimits, RuntimeSourceBuildEvidenceBundleSource,
};
pub use publish::RuntimeSourceBuildEvidenceBundlePublisher;
pub use verify::{RuntimeSourceBuildEvidenceBundleLease, RuntimeSourceBuildEvidenceBundleVerifier};
