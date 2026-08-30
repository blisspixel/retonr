use std::collections::HashMap;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, EffectivePackageEvidenceMode, EffectivePackageEvidenceRoleV2,
    EffectivePackageEvidenceV2, EffectivePackageMemberEvidenceV2, EffectivePackageMemberPurpose,
    EffectivePackageMemberUseV2, EmbeddedModelComponentPurpose, ModelPackageManifest,
    ModelPackageMemberRole, NativeMappingClass, PackageTransformation,
    PackageTransformationDisposition, RuntimeBuildIdentity, RuntimeBuildMode,
    RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMemberRole,
};
use rewrite_runtime_attestor::{
    ExpectedExternalNativeComponent, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::{CancellationToken, Digest};

use crate::effective_runtime_state_observation::ManagedOllamaEffectiveRuntimeState;
use crate::{
    ManagedOllamaIsolationLease, ModelLicenseControlId, RuntimePackageLease,
    VerifiedAdmittedRuntime, VerifiedManagedGenerationPath, VerifiedManagedOllamaModelPackageLease,
    managed_ollama_v0_32_15_launch_spec,
};

use super::plan::GenerationEffectivePackageDerivationError;
use super::{
    MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_ID,
    MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_VERSION,
};

const COMPLETENESS_DOMAIN: &[u8] = b"retonr:managed-effective-package:completeness:v1\0";
const ACQUISITION_DOMAIN: &[u8] = b"retonr:managed-effective-package:acquisition:v1\0";
const LICENSE_DOMAIN: &[u8] = b"retonr:managed-effective-package:license-control:v1\0";
const RUNTIME_CLOSURE_DOMAIN: &[u8] =
    b"retonr:managed-effective-package:portable-runtime-closure:v1\0";
const ISOLATION_EXCLUSION_DOMAIN: &[u8] =
    b"retonr:managed-effective-package:isolation-exclusion-policy:v1\0";

mod managed_judge;
pub(crate) use managed_judge::{ManagedJudgeBatchInputs, derive_managed_judge_batch};

pub(super) struct ProductionInputs<'a, 'model, 'runtime> {
    pub(super) runtime_manifest: &'a RuntimePackageManifest,
    pub(super) runtime_package: &'runtime RuntimePackageLease,
    pub(super) model_package: &'model VerifiedManagedOllamaModelPackageLease,
    pub(super) runtime_build: &'a RuntimeBuildIdentity,
    pub(super) effective_state: &'a ManagedOllamaEffectiveRuntimeState,
    pub(super) admitted_runtime: &'a VerifiedAdmittedRuntime,
    pub(super) generation_path: &'a VerifiedManagedGenerationPath,
    pub(super) frozen_components: &'a VerifiedFrozenExternalNativeComponentSet,
    pub(super) prepared_isolation: &'a PreparedIsolation,
    pub(super) managed_ollama: &'a ManagedOllamaIsolationLease<'model>,
    pub(super) cancellation: &'a CancellationToken,
}

pub(crate) struct DerivedEvidence {
    pub(crate) artifact_set: ArtifactSetManifest,
    pub(crate) evidence: EffectivePackageEvidenceV2,
}

pub(super) fn derive(
    input: &ProductionInputs<'_, '_, '_>,
) -> Result<DerivedEvidence, GenerationEffectivePackageDerivationError> {
    if input.cancellation.is_cancelled() {
        return Err(GenerationEffectivePackageDerivationError::Cancelled);
    }
    validate_relationships(input)?;

    let view = input.model_package.private_view();
    let artifact_set = view.artifact_set_manifest().clone();
    let model_manifest = view.model_package_manifest();
    let foundation = view.foundation_evidence();
    let member_evidence = derive_member_evidence(
        model_manifest,
        foundation.provenance_manifest().relative_path(),
    )?;
    let evidence = EffectivePackageEvidenceV2::new(
        &artifact_set,
        input.runtime_build,
        input.effective_state.state(),
        rewrite_model::EffectivePackageEvidenceV2Input {
            evidence_mode: EffectivePackageEvidenceMode::ManagedImmutablePackage,
            evidence_contract_id: MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_ID.to_owned(),
            evidence_contract_schema_version: MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_VERSION,
            member_evidence,
            artifact_set_completeness_evidence_digest: completeness_digest(input.model_package),
            acquisition_evidence_digest: acquisition_digest(input.model_package),
            license_review_evidence_digest: license_digest(
                input.managed_ollama.model_license_control_id(),
            ),
            transformation: transformation(model_manifest.transformation()),
            runtime_load_closure_evidence_digest: runtime_closure_digest(
                input.runtime_manifest,
                input.generation_path,
                input.frozen_components,
                input.effective_state.state().loaded_components_digest(),
            ),
            exclusion_isolation_evidence_digest: isolation_exclusion_digest(
                input.runtime_manifest,
                input.prepared_isolation,
                input.admitted_runtime,
            ),
        },
    )
    .map_err(GenerationEffectivePackageDerivationError::Evidence)?;
    if input.cancellation.is_cancelled() {
        return Err(GenerationEffectivePackageDerivationError::Cancelled);
    }
    Ok(DerivedEvidence {
        artifact_set,
        evidence,
    })
}

