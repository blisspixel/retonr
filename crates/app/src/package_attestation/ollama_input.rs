use std::{fmt, fs::File, marker::PhantomData};

use rewrite_model::{ArtifactId, ArtifactSetId, ModelPackageManifest, ModelPackageManifestId};
use rewrite_ollama_package::{ReconstructionError, ReconstructionLimits};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerError, RetainedModelWeight, RetainedModelWeightSink,
    RetainedModelWeightSource,
};
use rewrite_runtime_isolation::{
    IsolationError, IsolationResult, MANAGED_RUNTIME_INPUT_ROOT_V1, RetainedRuntimeInputSink,
    RetainedRuntimeInputSource, RetainedRuntimeInputTree,
};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use crate::OllamaModelReference;

use super::{
    ManagedOllamaModelPackageError, ModelPackageFoundationId, PackageAttestationError,
    PackageAttestationService, RetainedModelPackageIsolationSource,
    VerifiedManagedOllamaModelPackageLease,
};

mod launch;
mod validation;

pub use launch::{
    MANAGED_OLLAMA_V0_32_15_ENDPOINT, ManagedOllamaCloseError, ManagedOllamaIsolationLease,
    ManagedOllamaLaunchError, ManagedOllamaLaunchFinalizationFailures,
    ManagedOllamaModelAuthorityError, ManagedOllamaPostAcquisitionFailure,
    VerifiedManagedOllamaLaunchPlan, managed_ollama_v0_32_15_launch_spec,
};

/// Version of the redacted managed Ollama input mapping evidence.
pub const MANAGED_OLLAMA_INPUT_SCHEMA_VERSION: u32 = 2;
/// Redacted evidence joining one retained model package to one isolation input tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedOllamaInputEvidence {
    schema_version: u32,
    artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    installation_generation: u64,
    reference_digest: Digest,
    runtime_reference_digest: Digest,
    mapping_digest: Digest,
    input_layout_digest: Digest,
    model_artifact_id: ArtifactId,
    model_target_digest: Digest,
    member_count: u32,
    total_bytes: u64,
}

impl ManagedOllamaInputEvidence {
    /// Returns the evidence schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact managed byte-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the exact positive local installation generation.
    #[must_use]
    pub const fn installation_generation(&self) -> u64 {
        self.installation_generation
    }

    /// Returns a redacted binding of registry, namespace, model, and explicit tag.
    #[must_use]
    pub const fn reference_digest(&self) -> &Digest {
        &self.reference_digest
    }

    /// Returns the digest of the exact shortest reference sent to Ollama's API.
    #[must_use]
    pub const fn runtime_reference_digest(&self) -> &Digest {
        &self.runtime_reference_digest
    }

    /// Returns a redacted binding of every target alias and retained object.
    #[must_use]
    pub const fn mapping_digest(&self) -> &Digest {
        &self.mapping_digest
    }

    /// Returns the isolation tree's object-bound layout digest.
    #[must_use]
    pub const fn input_layout_digest(&self) -> &Digest {
        &self.input_layout_digest
    }

    /// Returns the unique selected GGUF byte identity.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }

    /// Returns a redacted binding of the selected target-visible GGUF path.
    #[must_use]
    pub const fn model_target_digest(&self) -> &Digest {
        &self.model_target_digest
    }

    /// Returns the exact target-visible retained member count.
    #[must_use]
    pub const fn member_count(&self) -> u32 {
        self.member_count
    }

    /// Returns the checked target-visible retained byte total.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

/// Exact target-visible selected GGUF identity for later worker observation.
#[derive(Clone, Eq, PartialEq)]
pub struct ManagedOllamaModelTarget {
    target_path: String,
    artifact_id: ArtifactId,
    target_digest: Digest,
}

impl ManagedOllamaModelTarget {
    /// Returns the fixed-root target path expected in the worker's `--model` argument.
    ///
    /// This is an isolation target path, never a host filesystem path.
    #[must_use]
    pub fn target_path(&self) -> &str {
        &self.target_path
    }

