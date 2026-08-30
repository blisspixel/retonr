use std::{fs, io::Cursor, path::Path};

use rewrite_model::{ArtifactSetManifest, ModelPackageManifest};
use rewrite_ollama_package::{
    BlobOpenError, CONFIG_MEDIA_TYPE, LICENSE_MEDIA_TYPE, MANIFEST_MEDIA_TYPE, MODEL_MEDIA_TYPE,
    PARAMS_MEDIA_TYPE, ReconstructionLimits, TEMPLATE_MEDIA_TYPE,
    reconstruct_model_package_with_limits,
};
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use tempfile::TempDir;

use crate::{
    ArtifactRepository, ArtifactSetImportLimits, ArtifactSetInstallationKey,
    ArtifactSetRemovalLimits, ModelPackageLeaseLimits, OfflineArtifactSetImportRequest,
    PackageAttestationService, RuntimeArtifactSetLease, RuntimeArtifactSetLeaseLimits,
    VerifiedManagedOllamaModelPackageLease,
};

const MODEL_LIMITS: ModelPackageLeaseLimits = ModelPackageLeaseLimits {
    maximum_members: 8,
    maximum_member_bytes: 16 * 1_024,
    maximum_bytes: 32 * 1_024,
};

const SET_LIMITS: RuntimeArtifactSetLeaseLimits = RuntimeArtifactSetLeaseLimits {
    maximum_members: 8,
    maximum_member_bytes: 16 * 1_024,
    maximum_total_bytes: 32 * 1_024,
    maximum_tree_entries: 16,
    maximum_storage_entries: 8,
};

const IMPORT_LIMITS: ArtifactSetImportLimits = ArtifactSetImportLimits {
    maximum_members: 8,
    maximum_member_bytes: 16 * 1_024,
    maximum_total_bytes: 32 * 1_024,
    maximum_tree_entries: 16,
    maximum_storage_entries: 8,
    maximum_staging_entries: 8,
};

const REMOVAL_LIMITS: ArtifactSetRemovalLimits = ArtifactSetRemovalLimits {
    maximum_members: 8,
    maximum_member_bytes: 16 * 1_024,
    maximum_total_bytes: 32 * 1_024,
    maximum_tree_entries: 16,
    maximum_storage_entries: 8,
};

pub(super) struct Fixture {
    pub(super) directory: TempDir,
    pub(super) repository: ArtifactRepository,
    pub(super) set: ArtifactSetManifest,
    pub(super) package: ModelPackageManifest,
    pub(super) key: ArtifactSetInstallationKey,
    files: Vec<(String, Vec<u8>)>,
}

impl Fixture {
    pub(super) fn new(shared_license_and_template: bool) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let repository =
            ArtifactRepository::new(directory.path().join("data")).expect("repository");
        let config = br#"{"model_format":"gguf","model_family":"fixture","model_families":["fixture"],"model_type":"fixture","file_type":"fixture","architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}"#.to_vec();
        let model = minimal_gguf();
        let template = b"{{ .Prompt }}".to_vec();
        let license = if shared_license_and_template {
            template.clone()
        } else {
            b"fixture model license".to_vec()
        };
        let parameters = br#"{"temperature":0}"#.to_vec();
        let raw_manifest = serde_json::to_vec(&json!({
            "schemaVersion": 2,
            "mediaType": MANIFEST_MEDIA_TYPE,
            "config": descriptor(CONFIG_MEDIA_TYPE, &config),
            "layers": [
                descriptor(MODEL_MEDIA_TYPE, &model),
                descriptor(TEMPLATE_MEDIA_TYPE, &template),
                descriptor(LICENSE_MEDIA_TYPE, &license),
                descriptor(PARAMS_MEDIA_TYPE, &parameters),
            ]
        }))
        .expect("manifest JSON");
        let blobs = vec![
            (Digest::sha256(&config), config.clone()),
            (Digest::sha256(&model), model.clone()),
            (Digest::sha256(&template), template.clone()),
            (Digest::sha256(&license), license.clone()),
            (Digest::sha256(&parameters), parameters.clone()),
        ];
        let reconstructed = reconstruct_model_package_with_limits(
            &raw_manifest,
            "registry.ollama.ai/library/fixture",
            &ReconstructionLimits::default(),
            |digest| open_blob(&blobs, digest),
            || false,
        )
        .expect("reconstruct exact fixture");
        let files = vec![
            ("config/ollama-config.json".to_owned(), config),
            ("config/parameters.json".to_owned(), parameters),
            ("legal/license.txt".to_owned(), license),
            ("model/model.gguf".to_owned(), model),
            ("prompts/template.go.tmpl".to_owned(), template),
            (
                "provenance/ollama-manifest-v2.json".to_owned(),
                raw_manifest,
            ),
        ];
        let set = reconstructed.artifact_set().clone();
        let package = reconstructed.model_package().clone();
        let key = install(
            &repository,
            directory.path(),
            "first-installation",
            &set,
            &files,
        );
        Self {
            directory,
            repository,
            set,
            package,
            key,
            files,
        }
    }

    pub(super) fn verify(&self) -> VerifiedManagedOllamaModelPackageLease {
        PackageAttestationService::verify_managed_ollama_model_package(
            self.lease(),
            &self.package,
            MODEL_LIMITS,
            &ReconstructionLimits::default(),
            &CancellationToken::new(),
        )
        .expect("verify exact managed Ollama package")
    }

    pub(super) fn advance_generation(&mut self) {
        self.repository
            .remove_set(&self.key, REMOVAL_LIMITS, &CancellationToken::new())
            .expect("remove inactive installation");
        self.key = install(
            &self.repository,
            self.directory.path(),
            "second-installation",
            &self.set,
            &self.files,
        );
    }

    pub(super) fn add_unexpected_member(&self) {
        let root = self
            .directory
            .path()
            .join("data")
            .join("artifact-storage")
            .join("sets")
            .join(format!(
                "set-v1-{}",
                self.set.artifact_set_id().digest().as_str()
            ));
        fs::write(root.join("unexpected-member.bin"), b"unexpected")
            .expect("add unexpected managed member");
    }

    fn lease(&self) -> RuntimeArtifactSetLease {
        self.repository
            .lease_set(
                self.key.artifact_set_id(),
                SET_LIMITS,
                &CancellationToken::new(),
            )
            .expect("lease exact installed generation")
    }
}

