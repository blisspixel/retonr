use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write as _,
    path::Path,
    str::FromStr as _,
};

use cargo_lock::Lockfile;
use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::{
    canonical_json,
    component_specs::{COMPONENT_SPECS, ComponentSpec},
    manifest_support::Measurement,
};

#[path = "license_inventory/input.rs"]
mod input;
#[path = "license_inventory/wire.rs"]
mod wire;

use input::{measure_relative, read_relative, root_stamp};
use wire::{MaterialKind, MaterialSelection, ReviewInput, ReviewStatus, SubjectSelection};

const INVENTORY_PATH: &str = "legal/licenses.json";
const REVIEW_PATH: &str = "review.json";
const MATERIAL_ROOT: &str = "materials";
const MAXIMUM_COMPONENT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAXIMUM_LOCK_BYTES: usize = 4 * 1024 * 1024;
const MAXIMUM_REVIEW_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_SUBJECTS: usize = 16_384;
const MAXIMUM_MATERIALS: usize = 65_536;
const MAXIMUM_MATERIAL_BYTES: usize = 2 * 1024 * 1024;
const MAXIMUM_TOTAL_MATERIAL_BYTES: usize = 12 * 1024 * 1024;
const MAXIMUM_INVENTORY_BYTES: usize = 16 * 1024 * 1024;

pub(super) fn write(component_root: &Path, review_root: &Path) -> Result<(), ()> {
    let review_root = review_root.canonicalize().map_err(|_| ())?;
    if review_root.starts_with(component_root) || component_root.starts_with(&review_root) {
        return Err(());
    }
    let component_before = root_stamp(component_root)?;
    let review_before = root_stamp(&review_root)?;
    let inventory = compile(component_root, &review_root)?;
    if root_stamp(component_root)? != component_before || root_stamp(&review_root)? != review_before
    {
        return Err(());
    }
    let destination = component_root.join(INVENTORY_PATH);
    if destination.exists() {
        return Err(());
    }
    let parent = destination.parent().ok_or(())?;
    match fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.is_dir() => {
            root_stamp(parent)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(parent).map_err(|_| ())?;
            root_stamp(parent)?;
        }
        Ok(_) | Err(_) => return Err(()),
    }
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&destination)
        .map_err(|_| ())?;
    let published = output
        .write_all(&inventory)
        .and_then(|()| output.flush())
        .and_then(|()| output.sync_all())
        .map_err(|_| ());
    drop(output);
    if published.is_err() {
        let _ = fs::remove_file(&destination);
        return Err(());
    }
    let inventory_path = ArtifactSetRelativePath::new(INVENTORY_PATH).map_err(|_| ())?;
    match read_relative(component_root, &inventory_path, MAXIMUM_INVENTORY_BYTES) {
        Ok(readback) if readback == inventory => Ok(()),
        Ok(_) | Err(()) => {
            let _ = fs::remove_file(&destination);
            Err(())
        }
    }
}

pub(super) fn write_template(component_root: &Path, destination: &Path) -> Result<(), ()> {
    let component_before = root_stamp(component_root)?;
    let mut expected = expected_components(component_root)?;
    expected.extend(expected_cargo_packages(component_root)?);
    if expected.is_empty() || expected.len() > MAXIMUM_SUBJECTS {
        return Err(());
    }
    let subjects = expected
        .iter()
        .map(|subject| {
            json!({
                "declared_spdx_expression": "",
                "materials": [],
                "subject_identity": subject.review_identity(),
                "subject_key": subject.key()
            })
        })
        .collect::<Vec<_>>();
    let mut bytes = serde_json::to_vec(&json!({
        "schema_version": 1,
        "status": "pending_review",
        "subjects": subjects
    }))
    .map_err(|_| ())?;
    bytes.push(b'\n');
    if bytes.len() > MAXIMUM_REVIEW_BYTES || root_stamp(component_root)? != component_before {
        return Err(());
    }
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|_| ())?;
    output.write_all(&bytes).map_err(|_| ())
}

