use std::{
    fs::{self, File},
    io::Read as _,
    path::Path,
};

use rewrite_types::Digest;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::Component;

const MAXIMUM_PARAMETER_BYTES: u64 = 64 * 1024;
const HASH_BUFFER_BYTES: usize = 64 * 1024;
const PARAMETER_KEYS: [&str; 14] = [
    "accelerator",
    "build_mode",
    "c_flags",
    "cmake_definitions",
    "go",
    "host_cxx_flags",
    "maximum_parallel_jobs",
    "native_library_alias_mode",
    "native_library_aliases",
    "reported_version",
    "schema_version",
    "source_date_epoch",
    "target",
    "x86_repack_compile_flags",
];

pub(super) struct Measurement {
    pub(super) byte_size: u64,
    pub(super) digest: Digest,
}

pub(super) fn measurement_value(measurement: &Measurement) -> Value {
    serde_json::json!({
        "byte_size": measurement.byte_size,
        "digest": measurement.digest
    })
}

pub(super) fn measure_file(path: &Path) -> Result<Measurement, ()> {
    let mut input = File::open(path).map_err(|_| ())?;
    let mut hasher = Sha256::new();
    let mut byte_size = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = input.read(&mut buffer).map_err(|_| ())?;
        if read == 0 {
            break;
        }
        byte_size = byte_size
            .checked_add(u64::try_from(read).map_err(|_| ())?)
            .ok_or(())?;
        hasher.update(&buffer[..read]);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize())).map_err(|_| ())?;
    Ok(Measurement { byte_size, digest })
}

pub(super) fn parameter_parallel_jobs(component_root: &Path) -> Result<String, ()> {
    let path = component_root.join("metadata/build-parameters.json");
    let metadata = path.symlink_metadata().map_err(|_| ())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAXIMUM_PARAMETER_BYTES {
        return Err(());
    }
    let bytes = fs::read(path).map_err(|_| ())?;
    if u64::try_from(bytes.len()).map_err(|_| ())? != metadata.len() {
        return Err(());
    }
    super::canonical_json::validate_unique(&bytes)?;
    let value = serde_json::from_slice::<Value>(&bytes).map_err(|_| ())?;
    let canonical = serde_json::to_vec(&value).map_err(|_| ())?;
    if bytes.strip_suffix(b"\n") != Some(canonical.as_slice()) {
        return Err(());
    }
    let object = value.as_object().ok_or(())?;
    if object.len() != PARAMETER_KEYS.len()
        || object.keys().map(String::as_str).ne(PARAMETER_KEYS)
        || value.get("schema_version").and_then(Value::as_u64) != Some(1)
    {
        return Err(());
    }
    let jobs = value
        .get("maximum_parallel_jobs")
        .and_then(Value::as_u64)
        .ok_or(())?;
    let jobs = u8::try_from(jobs).map_err(|_| ())?;
    if !(1..=64).contains(&jobs) {
        return Err(());
    }
    Ok(jobs.to_string())
}

