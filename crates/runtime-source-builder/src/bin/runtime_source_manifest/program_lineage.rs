use std::path::Path;

use rewrite_types::Digest;
use serde_json::{Value, json};

use super::manifest_support::{Measurement, measure_file, measurement_value};

const RUST_CHANNEL_MANIFEST_DIGEST: &str =
    "03569b1886ceb5c05276b50c8431ab111de944cd6140fe1fa7d821dd8e0f29cf";
const RUST_RELEASE_KEY_FINGERPRINT: &str = "108F66205EAEB0AAA8DD5E1C85AB96E6FA1BE5FE";
const ALPINE_RELEASE_KEY_FINGERPRINT: &str = "0482D84022F52DF1C4E7CD43293ACD0907D9495A";

struct RustDistributionComponent {
    name: &'static str,
    target: &'static str,
    relative_path: &'static str,
    byte_size: u64,
    digest: &'static str,
    url: &'static str,
}

pub(super) fn compile(root: &Path) -> Result<Value, ()> {
    let distribution_components = RUST_DISTRIBUTION_COMPONENTS
        .iter()
        .map(|component| distribution_component(root, component))
        .collect::<Result<Vec<_>, _>>()?;
    let channel_manifest = measure_expected(
        root,
        "lineage/rust/channel-rust-1.97.1.toml",
        845_916,
        RUST_CHANNEL_MANIFEST_DIGEST,
    )?;
    let channel_checksum = measure_expected(
        root,
        "lineage/rust/channel-rust-1.97.1.toml.sha256",
        91,
        "3cd57815146650e28e1756549952f655c683a6576a689fbbc7a5ee9d1c6ffab2",
    )?;
    let channel_signature = measure_expected(
        root,
        "lineage/rust/channel-rust-1.97.1.toml.asc",
        801,
        "14553bf89b963f1d1f0a92413b91510ed43f8d50c68fe665763747d815022017",
    )?;
    let release_key = measure_expected(
        root,
        "lineage/rust/rust-key.gpg.ascii",
        5_326,
        "e54b09a439647e006b4831eec9785cbaaf3e07ab371c3a6ee6a68e1bdb9fbc6b",
    )?;
    let build_host = build_host(root)?;
    compile_value(
        root,
        &build_host,
        &distribution_components,
        &channel_manifest,
        &channel_checksum,
        &channel_signature,
        &release_key,
    )
}

fn build_host(root: &Path) -> Result<Value, ()> {
    let minirootfs = measure_expected(
        root,
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz",
        3_713_234,
        "42d0e6d8de5521e7bf92e075e032b5690c1d948fa9775efa32a51a38b25460fb",
    )?;
    let checksum = measure_expected(
        root,
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256",
        105,
        "2eca4849bc42f82f2616b7d6c1ee38fa73a2d143acf0f6fb2092b068350b931f",
    )?;
    let signature = measure_expected(
        root,
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc",
        833,
        "7a4b882fa8cfb3b59d5346d14b02f0b11641bf2703f94b6a01bb806fe2b98405",
    )?;
    let release_key = measure_expected(
        root,
        "lineage/host/alpine/ncopa.asc",
        3_092,
        "75a9a7e0cc35bfa946ce40c26133b3ed29a204fbd98a3b33331b659d927b3027",
    )?;
    let libgcc = measure_expected(
        root,
        "lineage/host/alpine/libgcc-15.2.0-r2.apk",
        80_358,
        "4279d14cbf43311312d3a7d43619f4c15ede2e46d5608ba421ed157ded57c700",
    )?;
    let busybox_static = measure_expected(
        root,
        "lineage/host/alpine/busybox-static-1.37.0-r30.apk",
        640_317,
        "bb30365b954f531938a33de18173342185844828863121d7e821115d2f7f29c9",
    )?;
    let busybox_executable = measure_expected(
        root,
        "toolchains/busybox",
        1_034_600,
        "82bbbabec12a985ae58810cfe975c3399264dc888aa592d8e460732bdd30a8dd",
    )?;
    Ok(build_host_value(
        &minirootfs,
        &checksum,
        &signature,
        &release_key,
        &libgcc,
        &busybox_static,
        &busybox_executable,
    ))
}

