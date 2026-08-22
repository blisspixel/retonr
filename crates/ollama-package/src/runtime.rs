//! Bounded offline reconstruction of one reviewed Ollama runtime package.
//!
//! The first admitted contract is Linux `x86_64` GNU libc, family `ollama`, and an
//! untransformed reviewed layout. Reconstruction hashes declared members and
//! builds inert schema-6 manifests. It does not execute members, load code, or
//! grant runtime authority.

mod error;
mod layout;
mod reconstruct;

pub use error::{MemberOpenError, RuntimeReconstructionError, RuntimeReconstructionResult};
pub use layout::{
    ADMITTED_RUNTIME_FAMILY, RUNTIME_LAYOUT_SCHEMA_VERSION, RuntimeLayoutLimits,
    RuntimePackageLayout, RuntimePackageLayoutMember,
};
pub use reconstruct::{
    ReconstructedRuntimePackage, reconstruct_runtime_package,
    reconstruct_runtime_package_with_limits,
};