fn validate_relationships(
    input: &ProductionInputs<'_, '_, '_>,
) -> Result<(), GenerationEffectivePackageDerivationError> {
    let runtime_package_id = input.runtime_manifest.runtime_package_manifest_id();
    let expected_build = RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        input.runtime_manifest,
    )
    .map_err(|_error| GenerationEffectivePackageDerivationError::RelationshipMismatch)?;
    let model = input.model_package.private_view();
    let managed_input = input.managed_ollama.input_evidence();
    let launch_digest = managed_ollama_v0_32_15_launch_spec().redacted_digest();
    let matches = relationship_checks_pass(&[
        input.runtime_build == &expected_build,
        input
            .managed_ollama
            .binds_exact_model_package(input.model_package),
        input
            .runtime_package
            .evidence()
            .runtime_package_manifest_id()
            == &runtime_package_id,
        input
            .runtime_package
            .installation_key()
            .installation_generation()
            > 0,
        input.admitted_runtime.runtime_package_manifest_id() == &runtime_package_id,
        input.generation_path.runtime_package_manifest_id() == &runtime_package_id,
        input.frozen_components.runtime_package_manifest_id() == &runtime_package_id,
        input.generation_path.matches_runtime(
            input.admitted_runtime,
            input.runtime_manifest,
            input.generation_path.runtime_version(),
            input.frozen_components.frozen_set_id(),
        ),
        input.effective_state.state().runtime_build_id() == &input.runtime_build.runtime_build_id(),
        input.effective_state.binds_effective_package_inputs(
            input.admitted_runtime,
            input.generation_path,
            input.runtime_package,
            input.managed_ollama,
        ),
        managed_input.artifact_set_id() == &model.artifact_set_manifest().artifact_set_id(),
        managed_input.model_package_manifest_id()
            == &model.model_package_manifest().model_package_manifest_id(),
        managed_input.installation_generation() == model.installation_generation(),
        input.managed_ollama.plain_launch_spec_digest() == &launch_digest,
        input.admitted_runtime.startup_launch_spec_digest() == &launch_digest,
        input.managed_ollama.isolation_policy_digest() == &input.prepared_isolation.policy_digest(),
    ]);
    if matches {
        Ok(())
    } else {
        Err(GenerationEffectivePackageDerivationError::RelationshipMismatch)
    }
}

pub(super) fn relationship_checks_pass(checks: &[bool]) -> bool {
    checks.iter().all(|check| *check)
}

