use std::{
    fs::{self, File},
    io::{Read as _, Seek as _, SeekFrom},
    path::Path,
};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use crate::retained_object::{ObjectStamp, validate_file_object};

use super::Measurement;

const BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn root_stamp(root: &Path) -> Result<ObjectStamp, ()> {
    let metadata = fs::symlink_metadata(root).map_err(|_| ())?;
    if !metadata.is_dir() {
        return Err(());
    }
    ObjectStamp::from_path(root, &metadata)
}

pub(super) fn measure_relative(
    root: &Path,
    relative: &ArtifactSetRelativePath,
    maximum: u64,
) -> Result<Measurement, ()> {
    let path = checked_path(root, relative)?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| ())?;
    let stamp = ObjectStamp::from_path(&path, &metadata)?;
    if stamp.length == 0 || stamp.length > maximum {
        return Err(());
    }
    let mut file = File::open(&path).map_err(|_| ())?;
    validate_file_object(&path, &file, &stamp)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(|_| ())?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| ())?)
            .filter(|value| *value <= stamp.length)
            .ok_or(())?;
        hasher.update(&buffer[..read]);
    }
    validate_file_object(&path, &file, &stamp)?;
    if total != stamp.length {
        return Err(());
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize())).map_err(|_| ())?;
    Ok(Measurement {
        byte_size: total,
        digest,
    })
}

pub(super) fn read_relative(
    root: &Path,
    relative: &ArtifactSetRelativePath,
    maximum: usize,
) -> Result<Vec<u8>, ()> {
    let measured = measure_relative(root, relative, u64::try_from(maximum).map_err(|_| ())?)?;
    let expected = usize::try_from(measured.byte_size).map_err(|_| ())?;
    let path = checked_path(root, relative)?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| ())?;
    let stamp = ObjectStamp::from_path(&path, &metadata)?;
    let mut file = File::open(&path).map_err(|_| ())?;
    validate_file_object(&path, &file, &stamp)?;
    file.seek(SeekFrom::Start(0)).map_err(|_| ())?;
    let mut bytes = Vec::with_capacity(expected);
    (&mut file)
        .take(u64::try_from(expected).map_err(|_| ())? + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    validate_file_object(&path, &file, &stamp)?;
    if bytes.len() != expected || Digest::sha256(&bytes) != measured.digest {
        return Err(());
    }
    Ok(bytes)
}

fn checked_path(root: &Path, relative: &ArtifactSetRelativePath) -> Result<std::path::PathBuf, ()> {
    let mut current = root.to_path_buf();
    let components = relative.as_str().split('/').collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|_| ())?;
        if index + 1 == components.len() {
            if !metadata.is_file() {
                return Err(());
            }
        } else if !metadata.is_dir() {
            return Err(());
        }
        ObjectStamp::from_path(&current, &metadata)?;
    }
    Ok(current)
}
