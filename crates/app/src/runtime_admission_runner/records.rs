use rewrite_model::{NativeLoadObservation, NativeLoadObservationId, RuntimePackageManifest};
use rewrite_ollama::OllamaCloudDisableVersionStatus;
use rewrite_runtime_attestor::{
    FrozenExternalNativeComponentSetId, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_types::Digest;
use serde::Serialize;

use super::{RuntimeAdmissionFinalVerification, RuntimeAdmissionRunnerError};
use crate::{RuntimeAdmissionEvidenceFoundationId, VerifiedRuntimeAdmissionFoundationBinding};

/// Maximum accepted canonical bytes for one native-load execution record.
pub const MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES: usize = 2 * 1_048_576;
/// Maximum accepted canonical bytes for one managed-final execution record.
pub const MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES: usize = 262_144;

const NATIVE_RECORD_SCHEMA_VERSION: u32 = 1;
const MANAGED_RECORD_SCHEMA_VERSION: u32 = 1;
const NATIVE_RECORD_ID_DOMAIN: &[u8] = b"retonr:runtime-admission:native-load-record:v1";
const MANAGED_RECORD_ID_DOMAIN: &[u8] = b"retonr:runtime-admission:managed-final-record:v1";
const NATIVE_CONTROL_DOMAIN: &[u8] = b"retonr:runtime-admission:native-closure-control:v1";
const STARTUP_CONTROL_DOMAIN: &[u8] = b"retonr:runtime-admission:managed-startup-control:v1";
const CLOUD_CONTROL_DOMAIN: &[u8] = b"retonr:runtime-admission:cloud-disable-control:v1";

mod codec;
use codec::{
    ManagedFinalRecordWire, NativeLoadRecordWire, derive_managed_wire, derive_native_wire, encode,
    parse_canonical, parse_native_observation, record_digest, validate_native_observation,
    validate_subjects,
};

/// Content-derived identity of one canonical native-load execution record.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeAdmissionNativeLoadRecordId(Digest);

impl RuntimeAdmissionNativeLoadRecordId {
    /// Returns the digest defining this record identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Content-derived identity of one canonical managed-final execution record.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeAdmissionManagedFinalRecordId(Digest);

impl RuntimeAdmissionManagedFinalRecordId {
    /// Returns the digest defining this record identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Canonical native-load publication material without admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionNativeLoadRecord {
    canonical_bytes: Vec<u8>,
    record_id: RuntimeAdmissionNativeLoadRecordId,
}

impl RuntimeAdmissionNativeLoadRecord {
    /// Returns exact canonical publication bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the content-derived publication identity.
    #[must_use]
    pub const fn record_id(&self) -> &RuntimeAdmissionNativeLoadRecordId {
        &self.record_id
    }

    pub(super) fn compile(
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        package: &RuntimePackageManifest,
        frozen: &VerifiedFrozenExternalNativeComponentSet,
        observation: &NativeLoadObservation,
    ) -> Result<Self, RuntimeAdmissionRunnerError> {
        validate_subjects(foundation, package, frozen)?;
        validate_native_observation(package, frozen, observation)?;
        let wire = derive_native_wire(foundation.foundation_id(), package, frozen, observation)?;
        let canonical_bytes = encode(&wire, MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES)?;
        let record_id = RuntimeAdmissionNativeLoadRecordId(record_digest(
            NATIVE_RECORD_ID_DOMAIN,
            &canonical_bytes,
        ));
        Ok(Self {
            canonical_bytes,
            record_id,
        })
    }
}

/// Canonical managed-final publication material without policy authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionManagedFinalRecord {
    canonical_bytes: Vec<u8>,
    record_id: RuntimeAdmissionManagedFinalRecordId,
}

impl RuntimeAdmissionManagedFinalRecord {
    /// Returns exact canonical publication bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the content-derived publication identity.
    #[must_use]
    pub const fn record_id(&self) -> &RuntimeAdmissionManagedFinalRecordId {
        &self.record_id
    }

    pub(super) fn compile(
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        package: &RuntimePackageManifest,
        frozen: &VerifiedFrozenExternalNativeComponentSet,
        final_evidence: &RuntimeAdmissionFinalVerification,
        native_record_id: &RuntimeAdmissionNativeLoadRecordId,
    ) -> Result<Self, RuntimeAdmissionRunnerError> {
        validate_subjects(foundation, package, frozen)?;
        let wire = derive_managed_wire(
            foundation.foundation_id(),
            package,
            frozen,
            final_evidence,
            native_record_id,
        )?;
        let canonical_bytes = encode(&wire, MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES)?;
        let record_id = RuntimeAdmissionManagedFinalRecordId(record_digest(
            MANAGED_RECORD_ID_DOMAIN,
            &canonical_bytes,
        ));
        Ok(Self {
            canonical_bytes,
            record_id,
        })
    }
}

/// Independently verified passed native-closure result.
///
/// This value is inert and cannot mutate runtime policy or grant generation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPassedRuntimeAdmissionNativeClosureControl {
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    record_id: RuntimeAdmissionNativeLoadRecordId,
    control_digest: Digest,
    native_load_observation_id: NativeLoadObservationId,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
}