fn build_host_value(
    minirootfs: &Measurement,
    checksum: &Measurement,
    signature: &Measurement,
    release_key: &Measurement,
    libgcc: &Measurement,
    busybox_static: &Measurement,
    busybox_executable: &Measurement,
) -> Value {
    json!({
        "architecture": "x86_64",
        "busybox_executable": member_value("toolchains/busybox", busybox_executable),
        "busybox_static": {
            "package": member_value(
                "lineage/host/alpine/busybox-static-1.37.0-r30.apk",
                busybox_static
            ),
            "package_name": "busybox-static",
            "signature_disposition": "review_required",
            "version": "1.37.0-r30"
        },
        "distribution": "alpine",
        "libgcc": {
            "package": member_value("lineage/host/alpine/libgcc-15.2.0-r2.apk", libgcc),
            "package_name": "libgcc",
            "signature_disposition": "review_required",
            "version": "15.2.0-r2"
        },
        "minirootfs": member_value(
            "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz",
            minirootfs
        ),
        "minirootfs_checksum": member_value(
            "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256",
            checksum
        ),
        "minirootfs_signature": member_value(
            "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc",
            signature
        ),
        "minirootfs_signature_disposition": "review_required",
        "release_key": member_value("lineage/host/alpine/ncopa.asc", release_key),
        "release_key_fingerprint": ALPINE_RELEASE_KEY_FINGERPRINT,
        "schema_version": 1,
        "version": "3.23.3"
    })
}

fn compile_value(
    root: &Path,
    build_host: &Value,
    distribution_components: &[Value],
    channel_manifest: &Measurement,
    channel_checksum: &Measurement,
    channel_signature: &Measurement,
    release_key: &Measurement,
) -> Result<Value, ()> {
    Ok(json!({
        "authority": "none",
        "build_host": build_host,
        "build_programs": [
            {
                "name": "isolation_helper",
                "payload": member_value(
                    "helper/isolation",
                    &measure_file(&root.join("helper/isolation"))?
                )
            },
            {
                "name": "source_builder",
                "payload": member_value(
                    "scripts/build",
                    &measure_file(&root.join("scripts/build"))?
                )
            }
        ],
        "build_recipe": member_value(
            "lineage/build-recipe-v2.json",
            &measure_file(&root.join("lineage/build-recipe-v2.json"))?
        ),
        "cargo_lock": member_value(
            "lineage/source/Cargo.lock",
            &measure_file(&root.join("lineage/source/Cargo.lock"))?
        ),
        "dependency_source_archive": dependency_sources(root)?,
        "preparation_tools": [
            {
                "name": "source_archive_preparation",
                "payload": member_value(
                    "lineage/tools/rewrite-runtime-source-archive",
                    &measure_file(&root.join("lineage/tools/rewrite-runtime-source-archive"))?
                )
            },
            {
                "name": "source_manifest_preparation",
                "payload": member_value(
                    "lineage/tools/rewrite-runtime-source-manifest",
                    &measure_file(&root.join("lineage/tools/rewrite-runtime-source-manifest"))?
                )
            }
        ],
        "procedure_id": "retonr:runtime-source-build:retained-program-lineage",
        "procedure_version": 2,
        "repository_source_archive": {
            "archive_kind": "retonr_source",
            "archive_root": "retonr-source",
            "payload": member_value(
                "lineage/source/retonr-source.tar",
                &measure_file(&root.join("lineage/source/retonr-source.tar"))?
            ),
            "workspace_provenance": {
                "base_commit": "6a9a00bc1af7181fae6489f5856653ccc7c5bb4b",
                "identity_source": "archive",
                "workspace_state": "dirty_snapshot"
            }
        },
        "rust_distribution": {
            "channel": "1.97.1",
            "channel_manifest": member_value(
                "lineage/rust/channel-rust-1.97.1.toml",
                channel_manifest
            ),
            "channel_manifest_checksum": member_value(
                "lineage/rust/channel-rust-1.97.1.toml.sha256",
                channel_checksum
            ),
            "channel_manifest_digest": RUST_CHANNEL_MANIFEST_DIGEST,
            "channel_manifest_signature": member_value(
                "lineage/rust/channel-rust-1.97.1.toml.asc",
                channel_signature
            ),
            "components": distribution_components,
            "host_target": "x86_64-unknown-linux-musl",
            "release_key": member_value(
                "lineage/rust/rust-key.gpg.ascii",
                release_key
            ),
            "release_key_fingerprint": RUST_RELEASE_KEY_FINGERPRINT,
            "signature_disposition": "review_required",
            "target_standard_library": "x86_64-unknown-linux-musl"
        },
        "schema_version": 2
    }))
}