pub(super) fn derive_member_evidence(
    package: &ModelPackageManifest,
    raw_provenance_path: &rewrite_model::ArtifactSetRelativePath,
) -> Result<Vec<EffectivePackageMemberEvidenceV2>, GenerationEffectivePackageDerivationError> {
    let mut uses = package
        .members()
        .iter()
        .map(|member| {
            let mut purposes = Vec::new();
            let mut roles = Vec::new();
            for role in member.roles() {
                match role {
                    ModelPackageMemberRole::LicenseText => {
                        push_role(&mut roles, EffectivePackageEvidenceRoleV2::LicenseText);
                    }
                    ModelPackageMemberRole::ProvenanceRecord => {
                        push_role(&mut roles, EffectivePackageEvidenceRoleV2::ProvenanceRecord);
                    }
                    ModelPackageMemberRole::TransformationEvidence => push_role(
                        &mut roles,
                        EffectivePackageEvidenceRoleV2::TransformationEvidence,
                    ),
                    role => push_purpose(&mut purposes, purpose_for_role(*role)),
                }
            }
            if member.relative_path() == raw_provenance_path {
                push_purpose(&mut purposes, EffectivePackageMemberPurpose::AuxiliaryData);
                push_role(&mut roles, EffectivePackageEvidenceRoleV2::ProvenanceRecord);
            }
            (member, purposes, roles)
        })
        .collect::<Vec<_>>();
    for embedded in package.embedded_components() {
        let (_, purposes, _) = uses
            .iter_mut()
            .find(|(member, _, _)| member.relative_path() == embedded.container_path())
            .ok_or(GenerationEffectivePackageDerivationError::RelationshipMismatch)?;
        push_purpose(purposes, purpose_for_embedded(embedded.purpose()));
    }
    let mut unions = HashMap::<ArtifactId, Vec<EffectivePackageMemberPurpose>>::new();
    for (member, purposes, _) in &uses {
        let union = unions.entry(member.artifact_id().clone()).or_default();
        for purpose in purposes {
            push_purpose(union, *purpose);
        }
    }
    uses.into_iter()
        .map(|(member, _, roles)| {
            let purposes = unions
                .get(member.artifact_id())
                .cloned()
                .unwrap_or_default();
            let member_use = match (purposes.is_empty(), roles.is_empty()) {
                (false, true) => EffectivePackageMemberUseV2::Effective { purposes },
                (true, false) => EffectivePackageMemberUseV2::EvidenceOnly { roles },
                (false, false) => {
                    EffectivePackageMemberUseV2::EffectiveAndEvidence { purposes, roles }
                }
                (true, true) => {
                    return Err(GenerationEffectivePackageDerivationError::RelationshipMismatch);
                }
            };
            EffectivePackageMemberEvidenceV2::new(
                member.relative_path().clone(),
                member.artifact_id().clone(),
                member.byte_size(),
                member_use,
            )
            .map_err(GenerationEffectivePackageDerivationError::Evidence)
        })
        .collect()
}

pub(super) fn purpose_for_role(role: ModelPackageMemberRole) -> EffectivePackageMemberPurpose {
    match role {
        ModelPackageMemberRole::ModelWeights | ModelPackageMemberRole::ModelWeightShard => {
            EffectivePackageMemberPurpose::ModelWeights
        }
        ModelPackageMemberRole::ModelShardIndex => EffectivePackageMemberPurpose::ModelShardIndex,
        ModelPackageMemberRole::ModelConfiguration => {
            EffectivePackageMemberPurpose::ModelConfiguration
        }
        ModelPackageMemberRole::GenerationConfiguration => {
            EffectivePackageMemberPurpose::GenerationConfiguration
        }
        ModelPackageMemberRole::TokenizerModel => EffectivePackageMemberPurpose::TokenizerModel,
        ModelPackageMemberRole::TokenizerVocabulary => {
            EffectivePackageMemberPurpose::TokenizerVocabulary
        }
        ModelPackageMemberRole::TokenizerMerges => EffectivePackageMemberPurpose::TokenizerMerges,
        ModelPackageMemberRole::TokenizerConfiguration => {
            EffectivePackageMemberPurpose::TokenizerConfiguration
        }
        ModelPackageMemberRole::PromptTemplate => EffectivePackageMemberPurpose::PromptTemplate,
        ModelPackageMemberRole::SystemPrompt => EffectivePackageMemberPurpose::SystemPrompt,
        ModelPackageMemberRole::Adapter => EffectivePackageMemberPurpose::Adapter,
        ModelPackageMemberRole::Projector => EffectivePackageMemberPurpose::Projector,
        ModelPackageMemberRole::DraftModel => EffectivePackageMemberPurpose::DraftModel,
        ModelPackageMemberRole::GrammarOrSchema => EffectivePackageMemberPurpose::GrammarOrSchema,
        ModelPackageMemberRole::CustomModelCode => EffectivePackageMemberPurpose::CustomModelCode,
        ModelPackageMemberRole::CustomGenerationCode => {
            EffectivePackageMemberPurpose::CustomGenerationCode
        }
        ModelPackageMemberRole::AuxiliaryData => EffectivePackageMemberPurpose::AuxiliaryData,
        ModelPackageMemberRole::LicenseText
        | ModelPackageMemberRole::ProvenanceRecord
        | ModelPackageMemberRole::TransformationEvidence => {
            unreachable!("evidence-only role is handled before purpose mapping")
        }
    }
}

const fn purpose_for_embedded(
    purpose: EmbeddedModelComponentPurpose,
) -> EffectivePackageMemberPurpose {
    match purpose {
        EmbeddedModelComponentPurpose::ModelConfiguration => {
            EffectivePackageMemberPurpose::ModelConfiguration
        }
        EmbeddedModelComponentPurpose::GenerationConfiguration => {
            EffectivePackageMemberPurpose::GenerationConfiguration
        }
        EmbeddedModelComponentPurpose::Tokenizer => EffectivePackageMemberPurpose::TokenizerModel,
        EmbeddedModelComponentPurpose::PromptTemplate => {
            EffectivePackageMemberPurpose::PromptTemplate
        }
    }
}