impl VerifiedPassedRuntimeAdmissionNativeClosureControl {
    /// Returns the exact verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }

    /// Returns the verified native-load record identity.
    #[must_use]
    pub const fn record_id(&self) -> &RuntimeAdmissionNativeLoadRecordId {
        &self.record_id
    }

    /// Returns the domain-separated control digest.
    #[must_use]
    pub const fn control_digest(&self) -> &Digest {
        &self.control_digest
    }

    /// Returns the verified native observation identity.
    #[must_use]
    pub const fn native_load_observation_id(&self) -> &NativeLoadObservationId {
        &self.native_load_observation_id
    }

    /// Returns the independently verified frozen-set identity.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalNativeComponentSetId {
        &self.frozen_external_component_set_id
    }
}

/// Independently verified passed managed-startup result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPassedRuntimeAdmissionManagedStartupControl {
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    record_id: RuntimeAdmissionManagedFinalRecordId,
    native_record_id: RuntimeAdmissionNativeLoadRecordId,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    startup_launch_spec_digest: Digest,
    control_digest: Digest,
}

impl VerifiedPassedRuntimeAdmissionManagedStartupControl {
    /// Returns the exact verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }

    /// Returns the verified managed-final record identity.
    #[must_use]
    pub const fn record_id(&self) -> &RuntimeAdmissionManagedFinalRecordId {
        &self.record_id
    }

    /// Returns the exact native-load record joined by the managed record.
    #[must_use]
    pub const fn native_record_id(&self) -> &RuntimeAdmissionNativeLoadRecordId {
        &self.native_record_id
    }

    /// Returns the exact frozen native-component set joined by the managed record.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalNativeComponentSetId {
        &self.frozen_external_component_set_id
    }

    /// Returns the exact plain launch specification digest independently decoded
    /// from the passed managed-startup record.
    #[must_use]
    pub const fn startup_launch_spec_digest(&self) -> &Digest {
        &self.startup_launch_spec_digest
    }

    /// Returns the domain-separated control digest.
    #[must_use]
    pub const fn control_digest(&self) -> &Digest {
        &self.control_digest
    }
}

/// Independently verified production-reviewed cloud-disable result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPassedRuntimeAdmissionCloudDisableControl {
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    record_id: RuntimeAdmissionManagedFinalRecordId,
    native_record_id: RuntimeAdmissionNativeLoadRecordId,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    version_status: OllamaCloudDisableVersionStatus,
    control_digest: Digest,
}

impl VerifiedPassedRuntimeAdmissionCloudDisableControl {
    /// Returns the exact verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }

    /// Returns the verified managed-final record identity.
    #[must_use]
    pub const fn record_id(&self) -> &RuntimeAdmissionManagedFinalRecordId {
        &self.record_id
    }

    /// Returns the exact native-load record joined by the managed record.
    #[must_use]
    pub const fn native_record_id(&self) -> &RuntimeAdmissionNativeLoadRecordId {
        &self.native_record_id
    }

    /// Returns the exact frozen native-component set joined by the managed record.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalNativeComponentSetId {
        &self.frozen_external_component_set_id
    }

    /// Returns the production cloud-disable review status verified by this control.
    #[must_use]
    pub const fn version_status(&self) -> OllamaCloudDisableVersionStatus {
        self.version_status
    }

    /// Returns the domain-separated control digest.
    #[must_use]
    pub const fn control_digest(&self) -> &Digest {
        &self.control_digest
    }
}

#[cfg(test)]
pub(crate) fn test_execution_control_fixtures(
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    cloud_status: OllamaCloudDisableVersionStatus,
    native_record_matches: bool,
    frozen_set_matches: bool,
) -> (
    VerifiedPassedRuntimeAdmissionNativeClosureControl,
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionCloudDisableControl,
) {
    test_execution_control_fixtures_with_launch(
        foundation_id,
        cloud_status,
        native_record_matches,
        frozen_set_matches,
        Digest::sha256(b"launch"),
    )
}

