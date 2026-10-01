use rewrite_model::RuntimePackageManifest;
use rewrite_ollama::OllamaCloudDisableVersionStatus;
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;

use super::{
    RuntimeAdmissionFinalVerification, RuntimeAdmissionRunnerError,
    records::{
        RuntimeAdmissionManagedFinalRecord, RuntimeAdmissionNativeLoadRecord,
        RuntimeAdmissionNativeLoadRecordVerifier,
    },
};
use crate::VerifiedRuntimeAdmissionFoundationBinding;

/// Canonical observation records released after complete managed-runtime cleanup.
///
/// This value preserves the observed cloud review status, including `Unreviewed`.
/// It exposes no passed semantic control, runtime admission, or policy authority.
pub struct RuntimeAdmissionObservedFinalOperation {
    native_load_record: RuntimeAdmissionNativeLoadRecord,
    managed_final_record: RuntimeAdmissionManagedFinalRecord,
    cloud_version_status: OllamaCloudDisableVersionStatus,
}

impl RuntimeAdmissionObservedFinalOperation {
    /// Returns exact canonical native-load observation publication material.
    #[must_use]
    pub const fn native_load_record(&self) -> &RuntimeAdmissionNativeLoadRecord {
        &self.native_load_record
    }

    /// Returns exact canonical managed-final observation publication material.
    #[must_use]
    pub const fn managed_final_record(&self) -> &RuntimeAdmissionManagedFinalRecord {
        &self.managed_final_record
    }

    /// Returns the observed production review status, without granting authority.
    #[must_use]
    pub const fn cloud_version_status(&self) -> OllamaCloudDisableVersionStatus {
        self.cloud_version_status
    }

    pub(super) fn compile(
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        package: &RuntimePackageManifest,
        frozen: &VerifiedFrozenExternalNativeComponentSet,
        final_evidence: &RuntimeAdmissionFinalVerification,
    ) -> Result<Self, RuntimeAdmissionRunnerError> {
        let native_load_record = RuntimeAdmissionNativeLoadRecord::compile(
            foundation,
            package,
            frozen,
            final_evidence.native_load(),
        )?;
        RuntimeAdmissionNativeLoadRecordVerifier::verify(
            native_load_record.canonical_bytes(),
            foundation,
            package,
            frozen,
            final_evidence.native_load(),
        )?;
        let managed_final_record = RuntimeAdmissionManagedFinalRecord::compile(
            foundation,
            package,
            frozen,
            final_evidence,
            native_load_record.record_id(),
        )?;
        let independently_verified = RuntimeAdmissionManagedFinalRecord::verify_observed(
            managed_final_record.canonical_bytes(),
            foundation,
            package,
            frozen,
            final_evidence,
            native_load_record.record_id(),
        )?;
        if independently_verified != managed_final_record {
            return Err(RuntimeAdmissionRunnerError::InvalidRecordEncoding);
        }
        Ok(Self {
            native_load_record,
            managed_final_record,
            cloud_version_status: final_evidence.cloud_disable().version_status(),
        })
    }
}

impl std::fmt::Debug for RuntimeAdmissionObservedFinalOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeAdmissionObservedFinalOperation")
            .field("native_record_id", self.native_load_record.record_id())
            .field("managed_record_id", self.managed_final_record.record_id())
            .field("cloud_version_status", &self.cloud_version_status)
            .finish_non_exhaustive()
    }
}
