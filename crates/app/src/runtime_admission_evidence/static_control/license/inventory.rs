use std::str::FromStr as _;

use cargo_lock::Lockfile;
use rewrite_types::Digest;

use super::super::common::{RuntimeAdmissionStaticControlError, StaticEvidenceFacts};

mod json;
mod validation;
mod wire;

use validation::{ExpectedSubject, validate_complete_inventory};
use wire::InventoryWire;

pub(super) const MAXIMUM_INVENTORY_BYTES: usize = 16 * 1_024 * 1_024;
const MAXIMUM_LOCK_BYTES: usize = 4 * 1_024 * 1_024;
const MAXIMUM_SUBJECTS: usize = 16_384;

pub(super) struct VerifiedInventory {
    pub(super) digest: Digest,
    pub(super) material_bytes: usize,
    pub(super) material_count: usize,
    pub(super) subjects: Vec<VerifiedInventorySubject>,
}

pub(super) struct VerifiedInventorySubject {
    pub(super) identity: Digest,
    pub(super) key: String,
}

pub(super) fn verify(
    facts: &StaticEvidenceFacts,
    inventory_bytes: &[u8],
    lock_bytes: &[u8],
) -> Result<VerifiedInventory, RuntimeAdmissionStaticControlError> {
    if lock_bytes.is_empty() || lock_bytes.len() > MAXIMUM_LOCK_BYTES {
        return Err(RuntimeAdmissionStaticControlError::LimitExceeded);
    }
    let inventory = parse_inventory(inventory_bytes)?;
    let expected = expected_subjects(facts, lock_bytes)?;
    validate_complete_inventory(inventory_bytes, &inventory, &expected)
}

fn parse_inventory(bytes: &[u8]) -> Result<InventoryWire, RuntimeAdmissionStaticControlError> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_INVENTORY_BYTES {
        return Err(RuntimeAdmissionStaticControlError::LimitExceeded);
    }
    json::validate_unique(bytes)?;
    let value = serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidEncoding)?;
    if serde_json::to_vec(&value)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidEncoding)?
        != bytes
    {
        return Err(RuntimeAdmissionStaticControlError::InvalidEncoding);
    }
    serde_json::from_value(value).map_err(|_| RuntimeAdmissionStaticControlError::InvalidEncoding)
}

fn expected_subjects(
    facts: &StaticEvidenceFacts,
    lock_bytes: &[u8],
) -> Result<Vec<ExpectedSubject>, RuntimeAdmissionStaticControlError> {
    let mut expected = facts
        .components
        .iter()
        .filter(|component| {
            !component
                .roles
                .contains(&rewrite_ollama_package::RuntimeSourceBuildInputRole::LicenseEvidence)
        })
        .map(ExpectedSubject::from_component)
        .collect::<Vec<_>>();
    let lock_text = std::str::from_utf8(lock_bytes)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidBinding)?;
    let lockfile = Lockfile::from_str(lock_text)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidBinding)?;
    let mut packages = lockfile
        .packages
        .iter()
        .map(|package| ExpectedSubject::Cargo {
            checksum: package.checksum.as_ref().map(ToString::to_string),
            name: package.name.to_string(),
            source: package.source.as_ref().map(ToString::to_string),
            version: package.version.to_string(),
        })
        .collect::<Vec<_>>();
    packages.sort_by_key(ExpectedSubject::key);
    if packages
        .windows(2)
        .any(|pair| pair[0].key() >= pair[1].key())
    {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    expected.extend(packages);
    if expected.is_empty() || expected.len() > MAXIMUM_SUBJECTS {
        return Err(RuntimeAdmissionStaticControlError::LimitExceeded);
    }
    Ok(expected)
}
