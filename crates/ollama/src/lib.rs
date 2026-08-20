//! Bounded loopback-only adapter for the Ollama native API.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod backend;
#[cfg(test)]
mod backend_tests;
mod contract;
mod endpoint;
#[cfg(test)]
mod preflight_tests;
#[cfg(test)]
mod remote_tests;
mod response;
#[cfg(test)]
mod structured_tests;
mod wire;

pub use backend::OllamaBackend;
pub use contract::{
    OllamaInventoryEntry, OllamaLimits, OllamaModelBinding, OllamaModelDetails, OllamaPreflight,
    OllamaPreflightBinding, OllamaPreflightTarget, OllamaRunningModel,
};
pub use endpoint::{OllamaEndpoint, OllamaEndpointError};
