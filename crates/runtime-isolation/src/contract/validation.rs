use std::{ffi::OsStr, path::Path};

use crate::{IsolationError, IsolationResult};

pub(super) fn validate_absolute_path(path: &Path, field: &'static str) -> IsolationResult<()> {
    if !path.is_absolute() || path.as_os_str().is_empty() {
        return Err(IsolationError::InvalidLaunch(field));
    }
    Ok(())
}

pub(super) fn validate_environment_key(key: &OsStr, maximum: usize) -> IsolationResult<()> {
    validate_value(key, maximum)?;
    let text = key.to_string_lossy();
    if text.is_empty() || text.contains('=') || text.starts_with("REWRITE_ISOLATION_INTERNAL_") {
        return Err(IsolationError::InvalidLaunch("environment key"));
    }
    Ok(())
}

pub(super) fn validate_value(value: &OsStr, maximum: usize) -> IsolationResult<()> {
    if value.as_encoded_bytes().len() > maximum || value.as_encoded_bytes().contains(&0) {
        return Err(IsolationError::InvalidLaunch("value bytes"));
    }
    Ok(())
}
