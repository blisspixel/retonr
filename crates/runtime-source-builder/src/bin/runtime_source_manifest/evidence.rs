use std::{fs, fs::OpenOptions, io::Write as _, path::Path};

use rewrite_types::Digest;
use serde_json::{Value, json};

use super::{
    manifest_support::{Measurement, measure_file, measurement_value},
    program_lineage,
};

pub(super) fn write(root: &Path) -> Result<(), ()> {
    let source = source_provenance(root)?;
    let lineage = program_lineage::compile(root)?;
    let lineage_bytes = serde_json::to_vec(&lineage).map_err(|_| ())?;
    let lineage_measurement = Measurement {
        byte_size: u64::try_from(lineage_bytes.len()).map_err(|_| ())?,
        digest: Digest::sha256(&lineage_bytes),
    };
    let tools = tool_evidence(root, &lineage_measurement)?;
    let checksums = go_checksum_evidence(root)?;
    let mut records = [
        ("metadata/source-provenance.json", source),
        ("metadata/tool-evidence.json", tools),
        ("modules/go-checksum-set.json", checksums),
    ]
    .map(|(path, value)| {
        serde_json::to_vec(&value)
            .map(|bytes| (path, bytes))
            .map_err(|_| ())
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    records.push(("metadata/retained-program-lineage.json", lineage_bytes));
    if records
        .iter()
        .any(|(path, _bytes)| root.join(path).exists())
    {
        return Err(());
    }
    for (path, bytes) in records {
        let path = root.join(path);
        fs::create_dir_all(path.parent().ok_or(())?).map_err(|_| ())?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|_| ())?;
        output.write_all(&bytes).map_err(|_| ())?;
    }
    Ok(())
}

fn source_provenance(root: &Path) -> Result<Value, ()> {
    let ollama = measure_file(&root.join("sources/ollama.tar"))?;
    let llama = measure_file(&root.join("sources/llama-cpp.tar"))?;
    let zig_patch = measure_file(&root.join("patches/llama-cpp-zig-clang20-evex512.patch"))?;
    let ollama_patch = measure_file(&root.join("patches/ollama-mlx-reproducible-errors.patch"))?;
    Ok(json!({
        "llama_cpp": {
            "acquired_archive": {
                "byte_size": 36_853_739_u64,
                "digest": "a006ca1a0268a3748686040d1ae021939d92ec0f58f341a30e52056298da4a0b",
                "locator": "https://github.com/ggml-org/llama.cpp/archive/refs/tags/b10488.tar.gz"
            },
            "normalized_component": measurement_value(&llama),
            "revision": "9d77fa17254e1dee4b9e92504c91611a60b1359f",
            "upstream_tag": "b10488"
        },
        "ollama": {
            "acquired_archive": {
                "byte_size": 28_974_128_u64,
                "digest": "5370f62c5dd875b58e37b312524af6af311119b5443e2907e20c2aa239fca52e",
                "locator": "https://github.com/ollama/ollama/archive/refs/tags/v0.32.15.tar.gz"
            },
            "normalized_component": measurement_value(&ollama),
            "revision": "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
            "upstream_tag": "v0.32.15"
        },
        "retained_source_patches": [
            {
                "name": "llama-cpp-zig-clang20-evex512",
                "normalized_component": measurement_value(&zig_patch),
                "target_revision": "9d77fa17254e1dee4b9e92504c91611a60b1359f"
            },
            {
                "name": "ollama-mlx-reproducible-errors",
                "normalized_component": measurement_value(&ollama_patch),
                "target_revision": "b7871fc0d1d82fe109536efa3e0e8e411c766c75"
            }
        ],
        "schema_version": 1
    }))
}

fn tool_evidence(root: &Path, lineage: &Measurement) -> Result<Value, ()> {
    let measured = |path| measure_file(&root.join(path)).map(|value| measurement_value(&value));
    Ok(json!({
        "build_environment": {
            "alpine_minirootfs_digest": "42d0e6d8de5521e7bf92e075e032b5690c1d948fa9775efa32a51a38b25460fb",
            "alpine_version": "3.23.3",
            "installed_package_database_digest": "0e6b12c8a93fbade9e260548b69b05ef1a78603c58949d0099498e986ede555b"
        },
        "cmake": {
            "build": ["static", "openssl-disabled", "release"],
            "normalized_component": measured("toolchains/cmake.tar")?,
            "source_archive_digest": "42abb3f48f37dbd739cdfeb19d3712db0c5935ed5c2aef6c340f9ae9114238a2",
            "version": "3.31.2"
        },
        "go": {
            "acquired_archive_digest": "aac1b08a0fb0c4e0a7c1555beb7b59180b05dfc5a3d62e40e9de90cd42f88235",
            "normalized_component": measured("toolchains/go.tar")?,
            "version": "1.26.0"
        },
        "ninja": {
            "build": ["static", "retained-shell-patch"],
            "normalized_component": measured("toolchains/ninja.tar")?,
            "shell_patch_digest": "64183fa01024ca2bbf57f271b843d3bc5a784c9c0927b4555bcedfa933f34729",
            "source_archive_digest": "821bdff48a3f683bc4bb3b6f0b5fe7b2d647cf65d52aeb63328c91a6c6df285a",
            "version": "1.12.1"
        },
        "retained_program_lineage_binding": {
            "authority": "none",
            "builder": measured("scripts/build")?,
            "isolation_helper": measured("helper/isolation")?,
            "lineage_record": measurement_value(lineage),
            "profile_id": "retonr:runtime-source-build:production-lineage",
            "profile_version": 2,
            "rust_host_target": "x86_64-unknown-linux-musl",
            "rustc": "1.97.1",
            "source_archive_preparation": measured("lineage/tools/rewrite-runtime-source-archive")?,
            "source_manifest_preparation": measured("lineage/tools/rewrite-runtime-source-manifest")?
        },
        "schema_version": 1,
        "shell": {
            "component": measured("toolchains/busybox")?,
            "package": "busybox-static-1.37.0-r30"
        },
        "zig": {
            "acquired_archive_digest": "c61c5da6edeea14ca51ecd5e4520c6f4189ef5250383db33d01848293bfafe05",
            "normalized_component": measured("toolchains/zig.tar")?,
            "version": "0.15.1"
        }
    }))
}

fn go_checksum_evidence(root: &Path) -> Result<Value, ()> {
    let cache = measure_file(&root.join("modules/go-module-cache.tar"))?;
    Ok(json!({
        "go_mod": {
            "byte_size": 4_608_u64,
            "digest": "1795c1c2c37b3f43ad43945396c33c610167ce2541281d6158ab3586c885a70b"
        },
        "go_sum": {
            "byte_size": 50_566_u64,
            "digest": "b2cc57a1dd1de5a06653c1184943d8454333f850540c6470faced2ca48dc9bb5"
        },
        "module_cache": measurement_value(&cache),
        "offline_verification": "go mod verify",
        "schema_version": 1
    }))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn tool_evidence_binds_the_named_inert_program_lineage_subrecord() {
        let root = tempdir().expect("root");
        for path in [
            "helper/isolation",
            "lineage/tools/rewrite-runtime-source-archive",
            "lineage/tools/rewrite-runtime-source-manifest",
            "scripts/build",
            "toolchains/busybox",
            "toolchains/cmake.tar",
            "toolchains/go.tar",
            "toolchains/ninja.tar",
            "toolchains/zig.tar",
        ] {
            let destination = root.path().join(path);
            fs::create_dir_all(destination.parent().expect("parent")).expect("directory");
            fs::write(destination, path.as_bytes()).expect("fixture member");
        }
        let lineage = Measurement {
            byte_size: 42,
            digest: Digest::sha256(b"lineage"),
        };
        let value = tool_evidence(root.path(), &lineage).expect("tool evidence");
        let binding = &value["retained_program_lineage_binding"];
        assert_eq!(binding["authority"], "none");
        assert_eq!(binding["lineage_record"], measurement_value(&lineage));
        assert_eq!(binding["rust_host_target"], "x86_64-unknown-linux-musl");
        assert_eq!(binding["rustc"], "1.97.1");
        assert_ne!(
            binding["source_archive_preparation"],
            binding["source_manifest_preparation"]
        );
    }
}
