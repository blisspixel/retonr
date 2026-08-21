//! Bounded offline reconstruction of one narrow Ollama model package shape.
//!
//! This crate reads caller-supplied bytes only. It does not discover installed
//! models, open paths, access a registry, or grant runtime authority.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod gguf;
mod json;
mod manifest;
mod reconstruct;

pub use error::{BlobOpenError, ReconstructionError, ReconstructionResult};
pub use gguf::{GgufComponentDigests, GgufLimits, GgufObservation, inspect_gguf_v3};
pub use manifest::{
    BlobDescriptor, CONFIG_MEDIA_TYPE, LICENSE_MEDIA_TYPE, MANIFEST_MEDIA_TYPE, MODEL_MEDIA_TYPE,
    OllamaManifestPlan, PARAMS_MEDIA_TYPE, ReconstructionLimits, TEMPLATE_MEDIA_TYPE,
    parse_manifest_v2,
};
pub use reconstruct::{
    ReconstructedModelPackage, RootfsDescriptorComparison, reconstruct_model_package,
    reconstruct_model_package_with_limits,
};
