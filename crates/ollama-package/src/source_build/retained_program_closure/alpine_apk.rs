use std::io::{Cursor, Read};

use flate2::bufread::GzDecoder;
use rewrite_types::Digest;

use super::{RetainedProgramUpstreamClosureError, signature::verify_rsa_pkcs1_sha1_public_key_pem};

const ROOT_KEY_PATH: &[u8] = b"./etc/apk/keys/alpine-devel@lists.alpinelinux.org-6165ee59.rsa.pub";
const SIGNATURE_PATH: &[u8] = b".SIGN.RSA.alpine-devel@lists.alpinelinux.org-6165ee59.rsa.pub";
const MAXIMUM_ROOT_ENTRIES: usize = 1_024;
const MAXIMUM_ROOT_KEY_BYTES: u64 = 8 * 1024;
const MAXIMUM_SIGNATURE_SEGMENT_BYTES: u64 = 4 * 1024;
const MAXIMUM_CONTROL_SEGMENT_BYTES: u64 = 128 * 1024;
const MAXIMUM_DATA_SEGMENT_BYTES: u64 = 4 * 1024 * 1024;
const MAXIMUM_CONTROL_ENTRIES: usize = 64;
const MAXIMUM_METADATA_BYTES: u64 = 64 * 1024;
const MAXIMUM_DATA_ENTRIES: usize = 4_096;
const PRODUCTION_DATA_HASH: &str =
    "adf2fd208190617db6e69bcc6440c35aef8c7d70b4c7e51d4a6a4abe58ab4abe";

#[derive(Clone, Copy)]
struct ApkExpectation<'a> {
    signature_size: u64,
    package_name: &'a str,
    package_version: &'a str,
    origin: &'a str,
    build_date: &'a str,
    commit: &'a str,
    data_hash: &'a str,
    payload: Option<ApkPayloadExpectation<'a>>,
}

#[derive(Clone, Copy)]
struct ApkPayloadExpectation<'a> {
    path: &'a [u8],
    size: u64,
    mode: u32,
    digest: &'a str,
}

impl ApkExpectation<'static> {
    const fn libgcc() -> Self {
        Self {
            signature_size: 512,
            package_name: "libgcc",
            package_version: "15.2.0-r2",
            origin: "gcc",
            build_date: "1761849853",
            commit: "7fa6e7fd3465c5ff89cc33a56d20bf4d59105aaf",
            data_hash: PRODUCTION_DATA_HASH,
            payload: None,
        }
    }

    const fn busybox() -> Self {
        Self {
            signature_size: 512,
            package_name: "busybox-static",
            package_version: "1.37.0-r30",
            origin: "busybox",
            build_date: "1765894768",
            commit: "1e823a60eb85606954b3a5af5f8e5bbd1ea680cf",
            data_hash: "3f2808fa64ab074eea3e0910ebb80d5c0e36e15a369ebf18498e90ed467925ab",
            payload: Some(ApkPayloadExpectation {
                path: b"bin/busybox.static",
                size: 1_034_600,
                mode: 0o755,
                digest: "82bbbabec12a985ae58810cfe975c3399264dc888aa592d8e460732bdd30a8dd",
            }),
        }
    }
}

struct VerifiedApkPackage {
    payload: Option<Vec<u8>>,
}

pub(super) fn verify_alpine_libgcc_package(
    minirootfs: &[u8],
    package: &[u8],
) -> Result<(), RetainedProgramUpstreamClosureError> {
    verify_alpine_package(minirootfs, package, ApkExpectation::libgcc()).map(|_| ())
}

pub(super) fn verify_alpine_busybox_package(
    minirootfs: &[u8],
    package: &[u8],
) -> Result<Vec<u8>, RetainedProgramUpstreamClosureError> {
    verify_alpine_package(minirootfs, package, ApkExpectation::busybox())?
        .payload
        .ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)
}

