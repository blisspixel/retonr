//! Object-safe, backend-neutral contracts for bounded local inference.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod candidate;
mod conformance;
mod contract;
mod error;
mod local_judge;
mod port;
mod request_identity;
mod schemas;
mod single_candidate;
mod structured;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use candidate::{CandidateOutputError, CandidateOutputPolicy, parse_candidate_output};
pub use conformance::{CONFORMANCE_BACKEND_ID, ConformanceInferenceBackend};
pub use contract::{
    BackendDiscovery, BackendId, BackendIdError, GENERATION_REQUEST_SCHEMA_VERSION,
    GenerationCandidate, GenerationRequest, GenerationResponse, InferenceCapabilities,
    InventoryEntry, MAX_INFERENCE_CANDIDATES, OutputContract, ReasoningPolicy, SamplingParameters,
    UsageObservation,
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
pub use single_candidate::{
    SingleCandidateRequestMappingError, derive_single_candidate_structured_request,
    validate_single_candidate_structured_request,
};
pub use structured::{
    STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, StructuredCompletionFinish,
    StructuredCompletionRequest, StructuredCompletionResponse,
};
