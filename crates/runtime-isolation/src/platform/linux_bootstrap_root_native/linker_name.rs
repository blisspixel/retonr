use std::{fs, os::unix::fs::MetadataExt as _, path::Path};

use rewrite_types::Digest;

use super::{HelperFailure, tree::hash_file};

const LIBC_LINKER_NAME: &str = "lib/libc.so";
const LIBC_LINKER_TARGET: &str = "ld-musl-x86_64.so.1";
const LIBC_DOMAIN: &[u8] = b"runtime-isolation/alpine-libc-linker-name/v1";
const LIBGCC_LINKER_NAME: &str = "usr/lib/libgcc_s.so";
const LIBGCC_LINKER_TARGET: &str = "libgcc_s.so.1";
const LIBGCC_DOMAIN: &[u8] = b"runtime-isolation/alpine-libgcc-linker-name/v1";

pub(super) fn install_linker_names(root: &Path) -> Result<(), HelperFailure> {
    install(root, LIBC_LINKER_NAME, LIBC_LINKER_TARGET, LIBC_DOMAIN)?;
    install(
        root,
        LIBGCC_LINKER_NAME,
        LIBGCC_LINKER_TARGET,
        LIBGCC_DOMAIN,
    )?;
    Ok(())
}

pub(super) fn validate_libc_linker_name(root: &Path) -> Result<Digest, HelperFailure> {
    validate(root, LIBC_LINKER_NAME, LIBC_LINKER_TARGET, LIBC_DOMAIN)
}

pub(super) fn validate_libgcc_linker_name(root: &Path) -> Result<Digest, HelperFailure> {
    validate(
        root,
        LIBGCC_LINKER_NAME,
        LIBGCC_LINKER_TARGET,
        LIBGCC_DOMAIN,
    )
}

fn install(
    root: &Path,
    linker_name: &str,
    stored_target: &str,
    domain: &[u8],
) -> Result<Digest, HelperFailure> {
    std::os::unix::fs::symlink(stored_target, root.join(linker_name))
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    validate(root, linker_name, stored_target, domain)
}

fn validate(
    root: &Path,
    linker_name: &str,
    stored_target: &str,
    domain: &[u8],
) -> Result<Digest, HelperFailure> {
    let linker_path = root.join(linker_name);
    let payload = linker_path
        .parent()
        .ok_or(HelperFailure::BootstrapRootVerification)?
        .join(stored_target);
    let link_metadata =
        fs::symlink_metadata(&linker_path).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if !link_metadata.file_type().is_symlink()
        || fs::read_link(&linker_path).map_err(|_| HelperFailure::BootstrapRootVerification)?
            != Path::new(stored_target)
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let resolved =
        fs::metadata(&linker_path).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let payload_terminal =
        fs::metadata(&payload).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let payload_link =
        fs::symlink_metadata(&payload).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if !resolved.is_file()
        || !payload_terminal.is_file()
        || !payload_link.file_type().is_file()
        || resolved.dev() != payload_terminal.dev()
        || resolved.ino() != payload_terminal.ino()
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    binding_digest(domain, linker_name, stored_target, &hash_file(payload)?)
}

fn binding_digest(
    domain: &[u8],
    linker_name: &str,
    stored_target: &str,
    payload_digest: &Digest,
) -> Result<Digest, HelperFailure> {
    let mut commitment = Vec::new();
    for value in [
        domain,
        linker_name.as_bytes(),
        stored_target.as_bytes(),
        payload_digest.as_str().as_bytes(),
    ] {
        commitment.extend_from_slice(
            &u64::try_from(value.len())
                .map_err(|_| HelperFailure::BootstrapRootVerification)?
                .to_be_bytes(),
        );
        commitment.extend_from_slice(value);
    }
    Ok(Digest::sha256(&commitment))
}
