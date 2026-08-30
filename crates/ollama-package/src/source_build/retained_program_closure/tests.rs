use std::{
    fs::File,
    io::{self, Cursor, Read},
    path::PathBuf,
};

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_types::Digest;
use serde_json::{Value, json};

use crate::source_build::{
    RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputLimits,
    RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputOpenError, RuntimeSourceBuildInputRole,
    VerifiedRuntimeSourceBuildInputs, program_lineage::profile::PRODUCTION_COMPONENTS,
};

use super::{
    RetainedProgramUpstreamClosureError, RetainedProgramUpstreamClosureId,
    VerifiedAlpineReleaseUpstream, VerifiedRetainedProgramUpstreamClosure,
    VerifiedRustReleaseUpstream, member, signature, verify_closure,
    verify_retained_program_upstream_closure,
};

const SMALL_PATH: &str = "source/small.bin";
const SMALL_ROLES: &[RuntimeSourceBuildInputRole] = &[RuntimeSourceBuildInputRole::OllamaSource];
const SMALL_SPEC: member::MemberSpec = member::MemberSpec {
    path: SMALL_PATH,
    name: "small source",
    revision: "fixture-1",
    locator: "https://example.invalid/small.bin",
    roles: SMALL_ROLES,
    size: 3,
    digest: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    retain_bytes: true,
};

fn production_manifest() -> RuntimeSourceBuildInputManifest {
    let components = PRODUCTION_COMPONENTS
        .iter()
        .map(|expected| {
            let (byte_size, digest) = expected.official.map_or_else(
                || {
                    (
                        u64::try_from(expected.path.len()).expect("fixture size"),
                        Digest::sha256(expected.path.as_bytes()),
                    )
                },
                |(size, digest)| {
                    (
                        size,
                        Digest::from_sha256_hex(digest).expect("official digest"),
                    )
                },
            );
            json!({
                "byte_size": byte_size,
                "digest": digest,
                "name": expected.name,
                "relative_path": expected.path,
                "revision": expected.revision,
                "roles": expected.roles,
                "source_locator": expected.locator
            })
        })
        .collect::<Vec<_>>();
    parse_manifest(&components)
}

fn parse_manifest(components: &[Value]) -> RuntimeSourceBuildInputManifest {
    let artifact_set = ArtifactSetManifest::new(
        components
            .iter()
            .map(|component| {
                ArtifactSetMember::new(
                    ArtifactId::from_digest(
                        serde_json::from_value(component["digest"].clone()).expect("digest"),
                    ),
                    component["byte_size"].as_u64().expect("byte size"),
                    ArtifactSetRelativePath::new(
                        component["relative_path"].as_str().expect("relative path"),
                    )
                    .expect("valid path"),
                )
            })
            .collect(),
    )
    .expect("artifact set");
    let manifest = json!({
        "artifact_set_id": artifact_set.artifact_set_id(),
        "components": components,
        "policy": {
            "accelerator": "cpu_only",
            "build_arguments": ["--build-runtime", "--cpu-only", "--offline"],
            "cpu_feature_policy": "x86-64-v2",
            "environment": [
                {"name":"CGO_ENABLED", "value":"1"},
                {"name":"GOAMD64", "value":"v2"},
                {"name":"GOARCH", "value":"amd64"},
                {"name":"GOOS", "value":"linux"},
                {"name":"GOPROXY", "value":"off"},
                {"name":"GOSUMDB", "value":"off"},
                {"name":"LC_ALL", "value":"C.UTF-8"},
                {"name":"SOURCE_DATE_EPOCH", "value":"1725000000"},
                {"name":"TZ", "value":"UTC"}
            ],
            "locale": "C.UTF-8",
            "network_access": "denied",
            "source_date_epoch": 1_725_000_000_u64,
            "target": {
                "abi": "linux_gnu_libc",
                "architecture": "x86_64",
                "operating_system": "linux"
            },
            "timezone": "UTC"
        },
        "schema_version": 1
    });
    RuntimeSourceBuildInputManifest::parse(
        &serde_json::to_vec(&manifest).expect("manifest bytes"),
        RuntimeSourceBuildInputLimits::default(),
    )
    .expect("production manifest")
}

