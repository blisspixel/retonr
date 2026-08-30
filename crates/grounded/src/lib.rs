//! Bounded grounded candidate generation without validation authority.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod policy;
mod render;
mod strategy;

pub use policy::{
    GROUNDED_POLICY_SCHEMA_VERSION, GroundedPolicy, GroundedRequest, GroundedSentinel,
    GroundedSentinelKind,
};
pub use render::{
    GROUNDED_PROMPT_CONTENT_BOUNDARY_V1, GroundedPromptRenderError, GroundedPromptRenderInputV1,
    render_grounded_prompt_v1,
};
pub use strategy::{
    GROUNDED_TRACE_SCHEMA_VERSION, GroundedError, GroundedGeneration, GroundedStrategy,
    GroundedTrace,
};
