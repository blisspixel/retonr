use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    NativeLoadEvidenceClass, NativeLoadObservation, NativeLoadObservationInput, NativeLoadOrigin,
    NativeLoadVisibilityScope, NativeLoadedComponent, NativeMappingClass, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture, RuntimeIdentity,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_ollama::{
    OllamaInventoryEntry, OllamaModelDetails, OllamaPreflight, OllamaPreflightBinding,
};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    RetainedTcpConnectionEvidence, RetainedTcpConnectionEvidenceInput,
    TcpConnectionAttributionKind, TcpConnectionSharingLimitation,
};
use rewrite_types::Digest;

use crate::{
    LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION, LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
    LocalOllamaBoundPreflightPlan, LocalOllamaModelPlan, LocalOllamaPreflightMode,
    LocalOllamaPreflightPlan, LocalOllamaPreflightReport,
};

pub(super) fn artifact(value: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(value.as_bytes()))
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid path")
}

pub(super) fn package() -> RuntimePackageManifest {
    let members = vec![
        RuntimePackageMember::new(
            artifact("entrypoint"),
            11,
            path("bin/ollama"),
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        RuntimePackageMember::new(
            artifact("helper"),
            7,
            path("helper/isolation"),
            vec![RuntimePackageMemberRole::HelperExecutable],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("license"),
            5,
            path("legal/license"),
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("provenance"),
            9,
            path("legal/provenance"),
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
    ];
    let set = ArtifactSetManifest::new(
        members
            .iter()
            .map(|member| {
                ArtifactSetMember::new(
                    member.artifact_id().clone(),
                    member.byte_size(),
                    member.relative_path().clone(),
                )
            })
            .collect(),
    )
    .expect("valid set");
    RuntimePackageManifest::new(
        &set,
        "ollama",
        "0.16.2",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("target"),
        PackageSource::new(
            PackageSourceKind::UpstreamRelease,
            "https://example.invalid/ollama",
            "v0.16.2",
            Digest::sha256(b"source"),
        )
        .expect("source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"same"),
        },
        members,
    )
    .expect("package")
}

fn details() -> OllamaModelDetails {
    OllamaModelDetails {
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        quantization: "Q4_K_M".to_owned(),
        capabilities: vec!["completion".to_owned()],
        license_digest: Digest::sha256(b"license"),
        template_digest: Digest::sha256(b"template"),
        metadata_digest: Digest::sha256(b"metadata"),
    }
}

pub(super) fn plan(package: &RuntimePackageManifest) -> LocalOllamaBoundPreflightPlan {
    LocalOllamaBoundPreflightPlan {
        schema_version: LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION,
        preflight: LocalOllamaPreflightPlan {
            schema_version: LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
            plan_id: "managed-fixture".to_owned(),
            mode: LocalOllamaPreflightMode::Verify,
            endpoint: "http://127.0.0.1:11434".to_owned(),
            expected_runtime_version: "0.16.2".to_owned(),
            require_idle: true,
            models: vec![LocalOllamaModelPlan {
                reference: "fixture:latest".to_owned(),
                inventory_digest: Digest::sha256(b"inventory"),
                expected_details: Some(details()),
            }],
        },
        maximum_entrypoint_bytes: 1024,
        maximum_session_body_bytes: 1024 * 1024,
        expected_entrypoint_digest: Some(package.entrypoint().artifact_id().digest().clone()),
    }
}

pub(super) fn process() -> AttachedProcessEvidence {
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::WindowsOwnerPidProcessHandle,
        owner_pid: 42,
        process_instance_digest: Digest::sha256(b"process"),
        ownership_snapshot_digest: Digest::sha256(b"ownership"),
        entrypoint_object_digest: Digest::sha256(b"object"),
        entrypoint_digest: artifact("entrypoint").digest().clone(),
        entrypoint_bytes: 11,
        platform_evidence_digest: Digest::sha256(b"platform"),
    })
    .expect("process")
}

pub(super) fn connection(process: &AttachedProcessEvidence) -> RetainedTcpConnectionEvidence {
    RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
        attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
        sharing_limitation: TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable,
        process_evidence_digest: process.evidence_digest().clone(),
        platform_connection_digest: Digest::sha256(b"connection"),
    })
    .expect("connection")
}

pub(super) fn preflight(plan: &LocalOllamaBoundPreflightPlan) -> LocalOllamaPreflightReport {
    LocalOllamaPreflightReport {
        schema_version: 1,
        plan_id: plan.preflight.plan_id.clone(),
        plan_digest: Digest::sha256(&serde_json::to_vec(&plan.preflight).expect("plan")),
        mode: LocalOllamaPreflightMode::Verify,
        observed: OllamaPreflight {
            runtime: RuntimeIdentity {
                backend: "ollama_native".to_owned(),
                version: "0.16.2".to_owned(),
                digest: None,
            },
            inventory: vec![OllamaInventoryEntry {
                reference: "fixture:latest".to_owned(),
                inventory_digest: Digest::sha256(b"inventory"),
                byte_size: 1,
            }],
            bindings: vec![OllamaPreflightBinding {
                reference: "fixture:latest".to_owned(),
                inventory_digest: Digest::sha256(b"inventory"),
                details: details(),
            }],
            running: Vec::new(),
        },
        qualified: false,
    }
}

pub(super) fn native_load(
    package: &RuntimePackageManifest,
    process: &AttachedProcessEvidence,
) -> NativeLoadObservation {
    NativeLoadObservation::new(
        package,
        NativeLoadObservationInput {
            evidence_class: NativeLoadEvidenceClass::LinuxProcMapFiles,
            visibility_scope: NativeLoadVisibilityScope::FileBackedExecutableMappings,
            process_evidence_digest: process.evidence_digest().clone(),
            observation_contract_id: "linux-proc-map-files".to_owned(),
            observation_contract_schema_version: 1,
            components: vec![NativeLoadedComponent::new(
                package.entrypoint().artifact_id().clone(),
                package.entrypoint().byte_size(),
                NativeLoadOrigin::PackagedMember {
                    relative_path: package.entrypoint().relative_path().clone(),
                },
                NativeMappingClass::ExecutableImage,
                Digest::sha256(b"entrypoint object"),
            )],
        },
    )
    .expect("native load")
}
