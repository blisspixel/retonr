//! Real retained-source authority over an exact synthetic Prepared foundation.

use std::{fs, path::PathBuf};

use rewrite_app::{GenerationCaseSourceLease, GenerationCaseSourceLeaseLimits};
use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, GenerationCaseManifestV1, GenerationDeterministicCaseContractV1,
    GenerationSuiteManifestV1, InstalledArtifact, LicenseRecord,
};
use rewrite_model_store::{
    ArtifactStateStore, GenerationQualificationPlanFoundationV1Input, StoredArtifactInstallation,
};
use rewrite_types::{CancellationToken, Digest};

use crate::{GenerationCaseMaterialLimits, VerifiedGenerationCaseMaterial};

pub(crate) struct MaterialFixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    store: ArtifactStateStore,
    selection: StoredArtifactInstallation,
    case: GenerationCaseManifestV1,
    contract: GenerationDeterministicCaseContractV1,
    suite: GenerationSuiteManifestV1,
}

impl MaterialFixture {
    pub(crate) fn new(
        foundation: GenerationQualificationPlanFoundationV1Input<'_>,
        source: &[u8],
    ) -> Self {
        assert_eq!(foundation.cases.len(), 1, "single exact synthetic case");
        let case = foundation.cases[0].clone();
        assert_eq!(case.source_digest(), &Digest::sha256(source));
        let directory = tempfile::tempdir().expect("material fixture directory");
        let root = directory.path().join("managed");
        fs::create_dir(&root).expect("managed root");
        fs::create_dir(root.join("artifacts")).expect("artifact directory");
        fs::write(root.join(".artifact-import.lock"), []).expect("lifecycle lock");
        fs::write(
            root.join("artifacts").join(case.source_digest().as_str()),
            source,
        )
        .expect("exact source bytes");
        let mut store =
            ArtifactStateStore::open(&directory.path().join("state.db")).expect("material store");
        let manifest = ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            artifact_id: case.source_artifact_id().clone(),
            artifact_digest: case.source_digest().clone(),
            source: ArtifactSource {
                origin: "fixture/deterministic-settlement".to_owned(),
                revision: "v1".to_owned(),
            },
            byte_size: case.source_byte_count(),
            format: "utf8".to_owned(),
            family: "qualification-source".to_owned(),
            architecture: None,
            quantization: None,
            tokenizer: None,
            licenses: vec![LicenseRecord {
                component: "synthetic source".to_owned(),
                identifier: "CC0-1.0".to_owned(),
                text_digest: Digest::sha256(b"synthetic license"),
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
        let selection = store
            .put_installation(&manifest, &installed)
            .expect("material installation")
            .installation;
        Self {
            directory,
            root,
            store,
            selection,
            case,
            contract: foundation.deterministic_case_contracts[0].clone(),
            suite: foundation.suite.clone(),
        }
    }

    pub(crate) fn verify(&self) -> VerifiedGenerationCaseMaterial<'_> {
        let lease = GenerationCaseSourceLease::acquire(
            &self.root,
            &self.store,
            self.selection.clone(),
            &self.case,
            GenerationCaseSourceLeaseLimits {
                maximum_source_bytes: 4 * 1024 * 1024,
                maximum_storage_entries: 16,
            },
            &CancellationToken::new(),
        )
        .expect("real retained source lease");
        VerifiedGenerationCaseMaterial::verify(
            self.suite.clone(),
            vec![self.case.clone()],
            vec![self.contract.clone()],
            vec![lease],
            GenerationCaseMaterialLimits {
                maximum_cases: 1,
                maximum_total_source_bytes: 4 * 1024 * 1024,
            },
            &CancellationToken::new(),
        )
        .expect("exact retained material authority")
    }

    pub(crate) fn invalidate_source_installation(&self) {
        let connection = rusqlite::Connection::open(self.directory.path().join("state.db"))
            .expect("synthetic source state");
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("retain foreign-key enforcement");
        connection
            .execute(
                "DELETE FROM installed_artifacts WHERE artifact_id = ?1",
                [self.case.source_artifact_id().digest().as_str()],
            )
            .expect("synthetic source installation drift");
    }
}