fn push_purpose(
    purposes: &mut Vec<EffectivePackageMemberPurpose>,
    purpose: EffectivePackageMemberPurpose,
) {
    if !purposes.contains(&purpose) {
        purposes.push(purpose);
        purposes.sort_unstable_by_key(|purpose| purpose_rank(*purpose));
    }
}

fn push_role(
    roles: &mut Vec<EffectivePackageEvidenceRoleV2>,
    role: EffectivePackageEvidenceRoleV2,
) {
    if !roles.contains(&role) {
        roles.push(role);
        roles.sort_unstable_by_key(|role| evidence_role_rank(*role));
    }
}

const fn purpose_rank(purpose: EffectivePackageMemberPurpose) -> u8 {
    match purpose {
        EffectivePackageMemberPurpose::ModelWeights => 0,
        EffectivePackageMemberPurpose::ModelShardIndex => 1,
        EffectivePackageMemberPurpose::ModelConfiguration => 2,
        EffectivePackageMemberPurpose::GenerationConfiguration => 3,
        EffectivePackageMemberPurpose::TokenizerModel => 4,
        EffectivePackageMemberPurpose::TokenizerVocabulary => 5,
        EffectivePackageMemberPurpose::TokenizerMerges => 6,
        EffectivePackageMemberPurpose::TokenizerConfiguration => 7,
        EffectivePackageMemberPurpose::PromptTemplate => 8,
        EffectivePackageMemberPurpose::SystemPrompt => 9,
        EffectivePackageMemberPurpose::GrammarOrSchema => 10,
        EffectivePackageMemberPurpose::Adapter => 11,
        EffectivePackageMemberPurpose::Projector => 12,
        EffectivePackageMemberPurpose::DraftModel => 13,
        EffectivePackageMemberPurpose::CustomModelCode => 14,
        EffectivePackageMemberPurpose::CustomGenerationCode => 15,
        EffectivePackageMemberPurpose::AuxiliaryData => 16,
    }
}

const fn evidence_role_rank(role: EffectivePackageEvidenceRoleV2) -> u8 {
    match role {
        EffectivePackageEvidenceRoleV2::LicenseText => 0,
        EffectivePackageEvidenceRoleV2::ProvenanceRecord => 1,
        EffectivePackageEvidenceRoleV2::TransformationEvidence => 2,
    }
}

fn completeness_digest(package: &VerifiedManagedOllamaModelPackageLease) -> Digest {
    let view = package.private_view();
    let foundation = view.foundation_evidence();
    hash(COMPLETENESS_DOMAIN, |bytes| {
        append_digest(bytes, package.foundation_id().digest());
        append_digest(bytes, foundation.artifact_set_id().digest());
        append_digest(bytes, foundation.model_package_manifest_id().digest());
        append_digest(bytes, foundation.descriptor_mapping_digest());
        append_digest(bytes, foundation.logical_binding_digest());
    })
}

fn acquisition_digest(package: &VerifiedManagedOllamaModelPackageLease) -> Digest {
    let view = package.private_view();
    let foundation = view.foundation_evidence();
    hash(ACQUISITION_DOMAIN, |bytes| {
        append_digest(bytes, foundation.package_source_id().digest());
        append_text(
            bytes,
            foundation.provenance_manifest().relative_path().as_str(),
        );
        append_digest(
            bytes,
            foundation.provenance_manifest().artifact_id().digest(),
        );
        bytes.extend_from_slice(&foundation.provenance_manifest().byte_size().to_be_bytes());
    })
}

fn license_digest(control_id: &ModelLicenseControlId) -> Digest {
    hash(LICENSE_DOMAIN, |bytes| {
        append_digest(bytes, control_id.digest());
    })
}

fn runtime_closure_digest(
    runtime: &RuntimePackageManifest,
    generation_path: &VerifiedManagedGenerationPath,
    frozen: &VerifiedFrozenExternalNativeComponentSet,
    portable_actual_closure: &Digest,
) -> Digest {
    let runtime_package_id = runtime.runtime_package_manifest_id();
    runtime_closure_digest_from_portable_facts(
        runtime_package_id.digest(),
        generation_path.worker_artifact_id(),
        frozen.expected_components(),
        portable_actual_closure,
    )
}

