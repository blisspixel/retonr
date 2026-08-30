//! Opaque runtime-admission and managed-generation path capabilities.
//!
//! Neither capability mutates production policy or claims model qualification.

use std::str::FromStr;

use rewrite_model::{
    ArtifactId, ArtifactSetId, ManagedGenerationPathId, PackageSourceId, RuntimeAdmissionJoinId,
    RuntimePackageManifest, RuntimePackageManifestId,
};
use rewrite_ollama::{
    OllamaCloudDisableVersionStatus, OllamaManagedGenerationAdmissionPolicy,
    OllamaManagedGenerationAdmissionStatus, OllamaVersion,
};
use rewrite_ollama_package::{
    RuntimePackageReviewCheck, RuntimePackageReviewCheckStatus, RuntimePackageReviewDispositionV2,
    VerifiedRuntimePackageReviewV2,
};
use rewrite_runtime_attestor::FrozenExternalNativeComponentSetId;
use rewrite_types::Digest;
use serde::Serialize;
use thiserror::Error;

use crate::{
    RuntimeAdmissionEvidenceFoundationId, VerifiedPassedRuntimeAdmissionCloudDisableControl,
    VerifiedPassedRuntimeAdmissionLicenseControl,
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionNativeClosureControl,
    VerifiedPassedRuntimeAdmissionSourceLineageControl,
    VerifiedPassedRuntimeAdmissionTransformationControl, VerifiedRuntimeAdmissionFoundationBinding,
};

mod validation;
use validation::{
    AdmittedRuntimeBinding, GenerationPathReviewStatus, ManagedGenerationPathReviewWire,
    domain_digest, encode, exact_generation_worker, parse_canonical, validate_control_bindings,
    validate_review_relationships,
};

/// Current canonical managed-generation path review schema.
pub const MANAGED_GENERATION_PATH_REVIEW_SCHEMA_VERSION: u32 = 1;
/// Fixed managed-generation path review procedure identity.
pub const MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_ID: &str =
    "retonr:managed-generation-path-review";
/// Fixed managed-generation path review procedure version.
pub const MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_VERSION: u32 = 1;
/// Maximum accepted canonical managed-generation path review bytes.
pub const MAX_MANAGED_GENERATION_PATH_REVIEW_BYTES: usize = 65_536;

const ADMITTED_RUNTIME_ID_DOMAIN: &[u8] = b"retonr:verified-admitted-runtime:v1\0";
const GENERATION_PATH_ID_DOMAIN: &[u8] = b"retonr:verified-managed-generation-path:v1\0";
const REQUIRED_CHECKS: [RuntimePackageReviewCheck; 6] = [
    RuntimePackageReviewCheck::SourceLineage,
    RuntimePackageReviewCheck::Transformation,
    RuntimePackageReviewCheck::License,
    RuntimePackageReviewCheck::NativeClosure,
    RuntimePackageReviewCheck::ManagedStartup,
    RuntimePackageReviewCheck::CloudDisable,
];

/// Content-derived identity of one exact admitted runtime evidence join.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct VerifiedAdmittedRuntimeId(Digest);

