use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr as _,
};

use cargo_lock::Lockfile;
use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use serde_json::Value;

use super::{
    RetainedProgramLicenseEvidenceClosureId, RetainedProgramLicenseEvidenceError,
    RetainedProgramLicenseEvidenceLimits, RetainedProgramLicenseEvidenceReviewerFacts,
    VerifiedRetainedProgramLicenseEvidenceClosure,
    wire::{InventoryWire, MaterialWire, SubjectWire},
};
use crate::{
    json::validate_unique_json,
    source_build::{RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputRole, sequence_digest},
};

#[derive(Clone)]
pub(super) struct ClosureBindings {
    pub(super) cargo: Digest,
    pub(super) source_set: Digest,
    pub(super) upstream: Digest,
}

pub(super) fn verify_inventory(
    manifest: &RuntimeSourceBuildInputManifest,
    evidence_bytes: &[u8],
    lock_bytes: &[u8],
    bindings: ClosureBindings,
    limits: RetainedProgramLicenseEvidenceLimits,
) -> Result<VerifiedRetainedProgramLicenseEvidenceClosure, RetainedProgramLicenseEvidenceError> {
    let inventory = parse_inventory(evidence_bytes, limits)?;
    let expected = expected_subjects(manifest, lock_bytes, limits)?;
    let counts = verify_subjects_and_materials(&inventory, &expected, limits)?;
    let evidence_digest = Digest::sha256(evidence_bytes);
    let closure_digest = sequence_digest(
        b"retained-program-license-evidence/complete/v1",
        [
            bindings.source_set.as_str().as_bytes(),
            bindings.cargo.as_str().as_bytes(),
            bindings.upstream.as_str().as_bytes(),
            evidence_digest.as_str().as_bytes(),
            counts.component_subjects.to_string().as_bytes(),
            counts.cargo_packages.to_string().as_bytes(),
            counts.materials.to_string().as_bytes(),
            counts.material_bytes.to_string().as_bytes(),
        ],
    );
    Ok(VerifiedRetainedProgramLicenseEvidenceClosure {
        cargo_source_closure_id: bindings.cargo,
        closure_id: RetainedProgramLicenseEvidenceClosureId(closure_digest),
        evidence_digest,
        facts: counts,
        source_input_set_id: bindings.source_set,
        upstream_closure_id: bindings.upstream,
    })
}

pub(super) fn parse_inventory(
    bytes: &[u8],
    limits: RetainedProgramLicenseEvidenceLimits,
) -> Result<InventoryWire, RetainedProgramLicenseEvidenceError> {
    if bytes.is_empty() || bytes.len() > limits.maximum_inventory_bytes {
        return Err(RetainedProgramLicenseEvidenceError::QuotaExceeded);
    }
    validate_unique_json(bytes)
        .map_err(|()| RetainedProgramLicenseEvidenceError::NoncanonicalInventory)?;
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| RetainedProgramLicenseEvidenceError::InvalidInventory)?;
    if serde_json::to_vec(&value)
        .map_err(|_| RetainedProgramLicenseEvidenceError::InvalidInventory)?
        != bytes
    {
        return Err(RetainedProgramLicenseEvidenceError::NoncanonicalInventory);
    }
    match value.get("schema_version").and_then(Value::as_u64) {
        Some(1) => return Err(RetainedProgramLicenseEvidenceError::LegacyEvidenceIncomplete),
        Some(2) => {}
        _ => return Err(RetainedProgramLicenseEvidenceError::UnsupportedSchema),
    }
    serde_json::from_value(value).map_err(|_| RetainedProgramLicenseEvidenceError::InvalidInventory)
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

pub(super) fn expected_subjects(
    manifest: &RuntimeSourceBuildInputManifest,
    lock_bytes: &[u8],
    limits: RetainedProgramLicenseEvidenceLimits,
) -> Result<Vec<ExpectedSubject>, RetainedProgramLicenseEvidenceError> {
    let mut subjects = manifest
        .components()
        .iter()
        .filter(|component| {
            !component
                .roles()
                .contains(&RuntimeSourceBuildInputRole::LicenseEvidence)
        })
        .map(|component| ExpectedSubject::Component {
            byte_size: component.byte_size(),
            digest: component.digest().clone(),
            name: component.name().to_owned(),
            path: component.relative_path().clone(),
            revision: component.revision().to_owned(),
            roles: component.roles().to_vec(),
            source_locator: component.source_locator().to_owned(),
        })
        .collect::<Vec<_>>();
    let text = std::str::from_utf8(lock_bytes)
        .map_err(|_| RetainedProgramLicenseEvidenceError::IncompleteSubjectSet)?;
    let lockfile = Lockfile::from_str(text)
        .map_err(|_| RetainedProgramLicenseEvidenceError::IncompleteSubjectSet)?;
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
    subjects.extend(packages);
    if subjects.is_empty() || subjects.len() > limits.maximum_subjects {
        return Err(RetainedProgramLicenseEvidenceError::QuotaExceeded);
    }
    Ok(subjects)
}