pub(super) fn role_digest(components: &[Component], role: &str) -> Result<Digest, ()> {
    let mut matching = components
        .iter()
        .filter(|component| component.roles.contains(&role));
    let digest = matching.next().map(|component| component.digest.clone());
    if matching.next().is_some() {
        return Err(());
    }
    digest.ok_or(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::super::component_specs::COMPONENT_SPECS;
    use super::*;

    #[test]
    fn component_specs_are_exact_sorted_and_semantically_distinct() {
        assert_eq!(COMPONENT_SPECS.len(), 38);
        assert!(
            COMPONENT_SPECS.windows(2).all(|pair| {
                pair[0].relative_path.as_bytes() < pair[1].relative_path.as_bytes()
            })
        );
        for role in [
            "build_script",
            "isolation_helper",
            "retained_program_lineage",
            "canonical_build_recipe",
            "retonr_repository_source",
            "cargo_lockfile",
            "cargo_vendor_source",
            "cargo_raw_crate_source",
            "rust_channel_manifest",
            "rust_channel_manifest_checksum",
            "rust_channel_manifest_signature",
            "rust_release_trust_root",
            "rust_cargo_distribution",
            "rust_host_standard_library_distribution",
            "rust_target_standard_library_distribution",
            "rust_compiler_distribution",
            "source_archive_preparation_tool",
            "source_manifest_preparation_tool",
            "alpine_minirootfs",
            "alpine_minirootfs_checksum",
            "alpine_minirootfs_signature",
            "alpine_release_trust_root",
            "alpine_libgcc_package",
            "alpine_busybox_static_package",
        ] {
            assert_eq!(
                COMPONENT_SPECS
                    .iter()
                    .filter(|component| component.roles.contains(&role))
                    .count(),
                1,
                "{role}"
            );
        }
    }

    #[test]
    fn parameters_are_bounded_unique_canonical_and_jobs_are_limited() {
        let root = tempdir().expect("root");
        let metadata = root.path().join("metadata");
        fs::create_dir(&metadata).expect("metadata");
        let base = json!({
            "accelerator":"cpu_only", "build_mode":"pie", "c_flags":[],
            "cmake_definitions":{}, "go":{}, "host_cxx_flags":[],
            "maximum_parallel_jobs":64, "native_library_alias_mode":"materialize",
            "native_library_aliases":[], "reported_version":"fixture",
            "schema_version":1, "source_date_epoch":1, "target":"fixture",
            "x86_repack_compile_flags":[]
        });
        let write = |value: &Value| {
            let mut bytes = serde_json::to_vec(value).expect("parameters");
            bytes.push(b'\n');
            fs::write(metadata.join("build-parameters.json"), bytes).expect("write parameters");
        };
        write(&base);
        assert_eq!(parameter_parallel_jobs(root.path()), Ok("64".to_owned()));
        for jobs in [0, 65] {
            let mut changed = base.clone();
            changed["maximum_parallel_jobs"] = json!(jobs);
            write(&changed);
            assert!(parameter_parallel_jobs(root.path()).is_err());
        }
        fs::write(
            metadata.join("build-parameters.json"),
            b"{\"maximum_parallel_jobs\":1,\"maximum_parallel_jobs\":1}\n",
        )
        .expect("duplicate parameters");
        assert!(parameter_parallel_jobs(root.path()).is_err());
    }

    #[test]
    fn parameter_schema_rejects_type_shape_encoding_and_size_mutations() {
        let root = tempdir().expect("root");
        let metadata = root.path().join("metadata");
        fs::create_dir(&metadata).expect("metadata");
        let path = metadata.join("build-parameters.json");
        let base = json!({
            "accelerator":"cpu_only", "build_mode":"pie", "c_flags":[],
            "cmake_definitions":{}, "go":{}, "host_cxx_flags":[],
            "maximum_parallel_jobs":1, "native_library_alias_mode":"materialize",
            "native_library_aliases":[], "reported_version":"fixture",
            "schema_version":1, "source_date_epoch":1, "target":"fixture",
            "x86_repack_compile_flags":[]
        });
        for bytes in [
            serde_json::to_vec(&base).expect("missing LF"),
            serde_json::to_vec_pretty(&base).expect("pretty JSON"),
            b"{}\n".to_vec(),
        ] {
            fs::write(&path, bytes).expect("write mutation");
            assert!(parameter_parallel_jobs(root.path()).is_err());
        }
        for (pointer, replacement) in [
            ("/maximum_parallel_jobs", json!("1")),
            ("/schema_version", json!(2)),
        ] {
            let mut changed = base.clone();
            *changed.pointer_mut(pointer).expect("pointer") = replacement;
            let mut bytes = serde_json::to_vec(&changed).expect("changed parameters");
            bytes.push(b'\n');
            fs::write(&path, bytes).expect("write mutation");
            assert!(parameter_parallel_jobs(root.path()).is_err(), "{pointer}");
        }
        let mut unknown = base;
        unknown["unknown"] = json!(true);
        let mut bytes = serde_json::to_vec(&unknown).expect("unknown field");
        bytes.push(b'\n');
        fs::write(&path, bytes).expect("write unknown");
        assert!(parameter_parallel_jobs(root.path()).is_err());
        let oversized =
            usize::try_from(MAXIMUM_PARAMETER_BYTES).expect("parameter limit fits usize") + 1;
        fs::write(&path, vec![b'x'; oversized]).expect("oversized parameters");
        assert!(parameter_parallel_jobs(root.path()).is_err());
    }
}