fn install(
    repository: &ArtifactRepository,
    directory: &Path,
    label: &str,
    set: &ArtifactSetManifest,
    files: &[(String, Vec<u8>)],
) -> ArtifactSetInstallationKey {
    let source = directory.join(label);
    for (path, bytes) in files {
        let target = source.join(path);
        fs::create_dir_all(target.parent().expect("member parent")).expect("source directory");
        fs::write(target, bytes).expect("source member");
    }
    repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest: set.clone(),
            },
            IMPORT_LIMITS,
            &CancellationToken::new(),
        )
        .expect("import exact model set")
        .key
}

fn open_blob(
    blobs: &[(Digest, Vec<u8>)],
    digest: &Digest,
) -> Result<Cursor<Vec<u8>>, BlobOpenError> {
    blobs
        .iter()
        .find(|(candidate, _bytes)| candidate == digest)
        .map(|(_digest, bytes)| Cursor::new(bytes.clone()))
        .ok_or(BlobOpenError)
}

fn descriptor(media_type: &str, bytes: &[u8]) -> serde_json::Value {
    json!({
        "mediaType": media_type,
        "digest": format!("sha256:{}", Digest::sha256(bytes).as_str()),
        "size": bytes.len(),
    })
}

fn minimal_gguf() -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"GGUF");
    output.extend_from_slice(&3_u32.to_le_bytes());
    output.extend_from_slice(&1_u64.to_le_bytes());
    output.extend_from_slice(&6_u64.to_le_bytes());
    push_string_metadata(&mut output, "general.architecture", "llama");
    push_u32_metadata(&mut output, "general.file_type", 1);
    push_u32_metadata(&mut output, "general.quantization_version", 2);
    push_u64_metadata(&mut output, "general.parameter_count", 1);
    push_string_metadata(&mut output, "tokenizer.model", "fixture");
    push_string_metadata(&mut output, "tokenizer.chat_template", "{{ .Prompt }}");
    push_text(&mut output, "tensor");
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.extend_from_slice(&1_u64.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&0_u64.to_le_bytes());
    while output.len() % 32 != 0 {
        output.push(0);
    }
    output.push(0);
    output
}

fn push_string_metadata(output: &mut Vec<u8>, key: &str, value: &str) {
    push_text(output, key);
    output.extend_from_slice(&8_u32.to_le_bytes());
    push_text(output, value);
}

fn push_u32_metadata(output: &mut Vec<u8>, key: &str, value: u32) {
    push_text(output, key);
    output.extend_from_slice(&4_u32.to_le_bytes());
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64_metadata(output: &mut Vec<u8>, key: &str, value: u64) {
    push_text(output, key);
    output.extend_from_slice(&10_u32.to_le_bytes());
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}