fn dependency_sources(root: &Path) -> Result<Value, ()> {
    Ok(json!({
        "archive_kind": "cargo_vendor",
        "archive_root": "cargo-vendor",
        "cargo_lock": member_value(
            "lineage/source/Cargo.lock",
            &measure_file(&root.join("lineage/source/Cargo.lock"))?
        ),
        "payload": member_value(
            "lineage/source/cargo-vendor.tar",
            &measure_file(&root.join("lineage/source/cargo-vendor.tar"))?
        ),
        "raw_crate_archive_kind": "cargo_crates",
        "raw_crate_archive_root": "cargo-crates",
        "raw_crate_source_archive": member_value(
            "lineage/source/cargo-crates.tar",
            &measure_file(&root.join("lineage/source/cargo-crates.tar"))?
        )
    }))
}

fn distribution_component(root: &Path, component: &RustDistributionComponent) -> Result<Value, ()> {
    let measured = measure_expected(
        root,
        component.relative_path,
        component.byte_size,
        component.digest,
    )?;
    Ok(distribution_component_value(component, &measured))
}

fn distribution_component_value(
    component: &RustDistributionComponent,
    measured: &Measurement,
) -> Value {
    json!({
        "name": component.name,
        "payload": member_value(component.relative_path, measured),
        "target": component.target,
        "url": component.url
    })
}

fn member_value(relative_path: &str, measurement: &Measurement) -> Value {
    let mut value = measurement_value(measurement);
    value["relative_path"] = json!(relative_path);
    value
}

fn measure_expected(
    root: &Path,
    relative_path: &str,
    expected_bytes: u64,
    expected_digest: &str,
) -> Result<Measurement, ()> {
    let measurement = measure_file(&root.join(relative_path))?;
    let digest = Digest::from_sha256_hex(expected_digest).map_err(|_| ())?;
    if measurement.byte_size != expected_bytes || measurement.digest != digest {
        return Err(());
    }
    Ok(measurement)
}