#[cfg(test)]
pub(crate) fn test_execution_control_fixtures_with_launch(
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    cloud_status: OllamaCloudDisableVersionStatus,
    native_record_matches: bool,
    frozen_set_matches: bool,
    startup_launch_spec_digest: Digest,
) -> (
    VerifiedPassedRuntimeAdmissionNativeClosureControl,
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionCloudDisableControl,
) {
    let native_record_id = RuntimeAdmissionNativeLoadRecordId(Digest::sha256(b"native record"));
    let managed_record_id = RuntimeAdmissionManagedFinalRecordId(Digest::sha256(b"managed record"));
    let native_load_observation_id =
        serde_json::from_value(serde_json::json!(Digest::sha256(b"native observation")))
            .expect("test native observation identity");
    let frozen_external_component_set_id: FrozenExternalNativeComponentSetId =
        serde_json::from_value(serde_json::json!(Digest::sha256(
            b"frozen external components"
        )))
        .expect("test frozen set identity");
    let managed_native_record_id = if native_record_matches {
        native_record_id.clone()
    } else {
        RuntimeAdmissionNativeLoadRecordId(Digest::sha256(b"other native record"))
    };
    let managed_frozen_external_component_set_id = if frozen_set_matches {
        frozen_external_component_set_id.clone()
    } else {
        serde_json::from_value(serde_json::json!(Digest::sha256(b"other frozen set")))
            .expect("other test frozen set identity")
    };
    let native = VerifiedPassedRuntimeAdmissionNativeClosureControl {
        foundation_id: foundation_id.clone(),
        record_id: native_record_id.clone(),
        control_digest: Digest::sha256(b"native control"),
        native_load_observation_id,
        frozen_external_component_set_id: frozen_external_component_set_id.clone(),
    };
    let bindings = ManagedControlBindings {
        cloud_control_digest: Digest::sha256(b"cloud control"),
        cloud_status,
        foundation_id,
        frozen_external_component_set_id: managed_frozen_external_component_set_id,
        native_record_id: managed_native_record_id,
        record_id: managed_record_id,
        startup_launch_spec_digest,
        startup_control_digest: Digest::sha256(b"startup control"),
    };
    let (startup, cloud) = if cloud_status == OllamaCloudDisableVersionStatus::Reviewed {
        derive_passed_managed_controls(bindings).expect("reviewed test controls")
    } else {
        derive_managed_controls(bindings)
    };
    (native, startup, cloud)
}

/// Independent verifier for one native-load execution record.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionNativeLoadRecordVerifier;

impl RuntimeAdmissionNativeLoadRecordVerifier {
    /// Reparses canonical bytes and revalidates package, frozen-set, and live bindings.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionRunnerError`] for malformed, noncanonical,
    /// excessive, cross-foundation, cross-package, stale, or unfrozen evidence.
    pub fn verify(
        bytes: &[u8],
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        package: &RuntimePackageManifest,
        frozen: &VerifiedFrozenExternalNativeComponentSet,
        expected_observation: &NativeLoadObservation,
    ) -> Result<VerifiedPassedRuntimeAdmissionNativeClosureControl, RuntimeAdmissionRunnerError>
    {
        validate_subjects(foundation, package, frozen)?;
        let wire: NativeLoadRecordWire =
            parse_canonical(bytes, MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES)?;
        let observation = parse_native_observation(&wire.native_load, package)?;
        validate_native_observation(package, frozen, &observation)?;
        let expected = derive_native_wire(
            foundation.foundation_id(),
            package,
            frozen,
            expected_observation,
        )?;
        if wire != expected || observation != *expected_observation {
            return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
        }
        let record_id =
            RuntimeAdmissionNativeLoadRecordId(record_digest(NATIVE_RECORD_ID_DOMAIN, bytes));
        Ok(VerifiedPassedRuntimeAdmissionNativeClosureControl {
            foundation_id: foundation.foundation_id().clone(),
            control_digest: record_digest(NATIVE_CONTROL_DOMAIN, bytes),
            record_id,
            native_load_observation_id: observation.native_load_observation_id(),
            frozen_external_component_set_id: frozen.frozen_set_id().clone(),
        })
    }
}

/// Independent verifier for one managed-final execution record.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionManagedFinalRecordVerifier;

