//! Object-safe, backend-neutral contracts for bounded local inference.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod conformance;
mod contract;
mod error;
mod local_judge;
mod port;
mod schemas;
mod structured;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use conformance::{CONFORMANCE_BACKEND_ID, ConformanceInferenceBackend};
pub use contract::{
    BackendDiscovery, BackendId, BackendIdError, GENERATION_REQUEST_SCHEMA_VERSION,
    GenerationCandidate, GenerationRequest, GenerationResponse, InferenceCapabilities,
    InventoryEntry, OutputContract, ReasoningPolicy, SamplingParameters, UsageObservation,
};
pub use error::{ContractError, InferenceError, InferenceErrorKind};
pub use local_judge::{
    LOCAL_JUDGE_ATTEMPT_OUTPUT_SCHEMA_VERSION, LocalJudgeAttemptOutput,
    LocalJudgeAttemptOutputError, LocalJudgeByteSpan, LocalJudgeChoice,
    MAX_LOCAL_JUDGE_ATTEMPT_OUTPUT_BYTES, MAX_LOCAL_JUDGE_BYTE_SPANS, MAX_LOCAL_JUDGE_LABEL_BYTES,
    MAX_LOCAL_JUDGE_RUBRIC_CLAUSES, local_judge_attempt_output_contract,
    parse_local_judge_attempt_output,
};
pub use port::{InferenceBackend, OperationContext, PortFuture};
pub use schemas::{candidate_output_contract, claim_output_contract};
pub use structured::{
    STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, StructuredCompletionFinish,
    StructuredCompletionRequest, StructuredCompletionResponse,
};
