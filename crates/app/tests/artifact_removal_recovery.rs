use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use rewrite_app::{
    ArtifactRemovalDisposition, ArtifactRemovalLimits, ArtifactRemovalRequest,
    ArtifactRemovalService,
};
use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, InstalledArtifact, LicenseRecord,
};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};

const CHILD_MODE: &str = "RETONR_REMOVAL_CRASH_MODE";
const CHILD_ROOT: &str = "RETONR_REMOVAL_CRASH_ROOT";
const CHILD_STATE: &str = "RETONR_REMOVAL_CRASH_STATE";
const CHILD_DIGEST: &str = "RETONR_REMOVAL_CRASH_DIGEST";

#[test]
fn abrupt_process_exit_after_preparation_or_unlink_is_recoverable() {
    for mode in ["prepared", "unlinked"] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("managed");
        let state_path = directory.path().join("state.sqlite3");
        let bytes = b"artifact";
        let (digest, request) = initialize(&root, &state_path, bytes);
        let output = run_child(mode, &root, &state_path, digest.as_str());
        assert_eq!(
            output.status.code(),
            Some(86),
            "child did not stop at the requested boundary\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let mut store = ArtifactStateStore::open(&state_path).expect("reopen durable state");
        let result = ArtifactRemovalService::open_existing(&root, &mut store, limits())
            .expect("open recovery")
            .remove(&request, &CancellationToken::new(), |_| {})
            .expect("finish removal after abrupt process exit");
        assert_eq!(result.disposition, ArtifactRemovalDisposition::Recovered);
        assert!(!root.join("artifacts").join(digest.as_str()).exists());
    }
}

#[test]
#[ignore = "invoked in an isolated child process by the recovery parent"]
fn removal_crash_child() {
    let Ok(mode) = std::env::var(CHILD_MODE) else {
        return;
    };
    let root = PathBuf::from(std::env::var_os(CHILD_ROOT).expect("child storage root"));
    let state_path = PathBuf::from(std::env::var_os(CHILD_STATE).expect("child state path"));
    let digest = Digest::from_sha256_hex(std::env::var(CHILD_DIGEST).expect("child digest"))
        .expect("valid child digest");
    let artifact_id = ArtifactId::from_digest(digest.clone());
    let mut store = ArtifactStateStore::open(&state_path).expect("open child state");
    let (selection, removal) = store
        .artifact_removal_state(&artifact_id)
        .expect("load child selection");
    assert!(removal.is_none());
    let selection = selection.expect("installed child selection");
    store
        .prepare_artifact_removal(&selection)
        .expect("durably prepare child removal");
    if mode == "unlinked" {
        fs::remove_file(root.join("artifacts").join(digest.as_str()))
            .expect("unlink child artifact");
    } else {
        assert_eq!(mode, "prepared");
    }
    std::process::exit(86);
}

fn run_child(mode: &str, root: &Path, state_path: &Path, digest: &str) -> Output {
    let mut child = Command::new(std::env::current_exe().expect("locate integration test binary"))
        .args(["--exact", "removal_crash_child", "--ignored", "--nocapture"])
        .env(CHILD_MODE, mode)
        .env(CHILD_ROOT, root)
        .env(CHILD_STATE, state_path)
        .env(CHILD_DIGEST, digest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start removal crash child");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child
            .try_wait()
            .expect("poll removal crash child")
            .is_some()
        {
            return child
                .wait_with_output()
                .expect("collect removal crash child");
        }
        if Instant::now() >= deadline {
            child.kill().expect("terminate removal crash child");
            let output = child.wait_with_output().expect("collect timed out child");
            panic!(
                "removal crash child timed out\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn initialize(root: &Path, state_path: &Path, bytes: &[u8]) -> (Digest, ArtifactRemovalRequest) {
    fs::create_dir(root).expect("create root");
    fs::create_dir(root.join("artifacts")).expect("create artifacts");
    fs::write(root.join(".artifact-import.lock"), []).expect("create lifecycle lock");
    let digest = Digest::sha256(bytes);
    let artifact_id = ArtifactId::from_digest(digest.clone());
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: artifact_id.clone(),
        source: ArtifactSource {
            origin: "fixture/model".to_owned(),
            revision: "fixture-revision".to_owned(),
        },
        artifact_digest: digest.clone(),
        byte_size: u64::try_from(bytes.len()).expect("fixture size"),
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        architecture: Some("transformer".to_owned()),
        quantization: Some("q4".to_owned()),
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "weights".to_owned(),
            identifier: "Apache-2.0".to_owned(),
            text_digest: Digest::sha256(b"license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: Some(8_192),
        },
    };
    let installed = InstalledArtifact {
        artifact_id,
        artifact_digest: digest.clone(),
        byte_size: manifest.byte_size,
        storage_key: format!("artifacts/{}", digest.as_str()),
    };
    fs::write(root.join("artifacts").join(digest.as_str()), bytes).expect("write artifact");
    let mut store = ArtifactStateStore::open(state_path).expect("open state");
    let selection = store
        .put_installation(&manifest, &installed)
        .expect("register installation")
        .installation;
    (digest, ArtifactRemovalRequest { selection })
}

const fn limits() -> ArtifactRemovalLimits {
    ArtifactRemovalLimits {
        maximum_artifact_bytes: 1024,
        maximum_storage_entries: 8,
    }
}