impl VerifiedAdmittedRuntimeId {
    /// Returns the digest defining this admitted runtime join.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Opaque application-owned authority for one admitted runtime.
///
/// Construction requires all six passed controls to name the same foundation.
/// The separately verified schema-2 review must have every control passed and
/// must reconstruct the exact package and reviewed source identities named by
/// that foundation. This type is intentionally not cloneable or serializable.
pub struct VerifiedAdmittedRuntime {
    admitted_runtime_id: VerifiedAdmittedRuntimeId,
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    cloud_disable_version_status: OllamaCloudDisableVersionStatus,
    runtime_package_manifest_id: RuntimePackageManifestId,
    source_build_inputs_id: ArtifactSetId,
    package_source_id: PackageSourceId,
    startup_launch_spec_digest: Digest,
}

impl VerifiedAdmittedRuntime {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn exact_candidate_precursor_test_fixture(
        package: &RuntimePackageManifest,
        frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    ) -> Self {
        let source_build_inputs_id = ArtifactSetId::from_digest(Digest::sha256(
            b"candidate precursor reviewed source inputs",
        ));
        let foundation = crate::RuntimeAdmissionEvidenceFoundation::compile(
            crate::RuntimeAdmissionEvidenceFoundationInput::new(
                ArtifactSetId::from_digest(Digest::sha256(b"candidate precursor evidence")),
                source_build_inputs_id.clone(),
                Digest::sha256(b"candidate precursor source manifest"),
                Digest::sha256(b"candidate precursor build plan"),
                Digest::sha256(b"candidate precursor source report"),
                package.runtime_package_manifest_id(),
            ),
        )
        .expect("valid candidate precursor test foundation");
        Self {
            admitted_runtime_id: VerifiedAdmittedRuntimeId(Digest::sha256(
                b"candidate precursor admitted runtime",
            )),
            foundation_id: foundation.foundation_id().clone(),
            frozen_external_component_set_id,
            cloud_disable_version_status: OllamaCloudDisableVersionStatus::Reviewed,
            runtime_package_manifest_id: package.runtime_package_manifest_id(),
            source_build_inputs_id,
            package_source_id: package.source().package_source_id(),
            startup_launch_spec_digest: Digest::sha256(b"candidate precursor launch"),
        }
    }

    #[cfg(all(test, target_os = "linux", feature = "managed-launch-live-fixture"))]
    pub(crate) fn exact_launch_test_fixture(
        package: &RuntimePackageManifest,
        startup_launch_spec_digest: Digest,
    ) -> Self {
        let source_build_inputs_id =
            ArtifactSetId::from_digest(Digest::sha256(b"managed Ollama live launch source inputs"));
        let foundation = crate::RuntimeAdmissionEvidenceFoundation::compile(
            crate::RuntimeAdmissionEvidenceFoundationInput::new(
                ArtifactSetId::from_digest(Digest::sha256(b"managed Ollama live launch evidence")),
                source_build_inputs_id.clone(),
                Digest::sha256(b"managed Ollama live launch source manifest"),
                Digest::sha256(b"managed Ollama live launch build plan"),
                Digest::sha256(b"managed Ollama live launch source report"),
                package.runtime_package_manifest_id(),
            ),
        )
        .expect("valid managed Ollama live launch foundation");
        let frozen_external_component_set_id = serde_json::from_value(serde_json::json!(
            Digest::sha256(b"managed Ollama live launch frozen native set")
        ))
        .expect("valid frozen native set fixture identity");
        Self {
            admitted_runtime_id: VerifiedAdmittedRuntimeId(Digest::sha256(
                b"managed Ollama live launch admission",
            )),
            foundation_id: foundation.foundation_id().clone(),
            frozen_external_component_set_id,
            cloud_disable_version_status: OllamaCloudDisableVersionStatus::Reviewed,
            runtime_package_manifest_id: package.runtime_package_manifest_id(),
            source_build_inputs_id,
            package_source_id: package.source().package_source_id(),
            startup_launch_spec_digest,
        }
    }

