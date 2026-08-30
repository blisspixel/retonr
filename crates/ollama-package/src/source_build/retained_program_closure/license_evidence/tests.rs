use std::{collections::BTreeMap, io, io::Cursor};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_types::Digest;

use super::{
    INVENTORY_PATH, RetainedProgramLicenseEvidenceError, RetainedProgramLicenseEvidenceLimits,
    find_role, read_component,
    validation::{
        ClosureBindings, ExpectedSubject, expected_subjects, material_id, parse_inventory,
        verify_inventory, verify_subjects_and_materials,
    },
    wire::{InventoryStatus, InventoryWire, MaterialKind, MaterialWire, SubjectWire},
};
use crate::source_build::{
    RuntimeSourceBuildAcceleratorPolicy, RuntimeSourceBuildInputComponent,
    RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputOpenError, RuntimeSourceBuildInputRole,
    RuntimeSourceBuildNetworkPolicy, RuntimeSourceBuildPolicy, VerifiedRuntimeSourceBuildInputs,
};

const SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

#[path = "tests/adversarial.rs"]
mod adversarial;

struct Fixture {
    bindings: ClosureBindings,
    inventory: InventoryWire,
    lock: Vec<u8>,
    manifest: RuntimeSourceBuildInputManifest,
}

impl Fixture {
    fn valid() -> Self {
        let lock = lockfile();
        let provisional = manifest(&lock, b"placeholder inventory");
        let expected = expected_subjects(
            &provisional,
            &lock,
            RetainedProgramLicenseEvidenceLimits::default(),
        )
        .expect("subjects");
        let (subjects, materials) = inventory_subjects(&expected);
        let inventory = InventoryWire {
            materials,
            schema_version: 2,
            status: InventoryStatus::PendingReview,
            subjects,
        };
        let inventory_bytes = canonical(&inventory);
        let manifest = manifest(&lock, &inventory_bytes);
        let bindings = ClosureBindings {
            cargo: Digest::sha256(b"cargo closure"),
            source_set: manifest.artifact_set().artifact_set_id().digest().clone(),
            upstream: Digest::sha256(b"upstream closure"),
        };
        Self {
            bindings,
            inventory,
            lock,
            manifest,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        canonical(&self.inventory)
    }

    fn verify(
        &self,
    ) -> Result<
        super::VerifiedRetainedProgramLicenseEvidenceClosure,
        RetainedProgramLicenseEvidenceError,
    > {
        verify_inventory(
            &self.manifest,
            &self.bytes(),
            &self.lock,
            self.bindings.clone(),
            RetainedProgramLicenseEvidenceLimits::default(),
        )
    }
}

#[test]
fn complete_pending_evidence_returns_only_bound_reviewer_facts() {
    let fixture = Fixture::valid();
    let verified = fixture.verify().expect("complete evidence");
    assert_eq!(verified.source_input_set_id(), &fixture.bindings.source_set);
    assert_eq!(verified.cargo_source_closure_id(), &fixture.bindings.cargo);
    assert_eq!(verified.upstream_closure_id(), &fixture.bindings.upstream);
    assert_eq!(
        verified.evidence_digest(),
        &Digest::sha256(&fixture.bytes())
    );
    assert_ne!(verified.closure_id().digest(), verified.evidence_digest());
    let facts = verified.reviewer_facts();
    assert_eq!(facts.component_subject_count(), 2);
    assert_eq!(facts.cargo_package_subject_count(), 2);
    assert_eq!(facts.material_count(), 4);
    assert!(facts.material_byte_count() > 0);
    assert!(facts.human_adjudication_required());
}

#[test]
fn public_verifier_reopens_members_and_requires_one_shared_source_set() {
    let fixture = Fixture::valid();
    let source_set_id = fixture.manifest.artifact_set().artifact_set_id();
    let cargo = super::super::cargo_source::verified_cargo_source_closure_for_test(
        source_set_id.digest().clone(),
        Digest::sha256(&fixture.lock),
        fixture.bindings.cargo.clone(),
    );
    let upstream = super::super::VerifiedRetainedProgramUpstreamClosure {
        closure_id: super::super::RetainedProgramUpstreamClosureId(
            fixture.bindings.upstream.clone(),
        ),
        source_input_set_id: source_set_id,
        rust: super::super::VerifiedRustReleaseUpstream {
            closure_id: Digest::sha256(b"test Rust upstream"),
        },
        alpine: super::super::VerifiedAlpineReleaseUpstream {
            closure_id: Digest::sha256(b"test Alpine upstream"),
        },
    };
    let inputs = VerifiedRuntimeSourceBuildInputs {
        manifest: fixture.manifest.clone(),
        program_lineage: None,
    };
    let members = BTreeMap::from([
        (INVENTORY_PATH.to_owned(), fixture.bytes()),
        ("lineage/source/Cargo.lock".to_owned(), fixture.lock.clone()),
    ]);
    let verified = super::verify_retained_program_license_evidence_closure(
        &inputs,
        &cargo,
        &upstream,
        RetainedProgramLicenseEvidenceLimits::default(),
        |path| {
            members
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimeSourceBuildInputOpenError)
        },
        || false,
    )
    .expect("public verifier");
    assert_eq!(
        verified.closure_id().digest(),
        fixture.verify().expect("view").closure_id().digest()
    );