fn small_manifest() -> RuntimeSourceBuildInputManifest {
    let mut manifest = production_manifest();
    manifest.components = vec![RuntimeSourceBuildInputComponent {
        relative_path: ArtifactSetRelativePath::new(SMALL_SPEC.path).expect("small path"),
        byte_size: SMALL_SPEC.size,
        digest: Digest::from_sha256_hex(SMALL_SPEC.digest).expect("small digest"),
        name: SMALL_SPEC.name.to_owned(),
        revision: SMALL_SPEC.revision.to_owned(),
        source_locator: SMALL_SPEC.locator.to_owned(),
        roles: SMALL_SPEC.roles.to_vec(),
    }];
    manifest
}

#[test]
fn exact_production_members_join_and_metadata_drift_fails() {
    let manifest = production_manifest();
    assert!(member::validate_manifest_members(&manifest, &member::RUST_MEMBERS).is_ok());
    assert!(member::validate_manifest_members(&manifest, &member::ALPINE_MEMBERS).is_ok());

    let mut components = PRODUCTION_COMPONENTS
        .iter()
        .map(|expected| {
            let (size, digest) = expected.official.map_or_else(
                || {
                    (
                        u64::try_from(expected.path.len()).expect("fixture size"),
                        Digest::sha256(expected.path.as_bytes()),
                    )
                },
                |(size, digest)| {
                    (
                        size,
                        Digest::from_sha256_hex(digest).expect("official digest"),
                    )
                },
            );
            json!({
                "byte_size":size,
                "digest":digest,
                "name":expected.name,
                "relative_path":expected.path,
                "revision":expected.revision,
                "roles":expected.roles,
                "source_locator":expected.locator
            })
        })
        .collect::<Vec<_>>();
    components
        .iter_mut()
        .find(|component| component["relative_path"] == member::RUST_CHANNEL)
        .expect("channel")["source_locator"] = json!("https://example.invalid/channel");
    let changed = parse_manifest(&components);
    assert_eq!(
        member::validate_manifest_members(&changed, &member::RUST_MEMBERS),
        Err(RetainedProgramUpstreamClosureError::ManifestMismatch)
    );
}

#[test]
fn reviewer_facts_are_exact_and_inert() {
    let rust_id = member::closure_id(b"test/rust", &member::RUST_MEMBERS);
    let alpine_id = member::closure_id(b"test/alpine", &member::ALPINE_MEMBERS);
    let closure = VerifiedRetainedProgramUpstreamClosure {
        closure_id: RetainedProgramUpstreamClosureId(Digest::sha256(b"complete")),
        source_input_set_id: production_manifest().artifact_set().artifact_set_id(),
        rust: VerifiedRustReleaseUpstream {
            closure_id: rust_id,
        },
        alpine: VerifiedAlpineReleaseUpstream {
            closure_id: alpine_id,
        },
    };
    assert_eq!(closure.rust().release(), "1.97.1");
    assert_eq!(closure.rust().channel_date(), "2026-07-16");
    assert_eq!(
        closure.rust().rust_commit(),
        "8bab26f4f68e0e26f0bb7960be334d5b520ea452"
    );
    assert_eq!(
        closure.rust().cargo_version(),
        "0.98.0 (c980f4866 2026-06-30)"
    );
    assert_eq!(closure.rust().cargo_commit(), closure.rust().rust_commit());
    assert_eq!(closure.rust().target(), "x86_64-unknown-linux-musl");
    assert_eq!(
        closure.rust().release_key_fingerprint(),
        signature::RUST_FINGERPRINT
    );
    assert_eq!(closure.rust().release_key_id(), signature::RUST_KEY_ID);
    assert_eq!(
        closure.rust().signature_creation_time(),
        signature::RUST_SIGNATURE_CREATION
    );
    assert!(closure.rust().host_and_target_std_are_one_member());
    assert_eq!(closure.alpine().release(), "3.23.3");
    assert_eq!(
        closure.alpine().release_key_fingerprint(),
        signature::ALPINE_FINGERPRINT
    );
    assert_eq!(closure.alpine().release_key_id(), signature::ALPINE_KEY_ID);
    assert_eq!(
        closure.alpine().signature_creation_time(),
        signature::ALPINE_SIGNATURE_CREATION
    );
    assert_eq!(closure.alpine().libgcc_package_name(), "libgcc");
    assert_eq!(closure.alpine().libgcc_package_version(), "15.2.0-r2");
    assert_eq!(closure.alpine().busybox_package_version(), "1.37.0-r30");
    assert_eq!(
        closure.alpine().busybox_executable_digest(),
        "82bbbabec12a985ae58810cfe975c3399264dc888aa592d8e460732bdd30a8dd"
    );
    assert!(closure.alpine().embedded_apk_signature_verified());
    assert_ne!(closure.rust().closure_id(), closure.alpine().closure_id());
    assert_ne!(closure.closure_id().digest(), closure.rust().closure_id());
    assert_eq!(
        closure.source_input_set_id(),
        &production_manifest().artifact_set().artifact_set_id()
    );
}