fn compile(component_root: &Path, review_root: &Path) -> Result<Vec<u8>, ()> {
    let mut expected = expected_components(component_root)?;
    expected.extend(expected_cargo_packages(component_root)?);
    if expected.is_empty() || expected.len() > MAXIMUM_SUBJECTS {
        return Err(());
    }
    let review_path = ArtifactSetRelativePath::new(REVIEW_PATH).map_err(|_| ())?;
    let review_bytes = read_relative(review_root, &review_path, MAXIMUM_REVIEW_BYTES)?;
    canonical_json::validate_unique(&review_bytes)?;
    let body = review_bytes.strip_suffix(b"\n").ok_or(())?;
    let value = serde_json::from_slice::<Value>(body).map_err(|_| ())?;
    if serde_json::to_vec(&value).map_err(|_| ())? != body {
        return Err(());
    }
    let review = serde_json::from_value::<ReviewInput>(value).map_err(|_| ())?;
    compile_inventory(review_root, &expected, review)
}

fn compile_inventory(
    review_root: &Path,
    expected: &[ExpectedSubject],
    review: ReviewInput,
) -> Result<Vec<u8>, ()> {
    if review.schema_version != 1
        || !matches!(review.status, ReviewStatus::PendingReview)
        || review.subjects.len() != expected.len()
        || review.subjects.len() > MAXIMUM_SUBJECTS
    {
        return Err(());
    }
    let mut subjects = Vec::with_capacity(expected.len());
    let mut materials = Vec::new();
    let mut total_material_bytes = 0_usize;
    let mut material_ids = BTreeSet::new();
    for (expected, selection) in expected.iter().zip(review.subjects) {
        if selection.subject_key != expected.key()
            || selection.subject_identity != expected.review_identity()
        {
            return Err(());
        }
        validate_expression(&selection.declared_spdx_expression)?;
        let ids = compile_materials(
            review_root,
            &selection,
            &mut materials,
            &mut total_material_bytes,
            &mut material_ids,
        )?;
        subjects.push(expected.value(&selection.declared_spdx_expression, &ids));
    }
    if materials.is_empty() || materials.len() > MAXIMUM_MATERIALS {
        return Err(());
    }
    materials.sort_by(|left, right| material_order(left).cmp(&material_order(right)));
    let inventory = json!({
        "materials": materials,
        "schema_version": 2,
        "status": "pending_review",
        "subjects": subjects
    });
    let bytes = serde_json::to_vec(&inventory).map_err(|_| ())?;
    if bytes.len() > MAXIMUM_INVENTORY_BYTES {
        return Err(());
    }
    Ok(bytes)
}

fn material_order(value: &Value) -> (&str, &str) {
    (
        value["subject_key"]
            .as_str()
            .expect("compiled material always has a subject key"),
        value["relative_path"]
            .as_str()
            .expect("compiled material always has a relative path"),
    )
}

fn compile_materials(
    review_root: &Path,
    subject: &SubjectSelection,
    output: &mut Vec<Value>,
    total_bytes: &mut usize,
    material_ids: &mut BTreeSet<String>,
) -> Result<Vec<Digest>, ()> {
    if subject.materials.is_empty() {
        return Err(());
    }
    let mut previous = None;
    let mut ids = Vec::with_capacity(subject.materials.len());
    for selection in &subject.materials {
        if previous
            .as_deref()
            .is_some_and(|value: &str| value >= selection.relative_path.as_str())
        {
            return Err(());
        }
        previous = Some(selection.relative_path.clone());
        let material = material(review_root, &subject.subject_key, selection)?;
        *total_bytes = total_bytes
            .checked_add(material.content.len())
            .filter(|value| *value <= MAXIMUM_TOTAL_MATERIAL_BYTES)
            .ok_or(())?;
        let id = material_id(&material);
        if !material_ids.insert(id.as_str().to_owned()) {
            return Err(());
        }
        ids.push(id);
        output.push(material.value());
        if output.len() > MAXIMUM_MATERIALS {
            return Err(());
        }
    }
    ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    Ok(ids)
}

fn material(
    review_root: &Path,
    subject_key: &str,
    selection: &MaterialSelection,
) -> Result<Material, ()> {
    ArtifactSetRelativePath::new(&selection.relative_path).map_err(|_| ())?;
    let source_path =
        ArtifactSetRelativePath::new(format!("{MATERIAL_ROOT}/{}", selection.source_path))
            .map_err(|_| ())?;
    let bytes = read_relative(review_root, &source_path, MAXIMUM_MATERIAL_BYTES)?;
    let content = String::from_utf8(bytes).map_err(|_| ())?;
    let byte_size = u64::try_from(content.len()).map_err(|_| ())?;
    let digest = Digest::sha256(content.as_bytes());
    Ok(Material {
        byte_size,
        content,
        digest,
        kind: selection.kind,
        relative_path: selection.relative_path.clone(),
        subject_key: subject_key.to_owned(),
    })
}