fn verify_alpine_package(
    minirootfs: &[u8],
    package: &[u8],
    expected: ApkExpectation<'_>,
) -> Result<VerifiedApkPackage, RetainedProgramUpstreamClosureError> {
    let public_key_bytes = extract_root_key(minirootfs)?;

    let (signature_end, signature_tar) =
        decode_gzip_member(package, MAXIMUM_SIGNATURE_SEGMENT_BYTES)?;
    let (control_length, control_tar) = decode_gzip_member(
        package
            .get(signature_end..)
            .ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?,
        MAXIMUM_CONTROL_SEGMENT_BYTES,
    )?;
    let control_end = signature_end
        .checked_add(control_length)
        .ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    let data = package
        .get(control_end..)
        .ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    let (data_length, data_tar) = decode_gzip_member(data, MAXIMUM_DATA_SEGMENT_BYTES)?;
    if data_length != data.len() {
        return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
    }
    let payload = validate_data_tar(&data_tar, expected.payload)?;

    let signature_bytes = extract_signature(&signature_tar, expected.signature_size)?;
    verify_rsa_pkcs1_sha1_public_key_pem(
        &public_key_bytes,
        package
            .get(signature_end..control_end)
            .ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?,
        &signature_bytes,
    )?;

    let metadata = extract_metadata(&control_tar)?;
    verify_metadata(&metadata, data, expected)?;
    Ok(VerifiedApkPackage { payload })
}

fn decode_gzip_member(
    bytes: &[u8],
    maximum_output: u64,
) -> Result<(usize, Vec<u8>), RetainedProgramUpstreamClosureError> {
    if bytes.len() < 18 || bytes.get(..3) != Some(&[0x1f, 0x8b, 0x08]) {
        return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
    }
    let cursor = Cursor::new(bytes);
    let mut decoder = GzDecoder::new(cursor);
    let mut output = Vec::new();
    (&mut decoder)
        .take(maximum_output.saturating_add(1))
        .read_to_end(&mut output)
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    if u64::try_from(output.len()).unwrap_or(u64::MAX) > maximum_output {
        return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
    }
    let consumed = usize::try_from(decoder.into_inner().position())
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    if consumed == 0 || consumed > bytes.len() {
        return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
    }
    Ok((consumed, output))
}

fn extract_root_key(minirootfs: &[u8]) -> Result<Vec<u8>, RetainedProgramUpstreamClosureError> {
    let decoder = flate2::read::GzDecoder::new(minirootfs);
    let mut archive = tar::Archive::new(decoder);
    let mut result = None;
    let mut entries = archive
        .entries()
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    for (index, entry) in entries.by_ref().enumerate() {
        if index >= MAXIMUM_ROOT_ENTRIES {
            return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
        }
        let mut entry =
            entry.map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
        if entry.path_bytes().as_ref() == ROOT_KEY_PATH {
            if result.is_some()
                || !entry.header().entry_type().is_file()
                || entry.size() == 0
                || entry.size() > MAXIMUM_ROOT_KEY_BYTES
            {
                return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
            }
            let mut bytes = Vec::with_capacity(
                usize::try_from(entry.size())
                    .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?,
            );
            entry
                .read_to_end(&mut bytes)
                .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != entry.size() {
                return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
            }
            result = Some(bytes);
        }
    }
    result.ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)
}

fn extract_signature(
    tar_bytes: &[u8],
    expected_size: u64,
) -> Result<Vec<u8>, RetainedProgramUpstreamClosureError> {
    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    let mut entries = archive
        .entries()
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    let mut result = None;
    for (index, entry) in entries.by_ref().enumerate() {
        if index >= 2 {
            return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
        }
        let mut entry =
            entry.map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
        if entry.path_bytes().as_ref() != SIGNATURE_PATH
            || result.is_some()
            || !entry.header().entry_type().is_file()
            || entry.size() != expected_size
            || entry.header().mode().ok() != Some(0o644)
            || entry.header().uid().ok() != Some(0)
            || entry.header().gid().ok() != Some(0)
        {
            return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
        }
        let mut bytes = Vec::with_capacity(
            usize::try_from(expected_size)
                .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?,
        );
        entry
            .read_to_end(&mut bytes)
            .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != expected_size {
            return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
        }
        result = Some(bytes);
    }
    result.ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)
}

