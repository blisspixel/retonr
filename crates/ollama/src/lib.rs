//! Bounded loopback-only adapter for the Ollama native API.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod backend;
#[cfg(test)]
mod backend_tests;
mod cloud_disable;
mod contract;
mod endpoint;
mod generation_admission;
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
pub use generation_admission::{
    OllamaManagedGenerationAdmissionPolicy, OllamaManagedGenerationAdmissionStatus,
};
pub use single_connection::{
    OLLAMA_RESIDENT_COMPLETION_KEEP_ALIVE, OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION,
    OLLAMA_RESIDENT_COMPLETION_SOURCE_REVISION, OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES,
    OLLAMA_RUNTIME_PROBE_MAX_BODY_BYTES, OllamaConnectionAddresses,
    OllamaGenerateResourceObservation, OllamaGenerateResourceObservationError,
    OllamaObservedPreflightError, OllamaObservedRuntimeProbeError, OllamaObservedSessionError,
    OllamaResidentResourceObservedCompletion, OllamaResidentSessionExecutionReceipt,
    OllamaResponseObservation, OllamaResponseObservationPhase, OllamaRetainedSessionSubjectToken,
    OllamaRetainedStreamSession, OllamaRetainedStreamSessionConfig, OllamaRuntimeProbeEvidence,
    OllamaSessionExecutionReceipt, OllamaSingleConnectionPreflight,
    OllamaSingleConnectionRuntimeProbe, derive_ollama_retained_session_response_id,
};