fn validate_expression(expression: &str) -> Result<(), ()> {
    if expression.is_empty()
        || expression.len() > 512
        || expression.trim() != expression
        || !expression.is_ascii()
        || expression
            .bytes()
            .any(|byte| !(0x20..=0x7e).contains(&byte))
    {
        Err(())
    } else {
        Ok(())
    }
}

fn expected_components(root: &Path) -> Result<Vec<ExpectedSubject<'_>>, ()> {
    COMPONENT_SPECS
        .iter()
        .filter(|specification| specification.relative_path != INVENTORY_PATH)
        .map(|specification| {
            let path = ArtifactSetRelativePath::new(specification.relative_path).map_err(|_| ())?;
            let measurement = measure_relative(root, &path, MAXIMUM_COMPONENT_BYTES)?;
            Ok(ExpectedSubject::Component {
                measurement,
                specification,
            })
        })
        .collect()
}

fn expected_cargo_packages(root: &Path) -> Result<Vec<ExpectedSubject<'_>>, ()> {
    let path = ArtifactSetRelativePath::new("lineage/source/Cargo.lock").map_err(|_| ())?;
    let lock_bytes = read_relative(root, &path, MAXIMUM_LOCK_BYTES)?;
    let lock_text = std::str::from_utf8(&lock_bytes).map_err(|_| ())?;
    let lockfile = Lockfile::from_str(lock_text).map_err(|_| ())?;
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
        return Err(());
    }
    Ok(packages)
}

enum ExpectedSubject<'a> {
    Component {
        measurement: Measurement,
        specification: &'a ComponentSpec,
    },
    Cargo {
        checksum: Option<String>,
        name: String,
        source: Option<String>,
        version: String,
    },
}

impl ExpectedSubject<'_> {
    fn key(&self) -> String {
        match self {
            Self::Component { specification, .. } => {
                format!("component:{}", specification.relative_path)
            }
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

    fn value(&self, expression: &str, ids: &[Digest]) -> Value {
        match self {
            Self::Component {
                measurement,
                specification,
            } => json!({
                "byte_size": measurement.byte_size,
                "declared_spdx_expression": expression,
                "digest": measurement.digest,
                "kind": "component",
                "material_ids": ids,
                "name": specification.name,
                "relative_path": specification.relative_path,
                "revision": specification.revision,
                "roles": specification.roles,
                "source_locator": specification.source_locator
            }),
            Self::Cargo {
                checksum,
                name,
                source,
                version,
            } => json!({
                "checksum": checksum,
                "declared_spdx_expression": expression,
                "kind": "cargo_package",
                "material_ids": ids,
                "name": name,
                "source": source,
                "version": version
            }),
        }
    }

    fn review_identity(&self) -> Digest {
        let value = match self {
            Self::Component {
                measurement,
                specification,
            } => json!({
                "byte_size": measurement.byte_size,
                "digest": measurement.digest,
                "kind": "component",
                "name": specification.name,
                "relative_path": specification.relative_path,
                "revision": specification.revision,
                "roles": specification.roles,
                "source_locator": specification.source_locator
            }),
            Self::Cargo {
                checksum,
                name,
                source,
                version,
            } => json!({
                "checksum": checksum,
                "kind": "cargo_package",
                "name": name,
                "source": source,
                "version": version
            }),
        };
        let bytes = serde_json::to_vec(&value).expect("expected-subject JSON is infallible");
        sequence_digest(
            b"retained-program-license-review-subject/v1",
            [bytes.as_slice()],
        )
    }
}

struct Material {
    byte_size: u64,
    content: String,
    digest: Digest,
    kind: MaterialKind,
    relative_path: String,
    subject_key: String,
}

impl Material {
    fn value(&self) -> Value {
        json!({
            "byte_size": self.byte_size,
            "content": self.content,
            "digest": self.digest,
            "kind": self.kind,
            "relative_path": self.relative_path,
            "subject_key": self.subject_key
        })
    }
}

fn material_id(material: &Material) -> Digest {
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

#[cfg(test)]
#[path = "license_inventory/tests.rs"]
mod tests;