    /// Joins all six passed controls with an independently byte-verified review.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionAuthorityError`] if any control names another
    /// foundation, the managed controls do not name one record, the review is
    /// not all-pass and admitted, or its package or source identities differ.
    #[expect(
        clippy::too_many_arguments,
        reason = "all six independent control capabilities remain explicit"
    )]
    pub fn verify(
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        source_lineage: &VerifiedPassedRuntimeAdmissionSourceLineageControl,
        transformation: &VerifiedPassedRuntimeAdmissionTransformationControl,
        license: &VerifiedPassedRuntimeAdmissionLicenseControl,
        native_closure: &VerifiedPassedRuntimeAdmissionNativeClosureControl,
        managed_startup: &VerifiedPassedRuntimeAdmissionManagedStartupControl,
        cloud_disable: &VerifiedPassedRuntimeAdmissionCloudDisableControl,
        review: &VerifiedRuntimePackageReviewV2,
    ) -> Result<Self, RuntimeAdmissionAuthorityError> {
        let foundation_id = foundation.foundation_id();
        validate_control_bindings(
            foundation_id,
            [
                source_lineage.foundation_id(),
                transformation.foundation_id(),
                license.foundation_id(),
                native_closure.foundation_id(),
                managed_startup.foundation_id(),
                cloud_disable.foundation_id(),
            ],
            managed_startup.record_id() == cloud_disable.record_id(),
            managed_startup.native_record_id() == native_closure.record_id()
                && cloud_disable.native_record_id() == native_closure.record_id(),
            managed_startup.frozen_external_component_set_id()
                == native_closure.frozen_external_component_set_id()
                && cloud_disable.frozen_external_component_set_id()
                    == native_closure.frozen_external_component_set_id(),
        )?;
        if cloud_disable.version_status() != OllamaCloudDisableVersionStatus::Reviewed {
            return Err(RuntimeAdmissionAuthorityError::CloudDisableUnreviewed);
        }

        let review_declaration = review.review();
        let all_passed = REQUIRED_CHECKS.iter().all(|check| {
            review_declaration.check_status(*check) == RuntimePackageReviewCheckStatus::Passed
        });
        let (layout_digest, declared_package_id) = match review_declaration.disposition() {
            RuntimePackageReviewDispositionV2::NotAdmitted { .. } => (None, None),
            RuntimePackageReviewDispositionV2::Admitted {
                layout_digest,
                runtime_package_manifest_id,
                ..
            } => (Some(layout_digest), Some(runtime_package_manifest_id)),
        };
        let reconstructed_package_id = review
            .reconstructed_runtime()
            .map(|runtime| runtime.runtime_package().runtime_package_manifest_id());
        let reviewed_source_build_inputs_id = review
            .source_build_inputs()
            .artifact_set()
            .artifact_set_id();
        validate_review_relationships(
            all_passed,
            layout_digest.is_some(),
            declared_package_id,
            reconstructed_package_id.as_ref(),
            foundation.runtime_package_manifest_id(),
            &reviewed_source_build_inputs_id,
            foundation.source_build_inputs_id(),
        )?;
        let reconstructed = review
            .reconstructed_runtime()
            .ok_or(RuntimeAdmissionAuthorityError::ReviewNotAdmitted)?;
        let package = reconstructed.runtime_package();
        let package_id =
            reconstructed_package_id.ok_or(RuntimeAdmissionAuthorityError::ReviewNotAdmitted)?;
        let layout_digest =
            layout_digest.ok_or(RuntimeAdmissionAuthorityError::ReviewNotAdmitted)?;

        let package_source_id = package.source().package_source_id();
        let binding = AdmittedRuntimeBinding {
            cloud_disable_control_digest: cloud_disable.control_digest().clone(),
            cloud_disable_version_status: cloud_disable.version_status(),
            foundation_id: foundation_id.clone(),
            frozen_external_component_set_id: native_closure
                .frozen_external_component_set_id()
                .clone(),
            layout_digest: layout_digest.clone(),
            license_control_digest: license.control_id().digest().clone(),
            managed_startup_control_digest: managed_startup.control_digest().clone(),
            startup_launch_spec_digest: managed_startup.startup_launch_spec_digest().clone(),
            native_closure_control_digest: native_closure.control_digest().clone(),
            package_source_id: package_source_id.clone(),
            runtime_package_manifest_id: package_id.clone(),
            source_build_inputs_id: reviewed_source_build_inputs_id.clone(),
            source_lineage_control_digest: source_lineage.control_id().digest().clone(),
            transformation_control_digest: transformation.control_id().digest().clone(),
        };
        let admitted_runtime_id = VerifiedAdmittedRuntimeId(domain_digest(
            ADMITTED_RUNTIME_ID_DOMAIN,
            &encode(&binding)?,
        ));
        Ok(Self {
            admitted_runtime_id,
            foundation_id: foundation_id.clone(),
            frozen_external_component_set_id: native_closure
                .frozen_external_component_set_id()
                .clone(),
            cloud_disable_version_status: cloud_disable.version_status(),
            runtime_package_manifest_id: package_id,
            source_build_inputs_id: reviewed_source_build_inputs_id,
            package_source_id,
            startup_launch_spec_digest: managed_startup.startup_launch_spec_digest().clone(),
        })
    }

    /// Returns the identity of this exact admission evidence join.
    #[must_use]
    pub const fn admitted_runtime_id(&self) -> &VerifiedAdmittedRuntimeId {
        &self.admitted_runtime_id
    }

    /// Returns the inert portable identity of this exact admission join.
    ///
    /// This wraps the already-derived owner digest without rehashing. The
    /// portable identity does not carry admission or traffic authority.
    #[must_use]
    pub fn runtime_admission_join_id(&self) -> RuntimeAdmissionJoinId {
        RuntimeAdmissionJoinId::from_derived_digest(self.admitted_runtime_id.digest().clone())
    }

    /// Returns the exact verified admission foundation.
    #[must_use]
    pub const fn foundation_id(&self) -> &RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }

    /// Returns the exact independently verified frozen native-component set.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalNativeComponentSetId {
        &self.frozen_external_component_set_id
    }

    /// Returns the production cloud-disable review status bound by admission.
    #[must_use]
    pub const fn cloud_disable_version_status(&self) -> OllamaCloudDisableVersionStatus {
        self.cloud_disable_version_status
    }

    /// Returns the exact admitted runtime package.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact reviewed source-build input set.
    #[must_use]
    pub const fn source_build_inputs_id(&self) -> &ArtifactSetId {
        &self.source_build_inputs_id
    }

    /// Returns the exact reviewed package-source identity.
    #[must_use]
    pub const fn package_source_id(&self) -> &PackageSourceId {
        &self.package_source_id
    }

    /// Returns the exact plain launch specification digest authorized by the
    /// independently verified managed-startup record.
    #[must_use]
    pub const fn startup_launch_spec_digest(&self) -> &Digest {
        &self.startup_launch_spec_digest
    }
}