pub(super) fn runtime_closure_digest_from_portable_facts(
    runtime_package_id: &Digest,
    worker_artifact_id: &ArtifactId,
    expected_components: &[ExpectedExternalNativeComponent],
    portable_actual_closure: &Digest,
) -> Digest {
    hash(RUNTIME_CLOSURE_DOMAIN, |bytes| {
        append_digest(bytes, runtime_package_id);
        append_digest(bytes, worker_artifact_id.digest());
        append_count(bytes, expected_components.len());
        for component in expected_components {
            append_digest(bytes, component.artifact_id().digest());
            bytes.extend_from_slice(&component.byte_size().to_be_bytes());
            bytes.push(mapping_class_rank(component.mapping_class()));
        }
        append_digest(bytes, portable_actual_closure);
    })
}

fn isolation_exclusion_digest(
    runtime: &RuntimePackageManifest,
    prepared: &PreparedIsolation,
    admitted: &VerifiedAdmittedRuntime,
) -> Digest {
    isolation_exclusion_digest_from_portable_facts(
        runtime,
        &prepared.policy_digest(),
        admitted.startup_launch_spec_digest(),
    )
}

pub(super) fn isolation_exclusion_digest_from_portable_facts(
    runtime: &RuntimePackageManifest,
    isolation_policy_digest: &Digest,
    startup_launch_spec_digest: &Digest,
) -> Digest {
    hash(ISOLATION_EXCLUSION_DOMAIN, |bytes| {
        append_digest(bytes, runtime.runtime_package_manifest_id().digest());
        append_digest(bytes, isolation_policy_digest);
        append_digest(bytes, startup_launch_spec_digest);
        let excluded = runtime
            .members()
            .iter()
            .filter(|member| member.load_policy() == RuntimePackageLoadPolicy::MustNotBeCodeLoaded)
            .collect::<Vec<_>>();
        append_count(bytes, excluded.len());
        for member in excluded {
            append_text(bytes, member.relative_path().as_str());
            append_digest(bytes, member.artifact_id().digest());
            bytes.extend_from_slice(&member.byte_size().to_be_bytes());
            append_count(bytes, member.roles().len());
            for role in member.roles() {
                bytes.push(runtime_role_rank(*role));
            }
        }
    })
}

const fn mapping_class_rank(class: NativeMappingClass) -> u8 {
    match class {
        NativeMappingClass::ExecutableImage => 0,
        NativeMappingClass::ExecutableMapped => 1,
        NativeMappingClass::DataMapped => 2,
    }
}

const fn runtime_role_rank(role: RuntimePackageMemberRole) -> u8 {
    match role {
        RuntimePackageMemberRole::Entrypoint => 0,
        RuntimePackageMemberRole::NativeDependency => 1,
        RuntimePackageMemberRole::HelperExecutable => 2,
        RuntimePackageMemberRole::RuntimeResource => 3,
        RuntimePackageMemberRole::DefaultConfiguration => 4,
        RuntimePackageMemberRole::BuildConfiguration => 5,
        RuntimePackageMemberRole::LicenseText => 6,
        RuntimePackageMemberRole::ProvenanceRecord => 7,
        RuntimePackageMemberRole::TransformationRecord => 8,
        RuntimePackageMemberRole::WorkerExecutable => 9,
        RuntimePackageMemberRole::UtilityExecutable => 10,
    }
}

pub(super) fn transformation(value: &PackageTransformation) -> PackageTransformationDisposition {
    match value {
        PackageTransformation::Untransformed { evidence_digest } => {
            PackageTransformationDisposition::Untransformed {
                evidence_digest: evidence_digest.clone(),
            }
        }
        PackageTransformation::Transformed {
            source_artifact_set_id,
            tool_evidence_digest,
            parameters_digest,
            log_digest,
        } => PackageTransformationDisposition::Transformed {
            source_artifact_set_id: source_artifact_set_id.clone(),
            process_evidence_digest: tool_evidence_digest.clone(),
            parameters_digest: parameters_digest.clone(),
            log_digest: log_digest.clone(),
        },
    }
}

fn hash(domain: &[u8], append: impl FnOnce(&mut Vec<u8>)) -> Digest {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    append(&mut bytes);
    Digest::sha256(&bytes)
}

fn append_digest(bytes: &mut Vec<u8>, value: &Digest) {
    bytes.extend_from_slice(value.as_str().as_bytes());
}

fn append_text(bytes: &mut Vec<u8>, value: &str) {
    append_count(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn append_count(bytes: &mut Vec<u8>, value: usize) {
    let value = u64::try_from(value).expect("bounded manifest counts fit u64");
    bytes.extend_from_slice(&value.to_be_bytes());
}
