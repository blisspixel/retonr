//! Bounded grounded candidate generation without validation authority.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod policy;
mod strategy;

pub use policy::{
    GROUNDED_POLICY_SCHEMA_VERSION, GroundedPolicy, GroundedRequest, GroundedSentinel,
    GroundedSentinelKind,
};
pub use strategy::{
    GROUNDED_TRACE_SCHEMA_VERSION, GroundedError, GroundedGeneration, GroundedStrategy,
    GroundedTrace,
};
