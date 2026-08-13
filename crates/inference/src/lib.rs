//! Object-safe, backend-neutral contracts for bounded local inference.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod contract;
mod error;
mod port;
mod schemas;
mod structured;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use contract::{
    BackendDiscovery, BackendId, BackendIdError, GENERATION_REQUEST_SCHEMA_VERSION,
    GenerationCandidate, GenerationRequest, GenerationResponse, InferenceCapabilities,
    InventoryEntry, OutputContract, ReasoningPolicy, SamplingParameters, UsageObservation,
};
pub use error::{ContractError, InferenceError, InferenceErrorKind};
pub use port::{InferenceBackend, OperationContext, PortFuture};
pub use schemas::candidate_output_contract;
pub use structured::{
    STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, StructuredCompletionFinish,
    StructuredCompletionRequest, StructuredCompletionResponse,
};