impl std::fmt::Debug for VerifiedAdmittedRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedAdmittedRuntime")
            .field("admitted_runtime_id", &self.admitted_runtime_id)
            .field(
                "runtime_package_manifest_id",
                &self.runtime_package_manifest_id,
            )
            .finish_non_exhaustive()
    }
}

/// Opaque authority for one separately reviewed managed-generation worker path.
///
/// The token binds the exact admission join, package, worker executable, stable
/// runtime version, frozen native-component set, package source, and controlled-build
/// source input set. Canonical review bytes are not authority by themselves:
/// public construction also requires the exact production generation policy to
/// return `Reviewed`. The token does not claim model use, semantic correctness,
/// or qualification.
pub struct VerifiedManagedGenerationPath {
    generation_path_id: Digest,
    admitted_runtime_id: VerifiedAdmittedRuntimeId,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    worker_artifact_id: ArtifactId,
    runtime_version: OllamaVersion,
    source_build_inputs_id: ArtifactSetId,
    package_source_id: PackageSourceId,
    review_digest: Digest,
}

impl VerifiedManagedGenerationPath {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn exact_candidate_precursor_test_fixture(
        admitted: &VerifiedAdmittedRuntime,
        package: &RuntimePackageManifest,
    ) -> Self {
        let worker = exact_generation_worker(package).expect("one candidate precursor worker");
        let runtime_version = OllamaVersion::from_str(package.reported_version())
            .expect("candidate precursor runtime version");
        let review = encode(&ManagedGenerationPathReviewWire {
            admitted_runtime_id: admitted.admitted_runtime_id().digest().clone(),
            frozen_external_component_set_id: admitted.frozen_external_component_set_id().clone(),
            package_source_id: admitted.package_source_id().clone(),
            procedure_id: MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_ID.to_owned(),
            procedure_version: MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_VERSION,
            reviewed_source_build_inputs_id: admitted.source_build_inputs_id().clone(),
            runtime_package_manifest_id: package.runtime_package_manifest_id(),
            runtime_version: package.reported_version().to_owned(),
            schema_version: MANAGED_GENERATION_PATH_REVIEW_SCHEMA_VERSION,
            status: GenerationPathReviewStatus::Reviewed,
            worker_artifact_id: worker.artifact_id().clone(),
        })
        .expect("candidate precursor review encoding");
        Self::verify_with_admission_status(
            &review,
            admitted,
            package,
            runtime_version,
            OllamaManagedGenerationAdmissionStatus::Reviewed,
        )
        .expect("candidate precursor reviewed path")
    }