    /// Returns the exact selected GGUF byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns a redacted binding of the target path.
    #[must_use]
    pub const fn target_digest(&self) -> &Digest {
        &self.target_digest
    }
}

impl fmt::Debug for ManagedOllamaModelTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedOllamaModelTarget")
            .field("artifact_id", &self.artifact_id)
            .field("target_digest", &self.target_digest)
            .finish_non_exhaustive()
    }
}

/// Opaque managed Ollama input tree and the exact evidence needed to join its launch.
pub(crate) struct ManagedOllamaInputPlan<'lease> {
    tree: RetainedRuntimeInputTree,
    evidence: ManagedOllamaInputEvidence,
    model_target: ManagedOllamaModelTarget,
    retained_model_weight: RetainedModelWeight<'lease>,
    _lease: PhantomData<&'lease super::ModelPackageLease>,
}

impl<'lease> ManagedOllamaInputPlan<'lease> {
    /// Returns redacted mapping and identity evidence.
    #[must_use]
    pub(crate) const fn evidence(&self) -> &ManagedOllamaInputEvidence {
        &self.evidence
    }

    /// Returns the exact selected target-visible GGUF identity.
    #[must_use]
    pub(crate) const fn model_target(&self) -> &ManagedOllamaModelTarget {
        &self.model_target
    }

    /// Returns the exact retained GGUF object for later worker observation.
    ///
    /// The returned capability exposes neither a host path nor a raw file handle
    /// and remains bound to the originating model-package lease.
    #[must_use]
    pub(crate) const fn retained_model_weight(&self) -> &RetainedModelWeight<'lease> {
        &self.retained_model_weight
    }
}

impl fmt::Debug for ManagedOllamaInputPlan<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedOllamaInputPlan")
            .field("evidence", &self.evidence)
            .field("model_target", &self.model_target)
            .field("retained_model_weight", &self.retained_model_weight)
            .field("tree", &self.tree)
            .finish()
    }
}

/// Noncloneable managed input plan bound to a verified model foundation.
///
/// This wrapper does not expose the weaker inner plan. Later launch authority can
/// therefore require the exact originating live specialized lease, not merely an
/// equal portable foundation or installation generation, without changing the
/// existing managed-input evidence schema.
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedOllamaInputPlan;
///
/// fn clone_plan<'lease>(value: &VerifiedManagedOllamaInputPlan<'lease>) {
///     let _forged: VerifiedManagedOllamaInputPlan<'lease> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedOllamaInputPlan;
///
/// fn serialize_plan(value: &VerifiedManagedOllamaInputPlan<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("plan must not serialize");
/// }
/// ```
pub struct VerifiedManagedOllamaInputPlan<'lease> {
    plan: ManagedOllamaInputPlan<'lease>,
    foundation_id: ModelPackageFoundationId,
    package: &'lease VerifiedManagedOllamaModelPackageLease,
}

impl VerifiedManagedOllamaInputPlan<'_> {
    /// Returns the exact verified model foundation used to prepare this plan.
    #[must_use]
    pub const fn foundation_id(&self) -> &ModelPackageFoundationId {
        &self.foundation_id
    }

    /// Returns redacted managed input mapping evidence.
    #[must_use]
    pub const fn evidence(&self) -> &ManagedOllamaInputEvidence {
        self.plan.evidence()
    }

    /// Returns the selected target-visible GGUF identity.
    #[must_use]
    pub const fn model_target(&self) -> &ManagedOllamaModelTarget {
        self.plan.model_target()
    }

    /// Tests whether this plan retains the exact selected live package lease.
    ///
    /// Portable IDs and installation generations are deliberately insufficient.
    /// Application-owned joins use this predicate together with the model-license
    /// authority's live-lease check before granting later authority.
    #[must_use]
    pub fn binds_exact_package_lease(
        &self,
        selected: &VerifiedManagedOllamaModelPackageLease,
    ) -> bool {
        std::ptr::eq(self.package, selected)
    }

    pub(super) const fn retained_model_weight(&self) -> &RetainedModelWeight<'_> {
        self.plan.retained_model_weight()
    }
}