const RUST_DISTRIBUTION_COMPONENTS: [RustDistributionComponent; 3] = [
    RustDistributionComponent {
        name: "cargo",
        target: "x86_64-unknown-linux-musl",
        relative_path: "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        byte_size: 17_472_040,
        digest: "d0aeadfea55964a8866014efbe08bcfd6225d68529d522a6eef300d4f8d5c9d2",
        url: "https://static.rust-lang.org/dist/2026-07-16/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
    },
    RustDistributionComponent {
        name: "rust_std",
        target: "x86_64-unknown-linux-musl",
        relative_path: "lineage/rust/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        byte_size: 67_584_421,
        digest: "d160dfc81d21fdc72534859fae249fecf6ab70640375f64fc4e77005a48c18d0",
        url: "https://static.rust-lang.org/dist/2026-07-16/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
    },
    RustDistributionComponent {
        name: "rustc",
        target: "x86_64-unknown-linux-musl",
        relative_path: "lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        byte_size: 172_143_184,
        digest: "33a15df85ab0faf63b4c75b1113e47b41fd745a73bdc898c41034e9a9257b154",
        url: "https://static.rust-lang.org/dist/2026-07-16/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
    },
];

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn exact_measurement_rejects_size_and_digest_drift() {
        let root = tempdir().expect("root");
        fs::write(root.path().join("payload"), b"exact").expect("payload");
        let digest = Digest::sha256(b"exact");
        assert!(measure_expected(root.path(), "payload", 5, digest.as_str()).is_ok());
        assert!(measure_expected(root.path(), "payload", 4, digest.as_str()).is_err());
        assert!(
            measure_expected(root.path(), "payload", 5, Digest::sha256(b"other").as_str()).is_err()
        );
        let component = RustDistributionComponent {
            name: "fixture",
            target: "x86_64-unknown-linux-musl",
            relative_path: "payload",
            byte_size: 5,
            digest: "fa79d4746c21cd960a17b92db8976ddef95a7e20b590721f8e0fa7847a05e486",
            url: "https://example.invalid/payload",
        };
        assert!(distribution_component(root.path(), &component).is_ok());
    }

    #[test]
    fn compiled_record_is_content_free_inert_and_musl_hosted() {
        let root = tempdir().expect("root");
        for path in [
            "helper/isolation",
            "scripts/build",
            "lineage/build-recipe-v2.json",
            "lineage/source/Cargo.lock",
            "lineage/source/cargo-crates.tar",
            "lineage/source/cargo-vendor.tar",
            "lineage/source/retonr-source.tar",
            "lineage/tools/rewrite-runtime-source-archive",
            "lineage/tools/rewrite-runtime-source-manifest",
        ] {
            let destination = root.path().join(path);
            fs::create_dir_all(destination.parent().expect("parent")).expect("directory");
            fs::write(destination, path.as_bytes()).expect("fixture member");
        }
        let evidence = root.path().join("evidence");
        fs::write(&evidence, b"evidence").expect("evidence");
        let measured = measure_file(&evidence).expect("measurement");
        let host = build_host_value(
            &measured, &measured, &measured, &measured, &measured, &measured, &measured,
        );
        let distributions = RUST_DISTRIBUTION_COMPONENTS
            .iter()
            .map(|component| distribution_component_value(component, &measured))
            .collect::<Vec<_>>();
        let value = compile_value(
            root.path(),
            &host,
            &distributions,
            &measured,
            &measured,
            &measured,
            &measured,
        )
        .expect("compile record");
        assert_eq!(value["authority"], "none");
        assert_eq!(value["build_host"], host);
        assert_eq!(
            value["rust_distribution"]["host_target"],
            "x86_64-unknown-linux-musl"
        );
        assert_eq!(
            value["preparation_tools"][0]["payload"]["relative_path"],
            "lineage/tools/rewrite-runtime-source-archive"
        );
        assert_eq!(
            value["dependency_source_archive"]["cargo_lock"],
            value["cargo_lock"]
        );
        assert_eq!(
            value["dependency_source_archive"]["raw_crate_source_archive"]["relative_path"],
            "lineage/source/cargo-crates.tar"
        );
        assert_eq!(value["procedure_version"], 2);
        assert_eq!(value["schema_version"], 2);
        assert!(value.get("bootstrap_evidence").is_none());
        assert!(
            !serde_json::to_vec(&value)
                .expect("canonical")
                .windows(3)
                .any(|window| window == b"C:\\")
        );
    }

    #[test]
    fn incomplete_roots_and_wrong_official_measurements_fail_closed() {
        let root = tempdir().expect("root");
        assert!(compile(root.path()).is_err());
        assert!(build_host(root.path()).is_err());
        fs::write(root.path().join("payload"), b"value").expect("payload");
        assert!(measure_expected(root.path(), "payload", 5, "invalid").is_err());
    }

    #[test]
    fn musl_release_component_sequence_is_fixed_and_unique() {
        let keys = RUST_DISTRIBUTION_COMPONENTS
            .iter()
            .map(|component| (component.name, component.target, component.relative_path))
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 3);
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            RUST_DISTRIBUTION_COMPONENTS
                .iter()
                .map(|component| (
                    component.name,
                    component.byte_size,
                    component.digest,
                    component.relative_path,
                    component.url,
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "cargo",
                    17_472_040,
                    "d0aeadfea55964a8866014efbe08bcfd6225d68529d522a6eef300d4f8d5c9d2",
                    "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
                    "https://static.rust-lang.org/dist/2026-07-16/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
                ),
                (
                    "rust_std",
                    67_584_421,
                    "d160dfc81d21fdc72534859fae249fecf6ab70640375f64fc4e77005a48c18d0",
                    "lineage/rust/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
                    "https://static.rust-lang.org/dist/2026-07-16/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
                ),
                (
                    "rustc",
                    172_143_184,
                    "33a15df85ab0faf63b4c75b1113e47b41fd745a73bdc898c41034e9a9257b154",
                    "lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
                    "https://static.rust-lang.org/dist/2026-07-16/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
                ),
            ]
        );
    }
}
