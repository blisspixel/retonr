mod bracket_v1;
mod v1;

pub(crate) use bracket_v1::{
    GenerationBracketObservationInput, build_generation_bracket_observation,
};
pub use bracket_v1::{
    MANAGED_OLLAMA_GENERATION_BRACKET_OBSERVATION_SCHEMA_VERSION,
    ManagedOllamaGenerationBracketObservationV1,
};
pub(crate) use v1::{GenerationEvidenceInput, build_generation_evidence};
pub use v1::{
    LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION, LocalOllamaManagedGenerationEvidence,
};