impl fmt::Debug for VerifiedManagedOllamaInputPlan<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedOllamaInputPlan")
            .field("foundation_id", &self.foundation_id)
            .field("evidence", self.plan.evidence())
            .field("model_target", self.plan.model_target())
            .finish_non_exhaustive()
    }
}

/// Failure while joining a retained model package to managed Ollama input aliases.
#[derive(Debug, Error)]
pub enum ManagedOllamaInputError {
    /// Cancellation was observed before the opaque input plan completed.
    #[error("managed Ollama input preparation was cancelled")]
    Cancelled,
    /// Package identity, source, format, roles, paths, or descriptors did not agree.
    #[error("retained model package does not match the managed Ollama input profile")]
    RelationshipMismatch,
    /// The retained provenance manifest was malformed, unsupported, or over budget.
    #[error("retained Ollama manifest verification failed")]
    Reconstruction(#[source] ReconstructionError),
    /// A retained package object could not be revalidated or read exactly.
    #[error("retained model package verification failed")]
    Package(#[source] PackageAttestationError),
    /// The specialized verified model-package capability failed revalidation.
    #[error("verified managed Ollama model package revalidation failed")]
    VerifiedPackage(#[source] ManagedOllamaModelPackageError),
    /// Isolation rejected an object, alias, limit, or cancellation state.
    #[error("managed Ollama isolation input construction failed")]
    Isolation(#[source] IsolationError),
    /// The worker observer rejected the retained exact model-weight capability.
    #[error("managed Ollama retained model weight construction failed")]
    Worker(#[source] ManagedGenerationWorkerError),
}

impl PackageAttestationService {
    /// Prepares exact managed inputs from one specialized verified model lease.
    ///
    /// The caller cannot provide a model manifest or retained member source. Both
    /// are taken privately from the verified lease and the returned wrapper binds
    /// the resulting input tree to the same stable foundation identity.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaInputError`] on cancellation, foundation or package
    /// drift, reference mismatch, malformed retained provenance, or input refusal.
    pub fn prepare_verified_managed_ollama_inputs<'lease>(
        package: &'lease VerifiedManagedOllamaModelPackageLease,
        reference: &OllamaModelReference,
        reconstruction_limits: &ReconstructionLimits,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedManagedOllamaInputPlan<'lease>, ManagedOllamaInputError> {
        let source = package
            .clone_members_for_verified_input(cancellation)
            .map_err(ManagedOllamaInputError::VerifiedPackage)?;
        let plan = Self::prepare_managed_ollama_inputs(
            source,
            package.model_package(),
            reference,
            reconstruction_limits,
            cancellation,
        )?;
        if plan.evidence().artifact_set_id() != package.artifact_set_id()
            || plan.evidence().model_package_manifest_id() != package.model_package_manifest_id()
        {
            return Err(ManagedOllamaInputError::RelationshipMismatch);
        }
        Ok(VerifiedManagedOllamaInputPlan {
            plan,
            foundation_id: package.foundation_id().clone(),
            package,
        })
    }

    /// Consumes retained model objects into the exact original Ollama store shape.
    ///
    /// The supplied reference provides every manifest path component, including the
    /// explicit tag. This function never supplies or infers `latest`. Five content
    /// objects are mapped by digest below `blobs/`; the retained raw provenance
    /// manifest is mapped below `manifests/`. No member is reopened by host path.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaInputError`] on cancellation, package/reference drift,
    /// malformed provenance, descriptor mismatch, object drift, or isolation refusal.
    pub(crate) fn prepare_managed_ollama_inputs<'lease>(
        source: RetainedModelPackageIsolationSource<'lease>,
        package: &ModelPackageManifest,
        reference: &OllamaModelReference,
        reconstruction_limits: &ReconstructionLimits,
        cancellation: &CancellationToken,
    ) -> Result<ManagedOllamaInputPlan<'lease>, ManagedOllamaInputError> {
        let prepared = validation::prepare(
            source,
            package,
            reference,
            reconstruction_limits,
            cancellation,
        )?;
        let reference_digest = prepared.reference_digest;
        let mapping_digest = prepared.mapping_digest;
        let artifact_set_id = prepared.artifact_set_id;
        let model_package_manifest_id = prepared.model_package_manifest_id;
        let installation_generation = prepared.installation_generation;
        let model_artifact_id = prepared.model_artifact_id;
        let model_target = ManagedOllamaModelTarget {
            target_path: prepared.model_target_path,
            artifact_id: model_artifact_id.clone(),
            target_digest: prepared.model_target_digest.clone(),
        };
        let retained_model_weight =
            RetainedModelWeight::from_source(prepared.model_weight_source, cancellation)
                .map_err(map_worker)?;
        let tree = RetainedRuntimeInputTree::from_source(
            OrderedInputSource(prepared.members),
            cancellation,
        )
        .map_err(map_isolation)?;
        let member_count = u32::try_from(tree.member_count())
            .map_err(|_| ManagedOllamaInputError::RelationshipMismatch)?;
        let evidence = ManagedOllamaInputEvidence {
            schema_version: MANAGED_OLLAMA_INPUT_SCHEMA_VERSION,
            artifact_set_id,
            model_package_manifest_id,
            installation_generation,
            reference_digest,
            runtime_reference_digest: Digest::sha256(reference.runtime_reference().as_bytes()),
            mapping_digest,
            input_layout_digest: tree.redacted_digest().clone(),
            model_artifact_id,
            model_target_digest: prepared.model_target_digest,
            member_count,
            total_bytes: tree.total_bytes(),
        };
        Ok(ManagedOllamaInputPlan {
            tree,
            evidence,
            model_target,
            retained_model_weight,
            _lease: PhantomData,
        })
    }
}

pub(super) struct ExactModelWeightSource<'lease> {
    pub(super) artifact_id: ArtifactId,
    pub(super) byte_size: u64,
    pub(super) file: File,
    pub(super) _lease: PhantomData<&'lease super::ModelPackageLease>,
}

impl<'lease> RetainedModelWeightSource<'lease> for ExactModelWeightSource<'lease> {
    fn transfer(
        self,
        sink: &mut RetainedModelWeightSink<'_, 'lease>,
    ) -> Result<(), ManagedGenerationWorkerError> {
        sink.retain(self.artifact_id, self.byte_size, self.file)
    }
}

pub(super) struct OrderedInputMember {
    pub(super) alias: String,
    pub(super) digest: Digest,
    pub(super) byte_size: u64,
    pub(super) file: File,
}

struct OrderedInputSource(Vec<OrderedInputMember>);

impl RetainedRuntimeInputSource for OrderedInputSource {
    fn transfer(self, sink: &mut RetainedRuntimeInputSink<'_>) -> IsolationResult<()> {
        for member in self.0 {
            sink.retain(member.alias, member.digest, member.byte_size, member.file)?;
        }
        Ok(())
    }
}

fn map_isolation(error: IsolationError) -> ManagedOllamaInputError {
    if matches!(error, IsolationError::Cancelled) {
        ManagedOllamaInputError::Cancelled
    } else {
        ManagedOllamaInputError::Isolation(error)
    }
}

fn map_worker(error: ManagedGenerationWorkerError) -> ManagedOllamaInputError {
    if matches!(error, ManagedGenerationWorkerError::Cancelled) {
        ManagedOllamaInputError::Cancelled
    } else {
        ManagedOllamaInputError::Worker(error)
    }
}

pub(super) fn target_path(alias: &str) -> String {
    format!("{MANAGED_RUNTIME_INPUT_ROOT_V1}/{alias}")
}
