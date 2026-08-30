use std::{fs, path::PathBuf};

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, GenerationCaseManifestV1, InstalledArtifact, LicenseRecord,
};
use rewrite_model_store::{ArtifactStateStore, StoredArtifactInstallation};
use rewrite_types::CancellationToken;
use tempfile::TempDir;

use super::super::GenerationCaseSourceLease;
use super::{common_limits, digest};
use crate::GenerationCaseSourceLeaseLimits;

pub(crate) struct SourceFixture {
    _directory: TempDir,
    root: PathBuf,
    #[cfg_attr(
        all(feature = "test-support", not(test)),
        expect(dead_code, reason = "used by app-only source drift tests")
    )]
    pub(crate) database: PathBuf,
    store: ArtifactStateStore,
    selection: StoredArtifactInstallation,
    case: GenerationCaseManifestV1,
}

impl SourceFixture {
    pub(crate) fn acquire(&self) -> GenerationCaseSourceLease<'_> {
        GenerationCaseSourceLease::acquire(
            &self.root,
            &self.store,
            self.selection.clone(),
            &self.case,
            GenerationCaseSourceLeaseLimits {
                maximum_source_bytes: common_limits().maximum_source_bytes(),
                maximum_storage_entries: 8,
            },
            &CancellationToken::new(),
        )
        .expect("source lease")
    }
}

pub(crate) fn source_fixture(source: &[u8], case: &GenerationCaseManifestV1) -> SourceFixture {
    let directory = tempfile::tempdir().expect("source directory");
    let root = directory.path().join("managed");
    fs::create_dir(&root).expect("managed root");
    fs::create_dir(root.join("artifacts")).expect("artifacts directory");
    fs::write(root.join(".artifact-import.lock"), []).expect("lifecycle lock");
    fs::write(
        root.join("artifacts").join(case.source_digest().as_str()),
        source,
    )
    .expect("source bytes");
    let database = directory.path().join("state.sqlite3");
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: case.source_artifact_id().clone(),
        source: ArtifactSource {
            origin: "fixture/request-builder".to_owned(),
            revision: "v1".to_owned(),
        },
        artifact_digest: case.source_digest().clone(),
        byte_size: case.source_byte_count(),
        format: "utf8".to_owned(),
        family: "qualification-source".to_owned(),
        architecture: None,
        quantization: None,
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "source".to_owned(),
            identifier: "CC0-1.0".to_owned(),
            text_digest: digest("source license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: None,
        },
    };
    let installed = InstalledArtifact {
        artifact_id: case.source_artifact_id().clone(),
        artifact_digest: case.source_digest().clone(),
        byte_size: case.source_byte_count(),
        storage_key: format!("artifacts/{}", case.source_digest().as_str()),
    };
    let mut store = ArtifactStateStore::open(&database).expect("source state");
    let selection = store
        .put_installation(&manifest, &installed)
        .expect("source installation")
        .installation;
    SourceFixture {
        _directory: directory,
        root,
        database,
        store,
        selection,
        case: case.clone(),
    }
}