#[test]
fn member_streams_are_remeasured_retained_and_fail_closed() {
    let manifest = small_manifest();
    let specs = [SMALL_SPEC];
    let mut open = |_path: &ArtifactSetRelativePath| Ok(Cursor::new(b"abc".to_vec()));
    let mut active = || false;
    let verified = member::read_and_verify_members(&manifest, &specs, &mut open, &mut active)
        .expect("exact member bytes");
    assert_eq!(verified.required(SMALL_PATH), Ok(b"abc".as_slice()));
    assert_eq!(
        verified.required("source/unretained.bin"),
        Err(RetainedProgramUpstreamClosureError::ManifestMismatch)
    );

    for changed in [b"ab".as_slice(), b"abd".as_slice(), b"abcd".as_slice()] {
        let mut open = |_path: &ArtifactSetRelativePath| Ok(Cursor::new(changed.to_vec()));
        assert!(matches!(
            member::read_and_verify_members(&manifest, &specs, &mut open, &mut active),
            Err(RetainedProgramUpstreamClosureError::ComponentMeasurementMismatch)
        ));
    }

    let mut unavailable = |_path: &ArtifactSetRelativePath| {
        Err::<Cursor<Vec<u8>>, _>(RuntimeSourceBuildInputOpenError)
    };
    assert!(matches!(
        member::read_and_verify_members(&manifest, &specs, &mut unavailable, &mut active),
        Err(RetainedProgramUpstreamClosureError::ComponentUnavailable)
    ));

    let mut open_error = |_path: &ArtifactSetRelativePath| Ok(ReadError);
    assert!(matches!(
        member::read_and_verify_members(&manifest, &specs, &mut open_error, &mut active),
        Err(RetainedProgramUpstreamClosureError::ComponentRead)
    ));

    let mut open = |_path: &ArtifactSetRelativePath| Ok(Cursor::new(b"abc".to_vec()));
    let mut cancelled = || true;
    assert!(matches!(
        member::read_and_verify_members(&manifest, &specs, &mut open, &mut cancelled),
        Err(RetainedProgramUpstreamClosureError::Cancelled)
    ));

    let unretained = member::MemberSpec {
        retain_bytes: false,
        ..SMALL_SPEC
    };
    let mut open = |_path: &ArtifactSetRelativePath| Ok(Cursor::new(b"abc".to_vec()));
    let verified =
        member::read_and_verify_members(&manifest, &[unretained], &mut open, &mut active)
            .expect("streamed member");
    assert_eq!(
        verified.required(SMALL_PATH),
        Err(RetainedProgramUpstreamClosureError::ManifestMismatch)
    );

    let empty_roles = member::MemberSpec {
        roles: &[],
        ..SMALL_SPEC
    };
    assert_eq!(
        member::validate_manifest_members(&manifest, &[empty_roles]),
        Err(RetainedProgramUpstreamClosureError::ManifestMismatch)
    );
    let mut duplicated = small_manifest();
    duplicated.components.push(duplicated.components[0].clone());
    assert_eq!(
        member::validate_manifest_members(&duplicated, &specs),
        Err(RetainedProgramUpstreamClosureError::ManifestMismatch)
    );
}

#[test]
fn upstream_entry_points_reject_missing_lineage_unavailability_and_cancellation() {
    let manifest = small_manifest();
    let inputs = VerifiedRuntimeSourceBuildInputs {
        manifest,
        program_lineage: None,
    };
    assert_eq!(
        verify_retained_program_upstream_closure::<Cursor<Vec<u8>>, _, _>(
            &inputs,
            |_path| Err(RuntimeSourceBuildInputOpenError),
            || false,
        ),
        Err(RetainedProgramUpstreamClosureError::MissingProgramLineage)
    );

    let production = production_manifest();
    assert_eq!(
        verify_closure::<Cursor<Vec<u8>>, _, _>(
            &production,
            |_path| Err(RuntimeSourceBuildInputOpenError),
            || false,
        ),
        Err(RetainedProgramUpstreamClosureError::ComponentUnavailable)
    );
    assert_eq!(
        verify_closure::<Cursor<Vec<u8>>, _, _>(
            &production,
            |_path| Ok(Cursor::new(Vec::new())),
            || true,
        ),
        Err(RetainedProgramUpstreamClosureError::Cancelled)
    );
}