    /// Verifies one exact canonical reviewed generation-path record.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionAuthorityError`] for oversized, malformed,
    /// noncanonical, unsupported, unreviewed, cross-admission, cross-package,
    /// cross-source, cross-version, missing-worker, or multi-worker input.
    pub fn verify(
        review_bytes: &[u8],
        admitted: &VerifiedAdmittedRuntime,
        package: &RuntimePackageManifest,
        expected_runtime_version: OllamaVersion,
    ) -> Result<Self, RuntimeAdmissionAuthorityError> {
        let admission_status =
            OllamaManagedGenerationAdmissionPolicy::assess(expected_runtime_version, package);
        Self::verify_with_admission_status(
            review_bytes,
            admitted,
            package,
            expected_runtime_version,
            admission_status,
        )
    }

    fn verify_with_admission_status(
        review_bytes: &[u8],
        admitted: &VerifiedAdmittedRuntime,
        package: &RuntimePackageManifest,
        expected_runtime_version: OllamaVersion,
        admission_status: OllamaManagedGenerationAdmissionStatus,
    ) -> Result<Self, RuntimeAdmissionAuthorityError> {
        let wire: ManagedGenerationPathReviewWire = parse_canonical(review_bytes)?;
        let runtime_version = OllamaVersion::from_str(&wire.runtime_version)
            .map_err(|_| RuntimeAdmissionAuthorityError::GenerationPathBinding)?;
        let worker = exact_generation_worker(package)?;
        let package_id = package.runtime_package_manifest_id();
        let package_source_id = package.source().package_source_id();
        if wire.schema_version != MANAGED_GENERATION_PATH_REVIEW_SCHEMA_VERSION
            || wire.procedure_id != MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_ID
            || wire.procedure_version != MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_VERSION
            || wire.status != GenerationPathReviewStatus::Reviewed
            || wire.admitted_runtime_id != *admitted.admitted_runtime_id().digest()
            || admitted.cloud_disable_version_status() != OllamaCloudDisableVersionStatus::Reviewed
            || wire.frozen_external_component_set_id != *admitted.frozen_external_component_set_id()
            || wire.runtime_package_manifest_id != package_id
            || wire.runtime_package_manifest_id != *admitted.runtime_package_manifest_id()
            || wire.worker_artifact_id != *worker.artifact_id()
            || wire.runtime_version != package.reported_version()
            || runtime_version != expected_runtime_version
            || wire.reviewed_source_build_inputs_id != *admitted.source_build_inputs_id()
            || wire.package_source_id != package_source_id
            || wire.package_source_id != *admitted.package_source_id()
        {
            return Err(RuntimeAdmissionAuthorityError::GenerationPathBinding);
        }
        if admission_status != OllamaManagedGenerationAdmissionStatus::Reviewed {
            return Err(RuntimeAdmissionAuthorityError::GenerationPathUnreviewed);
        }
        let review_digest = Digest::sha256(review_bytes);
        let generation_path_id = domain_digest(GENERATION_PATH_ID_DOMAIN, review_bytes);
        Ok(Self {
            generation_path_id,
            admitted_runtime_id: admitted.admitted_runtime_id().clone(),
            frozen_external_component_set_id: admitted.frozen_external_component_set_id().clone(),
            runtime_package_manifest_id: package_id,
            worker_artifact_id: worker.artifact_id().clone(),
            runtime_version,
            source_build_inputs_id: admitted.source_build_inputs_id().clone(),
            package_source_id,
            review_digest,
        })
    }

    /// Returns the identity of the exact canonical path review.
    #[must_use]
    pub const fn generation_path_id(&self) -> &Digest {
        &self.generation_path_id
    }

    /// Returns the inert portable identity of this exact generation path.
    ///
    /// This wraps the already-derived owner digest without rehashing. The
    /// portable identity does not carry path-review or traffic authority.
    #[must_use]
    pub fn managed_generation_path_id(&self) -> ManagedGenerationPathId {
        ManagedGenerationPathId::from_derived_digest(self.generation_path_id().clone())
    }

    /// Returns the exact admitted runtime join required by this path.
    #[must_use]
    pub const fn admitted_runtime_id(&self) -> &VerifiedAdmittedRuntimeId {
        &self.admitted_runtime_id
    }