    let wrong_cargo = super::super::cargo_source::verified_cargo_source_closure_for_test(
        Digest::sha256(b"wrong source set"),
        Digest::sha256(&fixture.lock),
        fixture.bindings.cargo,
    );
    assert_eq!(
        super::verify_retained_program_license_evidence_closure::<Cursor<Vec<u8>>, _, _>(
            &inputs,
            &wrong_cargo,
            &upstream,
            RetainedProgramLicenseEvidenceLimits::default(),
            |_path| Err(RuntimeSourceBuildInputOpenError),
            || false,
        ),
        Err(RetainedProgramLicenseEvidenceError::ClosureBindingMismatch)
    );
}

#[test]
fn legacy_unknown_ambiguous_and_noncanonical_json_fail_closed() {
    let limits = RetainedProgramLicenseEvidenceLimits::default();
    assert_eq!(
        parse_inventory(
            br#"{"components":[],"schema_version":1,"status":"pending_review"}"#,
            limits
        ),
        Err(RetainedProgramLicenseEvidenceError::LegacyEvidenceIncomplete)
    );
    assert_eq!(
        parse_inventory(br#"{"schema_version":3}"#, limits),
        Err(RetainedProgramLicenseEvidenceError::UnsupportedSchema)
    );
    let fixture = Fixture::valid();
    let mut spaced = fixture.bytes();
    spaced.push(b'\n');
    assert_eq!(
        parse_inventory(&spaced, limits),
        Err(RetainedProgramLicenseEvidenceError::NoncanonicalInventory)
    );
    assert_eq!(
        parse_inventory(br#"{"schema_version":2,"schema_version":2}"#, limits),
        Err(RetainedProgramLicenseEvidenceError::NoncanonicalInventory)
    );
    let mut value = serde_json::to_value(&fixture.inventory).expect("value");
    value["unknown"] = serde_json::json!(true);
    assert_eq!(
        parse_inventory(&serde_json::to_vec(&value).expect("json"), limits),
        Err(RetainedProgramLicenseEvidenceError::InvalidInventory)
    );
}

#[test]
fn limits_reject_relaxation_and_bound_inventory_subjects_materials_and_strings() {
    let invalid = RetainedProgramLicenseEvidenceLimits {
        maximum_subjects: 0,
        ..RetainedProgramLicenseEvidenceLimits::default()
    };
    assert_eq!(
        invalid.validate(),
        Err(RetainedProgramLicenseEvidenceError::InvalidLimits)
    );
    let fixture = Fixture::valid();
    let inventory_limit = RetainedProgramLicenseEvidenceLimits {
        maximum_inventory_bytes: 1,
        ..RetainedProgramLicenseEvidenceLimits::default()
    };
    assert_eq!(
        parse_inventory(&fixture.bytes(), inventory_limit),
        Err(RetainedProgramLicenseEvidenceError::QuotaExceeded)
    );
    for limits in [
        RetainedProgramLicenseEvidenceLimits {
            maximum_subjects: 1,
            ..RetainedProgramLicenseEvidenceLimits::default()
        },
        RetainedProgramLicenseEvidenceLimits {
            maximum_materials: 1,
            ..RetainedProgramLicenseEvidenceLimits::default()
        },
        RetainedProgramLicenseEvidenceLimits {
            maximum_total_material_bytes: 1,
            maximum_material_bytes: 1,
            ..RetainedProgramLicenseEvidenceLimits::default()
        },
        RetainedProgramLicenseEvidenceLimits {
            maximum_string_bytes: 1,
            ..RetainedProgramLicenseEvidenceLimits::default()
        },
    ] {
        let parsed = parse_inventory(&fixture.bytes(), limits).expect("inventory parses");
        let expected = expected_subjects(&fixture.manifest, &fixture.lock, limits);
        if let Ok(expected) = expected {
            assert!(verify_subjects_and_materials(&parsed, &expected, limits).is_err());
        }
    }
}

#[test]
fn component_reading_rejects_open_read_cancel_size_and_digest_drift() {
    let bytes = b"evidence".to_vec();
    let component = component(
        INVENTORY_PATH,
        &bytes,
        RuntimeSourceBuildInputRole::LicenseEvidence,
    );
    assert_eq!(
        read_component::<Cursor<Vec<u8>>, _, _>(
            &component,
            1024,
            &mut |_path| Err(RuntimeSourceBuildInputOpenError),
            &mut || false,
        ),
        Err(RetainedProgramLicenseEvidenceError::ComponentUnavailable)
    );
    assert_eq!(
        read_component(&component, 1024, &mut |_path| Ok(ReadError), &mut || false,),
        Err(RetainedProgramLicenseEvidenceError::ComponentRead)
    );
    assert_eq!(
        read_component(
            &component,
            1024,
            &mut |_path| Ok(Cursor::new(bytes.clone())),
            &mut || true,
        ),
        Err(RetainedProgramLicenseEvidenceError::Cancelled)
    );
    for changed in [b"short".to_vec(), b"evidence-extra".to_vec()] {
        assert_eq!(
            read_component(
                &component,
                1024,
                &mut |_path| Ok(Cursor::new(changed.clone())),
                &mut || false,
            ),
            Err(RetainedProgramLicenseEvidenceError::ComponentMeasurementMismatch)
        );
    }
}

#[test]
fn role_lookup_and_lock_subject_derivation_fail_closed() {
    let fixture = Fixture::valid();
    assert!(
        find_role(
            &fixture.manifest,
            RuntimeSourceBuildInputRole::LicenseEvidence
        )
        .is_ok()
    );
    assert_eq!(
        find_role(&fixture.manifest, RuntimeSourceBuildInputRole::GoToolchain),
        Err(RetainedProgramLicenseEvidenceError::InvalidComponentRoles)
    );
    assert_eq!(
        expected_subjects(
            &fixture.manifest,
            b"not a lock",
            RetainedProgramLicenseEvidenceLimits::default()
        ),
        Err(RetainedProgramLicenseEvidenceError::IncompleteSubjectSet)
    );
}

fn inventory_subjects(expected: &[ExpectedSubject]) -> (Vec<SubjectWire>, Vec<MaterialWire>) {
    let mut subjects = Vec::new();
    let mut materials = Vec::new();
    for (index, expected) in expected.iter().enumerate() {
        let key = expected.key();
        let content = format!("license material for {key}\n");
        let material = MaterialWire {
            byte_size: content.len() as u64,
            content,
            digest: Digest::sha256(format!("license material for {key}\n").as_bytes()),
            kind: MaterialKind::LicenseText,
            relative_path: format!("captured/{index}/LICENSE"),
            subject_key: key,
        };
        let ids = vec![material_id(&material)];
        let subject = match expected {
            ExpectedSubject::Component {
                byte_size,
                digest,
                name,
                path,
                revision,
                roles,
                source_locator,
            } => SubjectWire::Component {
                byte_size: *byte_size,
                declared_spdx_expression: "NOASSERTION".to_owned(),
                digest: digest.clone(),
                material_ids: ids,
                name: name.clone(),
                relative_path: path.as_str().to_owned(),
                revision: revision.clone(),
                roles: roles.clone(),
                source_locator: source_locator.clone(),
            },
            ExpectedSubject::Cargo {
                checksum,
                name,
                source,
                version,
            } => SubjectWire::CargoPackage {
                checksum: checksum.clone(),
                declared_spdx_expression: "NOASSERTION".to_owned(),
                material_ids: ids,
                name: name.clone(),
                source: source.clone(),
                version: version.clone(),
            },
        };
        subjects.push(subject);
        materials.push(material);
    }
    materials.sort_by(|left, right| {
        (&left.subject_key, &left.relative_path).cmp(&(&right.subject_key, &right.relative_path))
    });
    (subjects, materials)
}

fn manifest(lock: &[u8], evidence: &[u8]) -> RuntimeSourceBuildInputManifest {
    let components = vec![
        component(
            INVENTORY_PATH,
            evidence,
            RuntimeSourceBuildInputRole::LicenseEvidence,
        ),
        component(
            "lineage/source/Cargo.lock",
            lock,
            RuntimeSourceBuildInputRole::CargoLockfile,
        ),
        component(
            "sources/code.tar",
            b"source archive",
            RuntimeSourceBuildInputRole::OllamaSource,
        ),
    ];
    let members = components
        .iter()
        .map(|component| {
            ArtifactSetMember::new(
                ArtifactId::from_digest(component.digest.clone()),
                component.byte_size,
                component.relative_path.clone(),
            )
        })
        .collect();
    RuntimeSourceBuildInputManifest {
        artifact_set: ArtifactSetManifest::new(members).expect("artifact set"),
        policy: policy(),
        components,
        manifest_digest: Digest::sha256(b"manifest"),
    }
}

fn component(
    path: &str,
    bytes: &[u8],
    role: RuntimeSourceBuildInputRole,
) -> RuntimeSourceBuildInputComponent {
    RuntimeSourceBuildInputComponent {
        relative_path: ArtifactSetRelativePath::new(path).expect("path"),
        byte_size: bytes.len() as u64,
        digest: Digest::sha256(bytes),
        name: format!("fixture {path}"),
        revision: "fixture-v1".to_owned(),
        source_locator: format!("urn:fixture:{path}"),
        roles: vec![role],
    }
}

fn policy() -> RuntimeSourceBuildPolicy {
    RuntimeSourceBuildPolicy {
        target: RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxMusl,
        )
        .expect("target"),
        network_access: RuntimeSourceBuildNetworkPolicy::Denied,
        accelerator: RuntimeSourceBuildAcceleratorPolicy::CpuOnly,
        cpu_feature_policy: "fixture".to_owned(),
        locale: "C".to_owned(),
        timezone: "UTC".to_owned(),
        source_date_epoch: 1,
        environment: Vec::new(),
        build_arguments: Vec::new(),
        environment_digest: Digest::sha256(b"environment"),
        build_arguments_digest: Digest::sha256(b"arguments"),
    }
}

fn lockfile() -> Vec<u8> {
    format!(
        "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"dep\"]\n\n[[package]]\nname = \"dep\"\nversion = \"1.0.0\"\nsource = \"{SOURCE}\"\nchecksum = \"{}\"\n",
        "a".repeat(64)
    )
    .into_bytes()
}

fn canonical<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let value = serde_json::to_value(value).expect("value");
    serde_json::to_vec(&value).expect("canonical JSON")
}

fn subject_materials_mut(subject: &mut SubjectWire) -> &mut Vec<Digest> {
    match subject {
        SubjectWire::Component { material_ids, .. }
        | SubjectWire::CargoPackage { material_ids, .. } => material_ids,
    }
}

fn set_expression(subject: &mut SubjectWire, value: &str) {
    match subject {
        SubjectWire::Component {
            declared_spdx_expression,
            ..
        }
        | SubjectWire::CargoPackage {
            declared_spdx_expression,
            ..
        } => *declared_spdx_expression = value.to_owned(),
    }
}

struct ReadError;

impl io::Read for ReadError {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("fixture read error"))
    }
}
