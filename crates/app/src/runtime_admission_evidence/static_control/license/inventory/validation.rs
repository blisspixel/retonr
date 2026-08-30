use std::collections::{BTreeMap, BTreeSet};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_ollama_package::RuntimeSourceBuildInputRole;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use super::{
    MAXIMUM_SUBJECTS, VerifiedInventory, VerifiedInventorySubject,
    wire::{InventoryStatus, InventoryWire, MaterialWire, SubjectWire},
};
use crate::runtime_admission_evidence::static_control::common::{
    RuntimeAdmissionStaticControlError, StaticInputComponent,
};

const MAXIMUM_MATERIALS: usize = 65_536;
const MAXIMUM_MATERIAL_BYTES: usize = 2 * 1_024 * 1_024;
const MAXIMUM_TOTAL_MATERIAL_BYTES: usize = 12 * 1_024 * 1_024;
const MAXIMUM_STRING_BYTES: usize = 64 * 1_024 * 1_024;

pub(super) enum ExpectedSubject {
    Component {
        byte_size: u64,
        digest: Digest,
        name: String,
        path: ArtifactSetRelativePath,
        revision: String,
        roles: Vec<RuntimeSourceBuildInputRole>,
        source_locator: String,
    },
    Cargo {
        checksum: Option<String>,
        name: String,
        source: Option<String>,
        version: String,
    },
}

impl ExpectedSubject {
    pub(super) fn from_component(component: &StaticInputComponent) -> Self {
        Self::Component {
            byte_size: component.byte_size,
            digest: component.digest.clone(),
            name: component.name.clone(),
            path: component.relative_path.clone(),
            revision: component.revision.clone(),
            roles: component.roles.clone(),
            source_locator: component.source_locator.clone(),
        }
    }

    pub(super) fn key(&self) -> String {
        match self {
            Self::Component { path, .. } => format!("component:{}", path.as_str()),
            Self::Cargo {
                name,
                source,
                version,
                ..
            } => format!(
                "cargo:{name}@{version}:{}",
                source.as_deref().unwrap_or("path")
            ),
        }
    }
}