    /// Returns the exact frozen native-component set required by this path.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalNativeComponentSetId {
        &self.frozen_external_component_set_id
    }

    /// Returns the exact admitted runtime package.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact reviewed worker executable.
    #[must_use]
    pub const fn worker_artifact_id(&self) -> &ArtifactId {
        &self.worker_artifact_id
    }

    /// Returns the exact reviewed stable runtime version.
    #[must_use]
    pub const fn runtime_version(&self) -> OllamaVersion {
        self.runtime_version
    }

    /// Returns the exact reviewed controlled-build input set.
    #[must_use]
    pub const fn source_build_inputs_id(&self) -> &ArtifactSetId {
        &self.source_build_inputs_id
    }

    /// Returns the exact reviewed package source.
    #[must_use]
    pub const fn package_source_id(&self) -> &PackageSourceId {
        &self.package_source_id
    }

    /// Returns the digest of the exact canonical review bytes.
    #[must_use]
    pub const fn review_digest(&self) -> &Digest {
        &self.review_digest
    }

    /// Returns whether this path remains bound to one exact launch-time runtime.
    ///
    /// This check does not mutate policy or qualify the runtime. Callers must
    /// separately require verified native-component and isolation evidence.
    #[must_use]
    pub fn matches_runtime(
        &self,
        admitted: &VerifiedAdmittedRuntime,
        package: &RuntimePackageManifest,
        runtime_version: OllamaVersion,
        frozen_external_component_set_id: &FrozenExternalNativeComponentSetId,
    ) -> bool {
        let package_id = package.runtime_package_manifest_id();
        self.admitted_runtime_id == *admitted.admitted_runtime_id()
            && self.runtime_package_manifest_id == package_id
            && admitted.runtime_package_manifest_id == package_id
            && admitted.cloud_disable_version_status == OllamaCloudDisableVersionStatus::Reviewed
            && self.frozen_external_component_set_id == *admitted.frozen_external_component_set_id()
            && &self.frozen_external_component_set_id == frozen_external_component_set_id
            && exact_generation_worker(package)
                .is_ok_and(|worker| &self.worker_artifact_id == worker.artifact_id())
            && self.runtime_version == runtime_version
            && self.source_build_inputs_id == *admitted.source_build_inputs_id()
            && self.package_source_id == package.source().package_source_id()
            && self.package_source_id == *admitted.package_source_id()
    }
}

impl std::fmt::Debug for VerifiedManagedGenerationPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedManagedGenerationPath")
            .field("generation_path_id", &self.generation_path_id)
            .field(
                "runtime_package_manifest_id",
                &self.runtime_package_manifest_id,
            )
            .field("worker_artifact_id", &self.worker_artifact_id)
            .finish_non_exhaustive()
    }
}

/// Failure while deriving runtime admission or managed-generation authority.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeAdmissionAuthorityError {
    /// One or more passed controls name another foundation or managed record.
    #[error("runtime admission controls do not share one exact foundation and record")]
    ControlBinding,
    /// The schema-2 runtime review is not admitted with all six controls passed.
    #[error("runtime package review is not all-pass and admitted")]
    ReviewNotAdmitted,
    /// The verified review names another runtime package or reviewed source set.
    #[error("runtime package review does not bind the admission foundation")]
    ReviewBinding,
    /// The cloud-disable control is not production-reviewed for the exact runtime.
    #[error("runtime admission cloud-disable runtime is unreviewed")]
    CloudDisableUnreviewed,
    /// Admission authority binding bytes could not be encoded within their bound.
    #[error("runtime admission authority encoding is invalid")]
    Encoding,
    /// The managed-generation path review exceeded its hard byte ceiling.
    #[error("managed-generation path review exceeds its byte limit")]
    GenerationPathLimit,
    /// The managed-generation path review is malformed or noncanonical.
    #[error("managed-generation path review encoding is invalid")]
    GenerationPathEncoding,
    /// The managed-generation path review does not bind the exact admitted path.
    #[error("managed-generation path review binding is invalid")]
    GenerationPathBinding,
    /// The exact package and worker are absent from production generation policy.
    #[error("managed-generation path is unreviewed by production policy")]
    GenerationPathUnreviewed,
}

#[cfg(test)]
mod tests;
