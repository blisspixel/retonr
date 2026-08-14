use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use super::{
    EFFECTIVE_RUNTIME_STATE_SCHEMA_VERSION, MAX_CANONICAL_IDENTITY_BYTES,
    MAX_RUNTIME_IDENTITY_JSON_BYTES, RuntimeBuildId, RuntimeBuildIdentity, append_digest,
    append_text, append_u32, valid_machine_id,
};

/// Compute software backend used by an effective runtime state.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeBackend {
    /// Direct CPU execution without a named compute framework.
    NativeCpu,
    /// NVIDIA CUDA.
    Cuda,
    /// AMD `ROCm`.
    Rocm,
    /// Apple Metal.
    Metal,
    /// Vulkan compute.
    Vulkan,
    /// SYCL compute.
    Sycl,
    /// Intel `OpenVINO`.
    OpenVino,
}

/// High-level device placement used by an effective runtime state.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlacement {
    /// All execution occurs on the CPU.
    CpuOnly,
    /// All qualifying execution occurs on accelerator devices.
    AcceleratorOnly,
    /// Execution is split across CPU and accelerator devices.
    Hybrid,
}

/// Content-derived identifier for one exact effective runtime state.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectiveRuntimeStateId(Digest);

impl EffectiveRuntimeStateId {
    /// Returns the digest that defines this effective state.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Caller-supplied facts required to construct an effective runtime state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveRuntimeStateInput {
    /// Stable adapter-owned provider snapshot contract identifier.
    pub provider_snapshot_contract: String,
    /// Provider snapshot contract version.
    pub provider_snapshot_schema_version: u32,
    /// Digest of the bounded canonical provider snapshot.
    pub provider_snapshot_digest: Digest,
    /// Digest of normalized launch arguments, environment, and lifecycle policy.
    pub launch_policy_digest: Digest,
    /// Digest of actually loaded code components and native dependencies.
    pub loaded_components_digest: Digest,
    /// Digest of effective output-affecting runtime defaults and configuration.
    pub effective_configuration_digest: Digest,
    /// Digest of exact operating-system, framework, and driver evidence.
    pub platform_digest: Digest,
    /// Digest of device class, offload, cache, data type, and parallelism state.
    pub execution_class_digest: Digest,
    /// Digest of endpoint, offline, update, fallback, plugin, and telemetry policy.
    pub isolation_policy_digest: Digest,
    /// Effective runtime context capacity in tokens.
    pub effective_context_tokens: u32,
    /// Effective compute software backend.
    pub compute_backend: ComputeBackend,
    /// Effective CPU and accelerator placement class.
    pub placement: ExecutionPlacement,
}

/// Validated content-addressed effective runtime state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveRuntimeState {
    schema_version: u32,
    runtime_build_id: RuntimeBuildId,
    provider_snapshot_contract: String,
    provider_snapshot_schema_version: u32,
    provider_snapshot_digest: Digest,
    launch_policy_digest: Digest,
    loaded_components_digest: Digest,
    effective_configuration_digest: Digest,
    platform_digest: Digest,
    execution_class_digest: Digest,
    isolation_policy_digest: Digest,
    effective_context_tokens: u32,
    compute_backend: ComputeBackend,
    placement: ExecutionPlacement,
}

impl EffectiveRuntimeState {
    /// Creates and validates a state bound to one exact runtime build.
    ///
    /// # Errors
    ///
    /// Returns [`EffectiveRuntimeStateError`] for unsupported, invalid, or
    /// inconsistent runtime state.
    pub fn new(
        build: &RuntimeBuildIdentity,
        input: EffectiveRuntimeStateInput,
    ) -> Result<Self, EffectiveRuntimeStateError> {
        Self::from_wire(
            EFFECTIVE_RUNTIME_STATE_SCHEMA_VERSION,
            build.runtime_build_id(),
            input,
        )
    }

    fn from_wire(
        schema_version: u32,
        runtime_build_id: RuntimeBuildId,
        input: EffectiveRuntimeStateInput,
    ) -> Result<Self, EffectiveRuntimeStateError> {
        if schema_version != EFFECTIVE_RUNTIME_STATE_SCHEMA_VERSION {
            return Err(EffectiveRuntimeStateError::UnsupportedSchema(
                schema_version,
            ));
        }
        if !valid_machine_id(&input.provider_snapshot_contract)
            || input.provider_snapshot_schema_version == 0
            || input.effective_context_tokens == 0
        {
            return Err(EffectiveRuntimeStateError::InvalidMetadata);
        }
        let placement_is_valid = input.compute_backend != ComputeBackend::NativeCpu
            || input.placement == ExecutionPlacement::CpuOnly;
        if !placement_is_valid {
            return Err(EffectiveRuntimeStateError::InvalidExecutionClass);
        }
        let state = Self {
            schema_version,
            runtime_build_id,
            provider_snapshot_contract: input.provider_snapshot_contract,
            provider_snapshot_schema_version: input.provider_snapshot_schema_version,
            provider_snapshot_digest: input.provider_snapshot_digest,
            launch_policy_digest: input.launch_policy_digest,
            loaded_components_digest: input.loaded_components_digest,
            effective_configuration_digest: input.effective_configuration_digest,
            platform_digest: input.platform_digest,
            execution_class_digest: input.execution_class_digest,
            isolation_policy_digest: input.isolation_policy_digest,
            effective_context_tokens: input.effective_context_tokens,
            compute_backend: input.compute_backend,
            placement: input.placement,
        };
        if state.canonical_bytes().len() > MAX_CANONICAL_IDENTITY_BYTES {
            return Err(EffectiveRuntimeStateError::CanonicalEncodingTooLarge);
        }
        Ok(state)
    }