pub(super) fn validate_complete_inventory(
    inventory_bytes: &[u8],
    inventory: &InventoryWire,
    expected: &[ExpectedSubject],
) -> Result<VerifiedInventory, RuntimeAdmissionStaticControlError> {
    if inventory.schema_version != 2
        || inventory.status != InventoryStatus::PendingReview
        || inventory.subjects.len() != expected.len()
        || inventory.subjects.len() > MAXIMUM_SUBJECTS
        || inventory.materials.is_empty()
        || inventory.materials.len() > MAXIMUM_MATERIALS
    {
        return Err(RuntimeAdmissionStaticControlError::ReviewRequired);
    }
    let mut declared = BTreeMap::new();
    let mut verified_subjects = Vec::with_capacity(expected.len());
    let mut string_bytes = 0_usize;
    for (actual, expected) in inventory.subjects.iter().zip(expected) {
        if !matches_expected(actual, expected) {
            return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
        }
        let key = expected.key();
        account(&mut string_bytes, &key)?;
        let (expression, ids) = subject_review(actual);
        validate_expression(expression)?;
        account(&mut string_bytes, expression)?;
        if ids.is_empty()
            || ids
                .windows(2)
                .any(|pair| pair[0].as_str() >= pair[1].as_str())
            || declared.insert(key.clone(), ids.to_vec()).is_some()
        {
            return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
        }
        verified_subjects.push(VerifiedInventorySubject {
            identity: subject_identity(&key, actual)?,
            key,
        });
    }
    let (actual, material_bytes) =
        verify_materials(&inventory.materials, &declared, &mut string_bytes)?;
    if actual != declared {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    Ok(VerifiedInventory {
        digest: Digest::sha256(inventory_bytes),
        material_bytes,
        material_count: inventory.materials.len(),
        subjects: verified_subjects,
    })
}

fn verify_materials(
    materials: &[MaterialWire],
    subjects: &BTreeMap<String, Vec<Digest>>,
    string_bytes: &mut usize,
) -> Result<(BTreeMap<String, Vec<Digest>>, usize), RuntimeAdmissionStaticControlError> {
    let mut actual = BTreeMap::<String, Vec<Digest>>::new();
    let mut seen = BTreeSet::new();
    let mut previous = None;
    let mut total = 0_usize;
    for material in materials {
        if !subjects.contains_key(&material.subject_key) {
            return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
        }
        ArtifactSetRelativePath::new(&material.relative_path)
            .map_err(|_| RuntimeAdmissionStaticControlError::InvalidBinding)?;
        let order = format!("{}\0{}", material.subject_key, material.relative_path);
        if previous
            .as_ref()
            .is_some_and(|value: &String| value >= &order)
        {
            return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
        }
        previous = Some(order);
        let bytes = material.content.as_bytes();
        total = total
            .checked_add(bytes.len())
            .filter(|value| *value <= MAXIMUM_TOTAL_MATERIAL_BYTES)
            .ok_or(RuntimeAdmissionStaticControlError::LimitExceeded)?;
        if bytes.is_empty()
            || bytes.len() > MAXIMUM_MATERIAL_BYTES
            || material.byte_size != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            || material.digest != Digest::sha256(bytes)
        {
            return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
        }
        account(string_bytes, &material.subject_key)?;
        account(string_bytes, &material.relative_path)?;
        let id = material_id(material);
        if !seen.insert(id.as_str().to_owned()) {
            return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
        }
        actual
            .entry(material.subject_key.clone())
            .or_default()
            .push(id);
    }
    for ids in actual.values_mut() {
        ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    }
    Ok((actual, total))
}

fn matches_expected(actual: &SubjectWire, expected: &ExpectedSubject) -> bool {
    match (actual, expected) {
        (
            SubjectWire::Component {
                byte_size,
                digest,
                name,
                relative_path,
                revision,
                roles,
                source_locator,
                ..
            },
            ExpectedSubject::Component {
                byte_size: expected_bytes,
                digest: expected_digest,
                name: expected_name,
                path,
                revision: expected_revision,
                roles: expected_roles,
                source_locator: expected_locator,
            },
        ) => {
            byte_size == expected_bytes
                && digest == expected_digest
                && name == expected_name
                && relative_path == path.as_str()
                && revision == expected_revision
                && roles == expected_roles
                && source_locator == expected_locator
        }
        (
            SubjectWire::CargoPackage {
                checksum,
                name,
                source,
                version,
                ..
            },
            ExpectedSubject::Cargo {
                checksum: expected_checksum,
                name: expected_name,
                source: expected_source,
                version: expected_version,
            },
        ) => {
            checksum == expected_checksum
                && name == expected_name
                && source == expected_source
                && version == expected_version
        }
        _ => false,
    }
}

fn subject_review(subject: &SubjectWire) -> (&str, &[Digest]) {
    match subject {
        SubjectWire::Component {
            declared_spdx_expression,
            material_ids,
            ..
        }
        | SubjectWire::CargoPackage {
            declared_spdx_expression,
            material_ids,
            ..
        } => (declared_spdx_expression, material_ids),
    }
}

fn validate_expression(expression: &str) -> Result<(), RuntimeAdmissionStaticControlError> {
    if expression.is_empty()
        || expression.len() > 512
        || expression.trim() != expression
        || !expression.is_ascii()
        || expression
            .bytes()
            .any(|byte| !(0x20..=0x7e).contains(&byte))
    {
        Err(RuntimeAdmissionStaticControlError::InvalidBinding)
    } else {
        Ok(())
    }
}

fn subject_identity(
    key: &str,
    subject: &SubjectWire,
) -> Result<Digest, RuntimeAdmissionStaticControlError> {
    let value = serde_json::to_value(subject)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidEncoding)?;
    let bytes = serde_json::to_vec(&value)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidEncoding)?;
    Ok(sequence_digest(
        b"retonr:runtime-admission:license-inventory-subject:v2",
        [key.as_bytes(), bytes.as_slice()],
    ))
}

fn material_id(material: &MaterialWire) -> Digest {
    sequence_digest(
        b"retained-program-license-evidence/material/v1",
        [
            material.subject_key.as_bytes(),
            material.kind.domain_name().as_bytes(),
            material.relative_path.as_bytes(),
            material.byte_size.to_string().as_bytes(),
            material.digest.as_str().as_bytes(),
        ],
    )
}

fn sequence_digest<'a>(domain: &[u8], values: impl IntoIterator<Item = &'a [u8]>) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for value in values {
        hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(value);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .expect("SHA-256 formatting always produces one valid digest")
}

fn account(total: &mut usize, value: &str) -> Result<(), RuntimeAdmissionStaticControlError> {
    *total = total
        .checked_add(value.len())
        .filter(|value| *value <= MAXIMUM_STRING_BYTES)
        .ok_or(RuntimeAdmissionStaticControlError::LimitExceeded)?;
    Ok(())
}