impl RuntimeAdmissionManagedFinalRecordVerifier {
    /// Reparses canonical bytes and re-derives every field from transient live evidence.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionRunnerError`] for malformed, noncanonical,
    /// excessive, stale, cross-foundation, cross-package, cross-record, or
    /// non-production-reviewed input.
    pub fn verify(
        bytes: &[u8],
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        package: &RuntimePackageManifest,
        frozen: &VerifiedFrozenExternalNativeComponentSet,
        final_evidence: &RuntimeAdmissionFinalVerification,
        native: &VerifiedPassedRuntimeAdmissionNativeClosureControl,
    ) -> Result<
        (
            VerifiedPassedRuntimeAdmissionManagedStartupControl,
            VerifiedPassedRuntimeAdmissionCloudDisableControl,
        ),
        RuntimeAdmissionRunnerError,
    > {
        validate_subjects(foundation, package, frozen)?;
        if native.foundation_id() != foundation.foundation_id()
            || native.frozen_external_component_set_id() != frozen.frozen_set_id()
        {
            return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
        }
        let wire: ManagedFinalRecordWire =
            parse_canonical(bytes, MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES)?;
        let expected = derive_managed_wire(
            foundation.foundation_id(),
            package,
            frozen,
            final_evidence,
            native.record_id(),
        )?;
        if wire != expected
            || &wire.native_load_observation_id != native.native_load_observation_id()
        {
            return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
        }
        let record_id =
            RuntimeAdmissionManagedFinalRecordId(record_digest(MANAGED_RECORD_ID_DOMAIN, bytes));
        derive_passed_managed_controls(ManagedControlBindings {
            cloud_control_digest: record_digest(CLOUD_CONTROL_DOMAIN, bytes),
            cloud_status: final_evidence.cloud_disable().version_status(),
            foundation_id: foundation.foundation_id().clone(),
            frozen_external_component_set_id: frozen.frozen_set_id().clone(),
            native_record_id: native.record_id().clone(),
            record_id,
            startup_launch_spec_digest: wire.managed_startup.launch_spec_digest,
            startup_control_digest: record_digest(STARTUP_CONTROL_DOMAIN, bytes),
        })
    }
}

struct ManagedControlBindings {
    cloud_control_digest: Digest,
    cloud_status: OllamaCloudDisableVersionStatus,
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    native_record_id: RuntimeAdmissionNativeLoadRecordId,
    record_id: RuntimeAdmissionManagedFinalRecordId,
    startup_launch_spec_digest: Digest,
    startup_control_digest: Digest,
}

fn derive_passed_managed_controls(
    bindings: ManagedControlBindings,
) -> Result<
    (
        VerifiedPassedRuntimeAdmissionManagedStartupControl,
        VerifiedPassedRuntimeAdmissionCloudDisableControl,
    ),
    RuntimeAdmissionRunnerError,
> {
    require_reviewed_cloud_disable(bindings.cloud_status)?;
    Ok(derive_managed_controls(bindings))
}

fn derive_managed_controls(
    bindings: ManagedControlBindings,
) -> (
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionCloudDisableControl,
) {
    (
        VerifiedPassedRuntimeAdmissionManagedStartupControl {
            foundation_id: bindings.foundation_id.clone(),
            record_id: bindings.record_id.clone(),
            native_record_id: bindings.native_record_id.clone(),
            frozen_external_component_set_id: bindings.frozen_external_component_set_id.clone(),
            startup_launch_spec_digest: bindings.startup_launch_spec_digest,
            control_digest: bindings.startup_control_digest,
        },
        VerifiedPassedRuntimeAdmissionCloudDisableControl {
            foundation_id: bindings.foundation_id,
            record_id: bindings.record_id,
            native_record_id: bindings.native_record_id,
            frozen_external_component_set_id: bindings.frozen_external_component_set_id,
            version_status: bindings.cloud_status,
            control_digest: bindings.cloud_control_digest,
        },
    )
}

fn require_reviewed_cloud_disable(
    status: OllamaCloudDisableVersionStatus,
) -> Result<(), RuntimeAdmissionRunnerError> {
    match status {
        OllamaCloudDisableVersionStatus::Reviewed => Ok(()),
        OllamaCloudDisableVersionStatus::Unreviewed => {
            Err(RuntimeAdmissionRunnerError::CloudDisableRuntimeUnreviewed)
        }
        OllamaCloudDisableVersionStatus::FeatureUnavailable => {
            Err(RuntimeAdmissionRunnerError::CloudDisableFeatureUnavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_domains_are_distinct_for_identical_record_bytes() {
        let bytes = b"canonical execution record";
        assert_ne!(
            record_digest(NATIVE_CONTROL_DOMAIN, bytes),
            record_digest(STARTUP_CONTROL_DOMAIN, bytes)
        );
        assert_ne!(
            record_digest(STARTUP_CONTROL_DOMAIN, bytes),
            record_digest(CLOUD_CONTROL_DOMAIN, bytes)
        );
    }

    #[test]
    fn only_reviewed_cloud_status_can_produce_a_passed_control() {
        assert!(require_reviewed_cloud_disable(OllamaCloudDisableVersionStatus::Reviewed).is_ok());
        assert!(matches!(
            require_reviewed_cloud_disable(OllamaCloudDisableVersionStatus::Unreviewed),
            Err(RuntimeAdmissionRunnerError::CloudDisableRuntimeUnreviewed)
        ));
        assert!(matches!(
            require_reviewed_cloud_disable(OllamaCloudDisableVersionStatus::FeatureUnavailable),
            Err(RuntimeAdmissionRunnerError::CloudDisableFeatureUnavailable)
        ));
    }
}