    /// Returns the content-derived identity of this structurally validated record.
    #[must_use]
    pub fn effective_runtime_state_id(&self) -> EffectiveRuntimeStateId {
        EffectiveRuntimeStateId(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the exact runtime build bound into this state.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }

    /// Returns the effective context capacity in tokens.
    #[must_use]
    pub const fn effective_context_tokens(&self) -> u32 {
        self.effective_context_tokens
    }

    /// Returns the effective compute software backend.
    #[must_use]
    pub const fn compute_backend(&self) -> ComputeBackend {
        self.compute_backend
    }

    /// Returns the effective CPU and accelerator placement class.
    #[must_use]
    pub const fn placement(&self) -> ExecutionPlacement {
        self.placement
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(b"retonr:effective-runtime-state:v1\0");
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.runtime_build_id.digest());
        append_text(&mut output, &self.provider_snapshot_contract);
        append_u32(&mut output, self.provider_snapshot_schema_version);
        for digest in [
            &self.provider_snapshot_digest,
            &self.launch_policy_digest,
            &self.loaded_components_digest,
            &self.effective_configuration_digest,
            &self.platform_digest,
            &self.execution_class_digest,
            &self.isolation_policy_digest,
        ] {
            append_digest(&mut output, digest);
        }
        output.push(compute_backend_byte(self.compute_backend));
        output.push(execution_placement_byte(self.placement));
        append_u32(&mut output, self.effective_context_tokens);
        output
    }
}

impl EffectiveRuntimeState {
    /// Parses a byte-bounded JSON record and revalidates every state field.
    ///
    /// # Errors
    ///
    /// Returns [`EffectiveRuntimeStateError`] before decoding when the input exceeds
    /// the fixed ceiling, or when JSON or state validation fails.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, EffectiveRuntimeStateError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            runtime_build_id: RuntimeBuildId,
            provider_snapshot_contract: String,
            provider_snapshot_schema_version: u32,
            provider_snapshot_digest: Digest,
            launch_policy_digest: Digest,
            loaded_components_digest: Digest,
            effective_configuration_digest: Digest,
            platform_digest: Digest,
            execution_class_digest: Digest,
            isolation_policy_digest: Digest,
            effective_context_tokens: u32,
            compute_backend: ComputeBackend,
            placement: ExecutionPlacement,
        }

        if bytes.len() > MAX_RUNTIME_IDENTITY_JSON_BYTES {
            return Err(EffectiveRuntimeStateError::EncodedIdentityTooLarge);
        }

        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| EffectiveRuntimeStateError::InvalidEncoding)?;
        Self::from_wire(
            wire.schema_version,
            wire.runtime_build_id,
            EffectiveRuntimeStateInput {
                provider_snapshot_contract: wire.provider_snapshot_contract,
                provider_snapshot_schema_version: wire.provider_snapshot_schema_version,
                provider_snapshot_digest: wire.provider_snapshot_digest,
                launch_policy_digest: wire.launch_policy_digest,
                loaded_components_digest: wire.loaded_components_digest,
                effective_configuration_digest: wire.effective_configuration_digest,
                platform_digest: wire.platform_digest,
                execution_class_digest: wire.execution_class_digest,
                isolation_policy_digest: wire.isolation_policy_digest,
                effective_context_tokens: wire.effective_context_tokens,
                compute_backend: wire.compute_backend,
                placement: wire.placement,
            },
        )
    }
}

pub(super) const fn compute_backend_byte(value: ComputeBackend) -> u8 {
    match value {
        ComputeBackend::NativeCpu => 0,
        ComputeBackend::Cuda => 1,
        ComputeBackend::Rocm => 2,
        ComputeBackend::Metal => 3,
        ComputeBackend::Vulkan => 4,
        ComputeBackend::Sycl => 5,
        ComputeBackend::OpenVino => 6,
    }
}

pub(super) const fn execution_placement_byte(value: ExecutionPlacement) -> u8 {
    match value {
        ExecutionPlacement::CpuOnly => 0,
        ExecutionPlacement::AcceleratorOnly => 1,
        ExecutionPlacement::Hybrid => 2,
    }
}

/// Effective-runtime-state validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum EffectiveRuntimeStateError {
    /// Encoded input exceeds the fixed pre-decode byte ceiling.
    #[error("encoded effective runtime state exceeds its limit")]
    EncodedIdentityTooLarge,
    /// Encoded JSON is malformed or contains an unknown field.
    #[error("effective runtime state encoding is invalid")]
    InvalidEncoding,
    /// The effective-state schema version is unsupported.
    #[error("unsupported effective runtime state schema {0}")]
    UnsupportedSchema(u32),
    /// Provider snapshot identity or context metadata is invalid.
    #[error("effective runtime state metadata is invalid")]
    InvalidMetadata,
    /// Compute backend and placement are inconsistent.
    #[error("effective runtime execution class is inconsistent")]
    InvalidExecutionClass,
    /// Canonical identity bytes exceed the fixed contract ceiling.
    #[error("effective runtime state canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
}
