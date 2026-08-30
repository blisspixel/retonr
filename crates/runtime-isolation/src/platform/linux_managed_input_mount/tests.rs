use rewrite_types::Digest;

use super::*;

fn declaration(alias: &str, byte: u8) -> RetainedRuntimeInputDeclaration {
    RetainedRuntimeInputDeclaration::from_wire(
        alias.to_owned(),
        Digest::sha256(&[byte]),
        1,
        crate::contract::RuntimeInputObjectIdentity {
            device: 10 + u64::from(byte),
            inode: 20 + u64::from(byte),
        },
    )
    .expect("declaration")
}

#[test]
fn expected_tree_accepts_exact_ollama_store_shape() {
    let declarations = [
        declaration("blobs/sha256-01", 1),
        declaration("blobs/sha256-02", 2),
        declaration("blobs/sha256-03", 3),
        declaration("blobs/sha256-04", 4),
        declaration("blobs/sha256-05", 5),
        declaration("manifests/registry.ollama.ai/library/granite3.3/2b", 6),
    ];
    let directories = expected_directories(&declarations).expect("directories");
    assert!(directories.contains("blobs"));
    assert!(directories.contains("manifests/registry.ollama.ai/library/granite3.3"));
    assert_eq!(directories.len(), 5);
}

#[test]
fn expected_tree_rejects_file_directory_conflicts() {
    let declarations = [declaration("blobs", 1), declaration("blobs/sha256-02", 2)];
    assert_eq!(
        expected_directories(&declarations),
        Err(HelperFailure::RuntimeInputObjectMismatch)
    );
}

#[test]
fn mount_flags_require_topmost_read_only_objects_and_writable_scratch() {
    let declaration = declaration("blobs/sha256-01", 1);
    let exact = format!(
        concat!(
            "1 0 0:1 / /tmp rw,nosuid,nodev,noexec - tmpfs tmpfs rw\n",
            "2 1 0:2 / {} ro,nosuid,nodev,noexec - tmpfs tmpfs ro\n",
            "3 2 0:3 / {} ro,nosuid,nodev,noexec - ext4 /dev/sda ro\n"
        ),
        MANAGED_RUNTIME_INPUT_ROOT_V1,
        input_path(&declaration.relative_alias).display(),
    );
    assert!(mount_flags_are_exact(&exact, &[&declaration]));
    assert!(!mount_flags_are_exact(
        &exact.replace("ro,nosuid,nodev,noexec", "rw,nosuid,nodev,noexec"),
        &[&declaration]
    ));
}

#[test]
fn empty_admission_probe_does_not_reserve_the_model_sized_tmpfs_ceiling() {
    assert_eq!(input_mount_bytes(0), 1024 * 1024);
    assert_eq!(input_mount_bytes(1), MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES);
}
