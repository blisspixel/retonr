//! Bounded loopback-only adapter for the Ollama native API.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod backend;
#[cfg(test)]
mod backend_tests;
mod contract;
mod endpoint;
mod response;
mod wire;

pub use backend::OllamaBackend;
pub use contract::{
    OllamaLimits, OllamaModelBinding, OllamaModelDetails, candidate_output_contract,
};
pub use endpoint::{OllamaEndpoint, OllamaEndpointError};
