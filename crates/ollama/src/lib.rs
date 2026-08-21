//! Bounded loopback-only adapter for the Ollama native API.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod backend;
#[cfg(test)]
mod backend_tests;
mod cloud_disable;
mod contract;
mod endpoint;
#[cfg(test)]
mod preflight_tests;
#[cfg(test)]
mod remote_tests;
mod response;
mod single_connection;
#[cfg(test)]
mod structured_tests;
mod wire;

pub use backend::OllamaBackend;
pub use cloud_disable::{
    OLLAMA_CLOUD_DISABLE_FEATURE_FLOOR, OllamaCloudDisableDeclarationSource,
    OllamaCloudDisableEvidence, OllamaCloudDisableEvidenceError, OllamaCloudDisableFeaturePolicy,
    OllamaCloudDisableMarkerSource, OllamaCloudDisableStartupMarker,
    OllamaCloudDisableVersionStatus, OllamaManagedCloudDisableEnvironment,
    OllamaNetworkIsolationStatus, OllamaProviderDeclarationStatus,
    OllamaProviderQualificationStatus, OllamaVersion, OllamaVersionParseError,
};
pub use contract::{
    OllamaInventoryEntry, OllamaLimits, OllamaModelBinding, OllamaModelDetails, OllamaPreflight,
    OllamaPreflightBinding, OllamaPreflightTarget, OllamaRunningModel,
};
pub use endpoint::{OllamaEndpoint, OllamaEndpointError};
pub use single_connection::{
    OLLAMA_RESIDENT_COMPLETION_KEEP_ALIVE, OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION,
    OLLAMA_RESIDENT_COMPLETION_SOURCE_REVISION, OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES,
    OllamaConnectionAddresses, OllamaObservedPreflightError, OllamaObservedSessionError,
    OllamaResidentSessionExecutionReceipt, OllamaResponseObservation,
    OllamaResponseObservationPhase, OllamaRetainedStreamSession, OllamaRetainedStreamSessionConfig,
    OllamaSessionExecutionReceipt, OllamaSingleConnectionPreflight,
};