pub(super) fn verify_subjects_and_materials(
    inventory: &InventoryWire,
    expected: &[ExpectedSubject],
    limits: RetainedProgramLicenseEvidenceLimits,
) -> Result<RetainedProgramLicenseEvidenceReviewerFacts, RetainedProgramLicenseEvidenceError> {
    if inventory.schema_version != 2
        || inventory.subjects.len() != expected.len()
        || inventory.subjects.len() > limits.maximum_subjects
        || inventory.materials.is_empty()
        || inventory.materials.len() > limits.maximum_materials
    {
        return Err(RetainedProgramLicenseEvidenceError::IncompleteSubjectSet);
    }
    let mut declared = BTreeMap::new();
    let mut string_bytes = 0_usize;
    for (actual, expected) in inventory.subjects.iter().zip(expected) {
        if !matches_expected(actual, expected) {
            return Err(RetainedProgramLicenseEvidenceError::IncompleteSubjectSet);
        }
        let key = expected.key();
        account(&mut string_bytes, &key, limits.maximum_string_bytes)?;
        let (expression, ids) = subject_review(actual);
        validate_expression(expression)?;
        account(&mut string_bytes, expression, limits.maximum_string_bytes)?;
        if ids.is_empty()
            || ids
                .windows(2)
                .any(|pair| pair[0].as_str() >= pair[1].as_str())
            || declared.insert(key, ids.clone()).is_some()
        {
            return Err(RetainedProgramLicenseEvidenceError::InvalidMaterialSet);
        }
    }
    let actual = verify_materials(&inventory.materials, &declared, limits, &mut string_bytes)?;
    if actual != declared {
        return Err(RetainedProgramLicenseEvidenceError::InvalidMaterialSet);
    }
    let component_subjects = expected
        .iter()
        .filter(|subject| matches!(subject, ExpectedSubject::Component { .. }))
        .count();
    Ok(RetainedProgramLicenseEvidenceReviewerFacts {
        cargo_packages: expected.len() - component_subjects,
        component_subjects,
        material_bytes: inventory
            .materials
            .iter()
            .map(|material| material.content.len())
            .sum(),
        materials: inventory.materials.len(),
    })
}

fn verify_materials(
    materials: &[MaterialWire],
    subjects: &BTreeMap<String, Vec<Digest>>,
    limits: RetainedProgramLicenseEvidenceLimits,
    string_bytes: &mut usize,
) -> Result<BTreeMap<String, Vec<Digest>>, RetainedProgramLicenseEvidenceError> {
    let mut actual = BTreeMap::<String, Vec<Digest>>::new();
    let mut seen = BTreeSet::new();
    let mut previous = None;
    let mut total = 0_usize;
    for material in materials {
        if !subjects.contains_key(&material.subject_key) {
            return Err(RetainedProgramLicenseEvidenceError::UnknownSubject);
        }
        ArtifactSetRelativePath::new(&material.relative_path)
            .map_err(|_| RetainedProgramLicenseEvidenceError::InvalidReviewerFact)?;
        let order = format!("{}\0{}", material.subject_key, material.relative_path);
        if previous
            .as_ref()
            .is_some_and(|value: &String| value >= &order)
        {
            return Err(RetainedProgramLicenseEvidenceError::InvalidMaterialSet);
        }
        previous = Some(order);
        let bytes = material.content.as_bytes();
        total = total
            .checked_add(bytes.len())
            .filter(|value| *value <= limits.maximum_total_material_bytes)
            .ok_or(RetainedProgramLicenseEvidenceError::QuotaExceeded)?;
        if bytes.is_empty()
            || bytes.len() > limits.maximum_material_bytes
            || material.byte_size != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            || material.digest != Digest::sha256(bytes)
        {
            return Err(RetainedProgramLicenseEvidenceError::MaterialMeasurementMismatch);
        }
        account(
            string_bytes,
            &material.subject_key,
            limits.maximum_string_bytes,
        )?;
        account(
            string_bytes,
            &material.relative_path,
            limits.maximum_string_bytes,
        )?;
        let id = material_id(material);
        if !seen.insert(id.as_str().to_owned()) {
            return Err(RetainedProgramLicenseEvidenceError::InvalidMaterialSet);
        }
        actual
            .entry(material.subject_key.clone())
            .or_default()
            .push(id);
    }
    for ids in actual.values_mut() {
        ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    }
    Ok(actual)
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

fn subject_review(subject: &SubjectWire) -> (&str, &Vec<Digest>) {
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

fn validate_expression(expression: &str) -> Result<(), RetainedProgramLicenseEvidenceError> {
    if expression.is_empty()
        || expression.len() > 512
        || expression.trim() != expression
        || !expression.is_ascii()
        || expression
            .bytes()
            .any(|byte| !(0x20..=0x7e).contains(&byte))
    {
        Err(RetainedProgramLicenseEvidenceError::InvalidReviewerFact)
    } else {
        Ok(())
    }
}

pub(super) fn material_id(material: &MaterialWire) -> Digest {
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

fn account(
    total: &mut usize,
    value: &str,
    maximum: usize,
) -> Result<(), RetainedProgramLicenseEvidenceError> {
    *total = total
        .checked_add(value.len())
        .filter(|value| *value <= maximum)
        .ok_or(RetainedProgramLicenseEvidenceError::QuotaExceeded)?;
    Ok(())
}