fn extract_metadata(tar_bytes: &[u8]) -> Result<Vec<u8>, RetainedProgramUpstreamClosureError> {
    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    let mut entries = archive
        .entries()
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    let mut result = None;
    for (index, entry) in entries.by_ref().enumerate() {
        if index >= MAXIMUM_CONTROL_ENTRIES {
            return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
        }
        let mut entry =
            entry.map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
        if entry.path_bytes().as_ref() == b".PKGINFO" {
            if result.is_some()
                || !entry.header().entry_type().is_file()
                || entry.size() == 0
                || entry.size() > MAXIMUM_METADATA_BYTES
            {
                return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
            }
            let mut bytes = Vec::with_capacity(
                usize::try_from(entry.size())
                    .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?,
            );
            entry
                .read_to_end(&mut bytes)
                .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
            result = Some(bytes);
        }
    }
    result.ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)
}

fn validate_data_tar(
    bytes: &[u8],
    expected_payload: Option<ApkPayloadExpectation<'_>>,
) -> Result<Option<Vec<u8>>, RetainedProgramUpstreamClosureError> {
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut files = 0_usize;
    let mut payload = None;
    let mut entries = archive
        .entries()
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    for (index, entry) in entries.by_ref().enumerate() {
        if index >= MAXIMUM_DATA_ENTRIES {
            return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
        }
        let mut entry =
            entry.map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
        let path_bytes = entry.path_bytes();
        let mut path = path_bytes.as_ref();
        if let Some(stripped) = path.strip_prefix(b"./") {
            path = stripped;
        }
        if entry.header().entry_type().is_dir()
            && let Some(stripped) = path.strip_suffix(b"/")
        {
            path = stripped;
        }
        if path.is_empty()
            || path.starts_with(b"/")
            || path.contains(&b'\\')
            || path
                .split(|byte| *byte == b'/')
                .any(|part| part.is_empty() || part == b"." || part == b"..")
            || !(entry.header().entry_type().is_file() || entry.header().entry_type().is_dir())
        {
            return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
        }
        if expected_payload.is_some_and(|expected| path == expected.path) {
            let expected = expected_payload
                .ok_or(RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
            if payload.is_some()
                || !entry.header().entry_type().is_file()
                || entry.size() != expected.size
                || entry.header().mode().ok() != Some(expected.mode)
            {
                return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
            }
            let mut bytes = Vec::with_capacity(
                usize::try_from(expected.size)
                    .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?,
            );
            entry
                .read_to_end(&mut bytes)
                .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != expected.size
                || Digest::sha256(&bytes).as_str() != expected.digest
            {
                return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
            }
            payload = Some(bytes);
        }
        files += usize::from(entry.header().entry_type().is_file());
    }
    if files == 0 || (expected_payload.is_some() && payload.is_none()) {
        return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
    }
    Ok(payload)
}

fn verify_metadata(
    bytes: &[u8],
    data_gzip: &[u8],
    expected: ApkExpectation<'_>,
) -> Result<(), RetainedProgramUpstreamClosureError> {
    if bytes.last() != Some(&b'\n') || bytes.contains(&b'\r') || bytes.contains(&0) {
        return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)?;
    let required = [
        ("pkgname", expected.package_name),
        ("pkgver", expected.package_version),
        ("arch", "x86_64"),
        ("origin", expected.origin),
        ("builddate", expected.build_date),
        ("commit", expected.commit),
        ("datahash", expected.data_hash),
    ];
    for (key, value) in required {
        let prefix = format!("{key} = ");
        let mut matches = text.lines().filter_map(|line| line.strip_prefix(&prefix));
        if matches.next() != Some(value) || matches.next().is_some() {
            return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
        }
    }
    if Digest::sha256(data_gzip).as_str() != expected.data_hash {
        return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
