use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::unix::fs::{FileExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::Path,
};

use sha2::{Digest as _, Sha256};

use crate::{
    contract::{RetainedRuntimeInputMember, runtime_input_file_digest},
    platform::linux_helper_setup::HelperFailure,
};

use super::materialized_metadata_is_exact;

pub(super) fn revalidate(input: &RetainedRuntimeInputMember) -> Result<(), HelperFailure> {
    let metadata = input
        .file
        .metadata()
        .map_err(|_| HelperFailure::RuntimeInputObjectMismatch)?;
    if metadata.is_file()
        && metadata.nlink() <= 1
        && metadata.len() == input.declaration.expected_bytes
        && metadata.dev() == input.declaration.identity.device
        && metadata.ino() == input.declaration.identity.inode
        && runtime_input_file_digest(&input.file, input.declaration.expected_bytes, None)
            .is_ok_and(|digest| digest == input.declaration.expected_digest)
    {
        Ok(())
    } else {
        Err(HelperFailure::RuntimeInputObjectMismatch)
    }
}

pub(super) fn member(
    input: &RetainedRuntimeInputMember,
    target: &Path,
) -> Result<(), HelperFailure> {
    revalidate(input)?;
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(target)
        .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    let mut offset = 0_u64;
    while offset < input.declaration.expected_bytes {
        let remaining = input.declaration.expected_bytes.saturating_sub(offset);
        let requested = buffer
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let read = input
            .file
            .read_at(&mut buffer[..requested], offset)
            .map_err(|_| HelperFailure::RuntimeInputObjectMismatch)?;
        if read == 0 {
            return Err(HelperFailure::RuntimeInputObjectMismatch);
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or(HelperFailure::RuntimeInputObjectMismatch)?;
    }
    require_exact_source_end(input, &mut buffer)?;
    drop(destination);
    let copied_digest = rewrite_types::Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| HelperFailure::RuntimeInputObjectMismatch)?;
    if copied_digest != input.declaration.expected_digest {
        return Err(HelperFailure::RuntimeInputObjectMismatch);
    }
    fs::set_permissions(target, fs::Permissions::from_mode(0o400))
        .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    let materialized = File::open(target).map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    let metadata = materialized
        .metadata()
        .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    let materialized_digest =
        runtime_input_file_digest(&materialized, input.declaration.expected_bytes, None)
            .map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?;
    if !materialized_metadata_is_exact(&input.declaration, &metadata)
        || materialized_digest != input.declaration.expected_digest
    {
        return Err(HelperFailure::RuntimeInputBoundaryBehavior);
    }
    revalidate(input)
}

fn require_exact_source_end(
    input: &RetainedRuntimeInputMember,
    buffer: &mut [u8],
) -> Result<(), HelperFailure> {
    if input
        .file
        .read_at(&mut buffer[..1], input.declaration.expected_bytes)
        .map_err(|_| HelperFailure::RuntimeInputObjectMismatch)?
        == 0
    {
        Ok(())
    } else {
        Err(HelperFailure::RuntimeInputObjectMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{RetainedRuntimeInputDeclaration, RuntimeInputObjectIdentity};
    use rewrite_types::Digest;

    fn retained_member(bytes: &[u8]) -> (tempfile::TempDir, RetainedRuntimeInputMember) {
        let root = tempfile::tempdir().expect("retained source root");
        let path = root.path().join("source");
        fs::write(&path, bytes).expect("write retained source");
        let file = File::open(&path).expect("open retained source");
        let metadata = file.metadata().expect("retained source metadata");
        let declaration = RetainedRuntimeInputDeclaration::from_wire(
            "blobs/sha256-01".to_owned(),
            Digest::sha256(bytes),
            u64::try_from(bytes.len()).expect("retained source length"),
            RuntimeInputObjectIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            },
        )
        .expect("retained source declaration");
        (root, RetainedRuntimeInputMember { declaration, file })
    }

    #[test]
    fn materialization_is_byte_exact_and_uses_a_distinct_read_only_inode() {
        let (_source, input) = retained_member(b"exact retained bytes");
        let destination = tempfile::tempdir().expect("private destination root");
        let target = destination.path().join("materialized");
        member(&input, &target).expect("materialize retained member");
        assert_eq!(
            fs::read(&target).expect("read materialized member"),
            b"exact retained bytes"
        );
        let target_metadata = fs::metadata(&target).expect("materialized metadata");
        assert_eq!(target_metadata.mode() & 0o777, 0o400);
        assert_ne!(
            (target_metadata.dev(), target_metadata.ino()),
            (
                input.declaration.identity.device,
                input.declaration.identity.inode
            )
        );
    }

    #[test]
    fn materialization_rejects_existing_targets_and_source_drift() {
        let (source, input) = retained_member(b"original");
        let destination = tempfile::tempdir().expect("private destination root");
        let target = destination.path().join("materialized");
        fs::write(&target, b"ambient").expect("write ambient target");
        assert_eq!(
            member(&input, &target),
            Err(HelperFailure::RuntimeInputBoundarySetup)
        );
        fs::write(source.path().join("source"), b"mutated!").expect("mutate source");
        assert_eq!(
            revalidate(&input),
            Err(HelperFailure::RuntimeInputObjectMismatch)
        );
    }
}