struct ReadError;

impl Read for ReadError {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("synthetic read failure"))
    }
}

#[test]
#[ignore = "requires durable official files outside the repository"]
fn authenticates_real_official_rust_and_alpine_bytes() {
    let manifest = production_manifest();
    let result = verify_closure::<File, _, _>(
        &manifest,
        |relative_path| {
            durable_path(relative_path.as_str())
                .and_then(|path| File::open(path).ok())
                .ok_or(RuntimeSourceBuildInputOpenError)
        },
        || false,
    )
    .expect("official upstream closure");
    assert_eq!(result.rust().release(), "1.97.1");
    assert_eq!(result.alpine().release(), "3.23.3");
    assert_eq!(
        result.source_input_set_id(),
        &manifest.artifact_set().artifact_set_id()
    );

    let rust_root = read(member::RUST_TRUST_ROOT);
    let rust_signature = read(member::RUST_CHANNEL_SIGNATURE);
    let channel = read(member::RUST_CHANNEL);
    signature::verify_detached_signature(
        &rust_root,
        &rust_signature,
        &channel,
        signature::SignatureExpectation::rust(),
    )
    .expect("real Rust signature");
    let mut changed_channel = channel;
    changed_channel[0] ^= 1;
    assert_eq!(
        signature::verify_detached_signature(
            &rust_root,
            &rust_signature,
            &changed_channel,
            signature::SignatureExpectation::rust(),
        ),
        Err(RetainedProgramUpstreamClosureError::InvalidSignature)
    );

    let alpine_root = read(member::ALPINE_TRUST_ROOT);
    let alpine_signature = read(member::ALPINE_SIGNATURE);
    let alpine_payload = read(member::ALPINE_MINIROOTFS);
    signature::verify_detached_signature(
        &alpine_root,
        &alpine_signature,
        &alpine_payload,
        signature::SignatureExpectation::alpine(),
    )
    .expect("real Alpine signature");
}

fn read(relative_path: &str) -> Vec<u8> {
    std::fs::read(durable_path(relative_path).expect("known durable member"))
        .expect("durable official bytes")
}

fn durable_path(relative_path: &str) -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("RETONR_RETAINED_LINEAGE_ROOT").expect(
        "set RETONR_RETAINED_LINEAGE_ROOT to the retained-lineage directory before running the ignored golden test",
    ));
    let suffix = match relative_path {
        "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz" => {
            "downloads/rust-1.97.1-musl-host/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz"
        }
        "lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz" => {
            "downloads/rust-1.97.1-musl-host/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz"
        }
        "lineage/rust/channel-rust-1.97.1.toml" => "downloads/rust-1.97.1/channel-rust-1.97.1.toml",
        "lineage/rust/channel-rust-1.97.1.toml.asc" => {
            "downloads/rust-1.97.1/channel-rust-1.97.1.toml.asc"
        }
        "lineage/rust/channel-rust-1.97.1.toml.sha256" => {
            "downloads/rust-1.97.1/channel-rust-1.97.1.toml.sha256"
        }
        "lineage/rust/rust-key.gpg.ascii" => "downloads/rust-1.97.1/rust-key.gpg.ascii",
        "lineage/rust/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz" => {
            "downloads/rust-1.97.1/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz"
        }
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz" => {
            "downloads/alpine-3.23.3/alpine-minirootfs-3.23.3-x86_64.tar.gz"
        }
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc" => {
            "downloads/alpine-3.23.3/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc"
        }
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256" => {
            "downloads/alpine-3.23.3/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256"
        }
        "lineage/host/alpine/libgcc-15.2.0-r2.apk" => {
            "downloads/alpine-3.23.3/libgcc-15.2.0-r2.apk"
        }
        "lineage/host/alpine/busybox-static-1.37.0-r30.apk" => {
            "downloads/alpine-3.23.3/busybox-static-1.37.0-r30.apk"
        }
        "toolchains/busybox" => "downloads/alpine-3.23.3/busybox.static",
        "lineage/host/alpine/ncopa.asc" => "downloads/alpine-3.23.3/ncopa.asc",
        _ => return None,
    };
    Some(root.join(suffix))
}
