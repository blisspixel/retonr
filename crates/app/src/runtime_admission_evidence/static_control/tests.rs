use std::{cell::Cell, collections::BTreeMap};

use rewrite_model::{ArtifactSetId, ArtifactSetRelativePath, RuntimePackageManifestId};
use rewrite_ollama_package::RuntimeSourceBuildInputRole;
use rewrite_types::{CancellationToken, Digest};
use serde_json::{Value, json};

mod cases;
mod license_fixture;

use super::{common::*, license, lineage, transformation};
use crate::runtime_admission_evidence::{
    RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceFoundationInput,
    binding::{
        RuntimeAdmissionFoundationBindingError, RuntimeAdmissionFoundationEvidenceView,
        RuntimeAdmissionFoundationSnapshot, verify_view as verify_foundation_view,
    },
};

struct Fixture {
    evidence: FakeEvidence,
    foundation: RuntimeAdmissionEvidenceFoundation,
    binding: crate::VerifiedRuntimeAdmissionFoundationBinding,
    lineage_review: Vec<u8>,
    transformation_review: Vec<u8>,
    license_review: Vec<u8>,
}

struct FakeEvidence {
    snapshot: RuntimeAdmissionFoundationSnapshot,
    facts: StaticEvidenceFacts,
    source: BTreeMap<String, Vec<u8>>,
    evidence: BTreeMap<String, Vec<u8>>,
    calls: Cell<usize>,
    fail_at: Cell<Option<usize>>,
}

impl RuntimeAdmissionFoundationEvidenceView for FakeEvidence {
    fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionFoundationBindingError> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if cancellation.is_cancelled() || self.fail_at.get() == Some(call) {
            Err(RuntimeAdmissionFoundationBindingError::InvalidBinding)
        } else {
            Ok(())
        }
    }

    fn snapshot(
        &self,
    ) -> Result<RuntimeAdmissionFoundationSnapshot, RuntimeAdmissionFoundationBindingError> {
        Ok(self.snapshot.clone())
    }
}

impl RuntimeAdmissionStaticEvidenceView for FakeEvidence {
    fn facts(&self) -> StaticEvidenceFacts {
        self.facts.clone()
    }

    fn read_source_input(
        &self,
        path: &ArtifactSetRelativePath,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError> {
        self.source
            .get(path.as_str())
            .cloned()
            .ok_or(RuntimeAdmissionStaticControlError::EvidenceInsufficient)
    }

    fn read_evidence(
        &self,
        path: &ArtifactSetRelativePath,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError> {
        self.evidence
            .get(path.as_str())
            .cloned()
            .ok_or(RuntimeAdmissionStaticControlError::EvidenceInsufficient)
    }
}

fn compile_lineage(
    fixture: &Fixture,
    cancellation: &CancellationToken,
) -> Result<lineage::CompiledRuntimeAdmissionSourceLineageControl, RuntimeAdmissionStaticControlError>
{
    let review = parse_canonical(
        &fixture.lineage_review,
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )?;
    lineage::compile_view(&fixture.binding, &fixture.evidence, review, cancellation)
}

fn compile_transformation(
    fixture: &Fixture,
    cancellation: &CancellationToken,
) -> Result<
    transformation::CompiledRuntimeAdmissionTransformationControl,
    RuntimeAdmissionStaticControlError,
> {
    let review = parse_canonical(
        &fixture.transformation_review,
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )?;
    transformation::compile_view(&fixture.binding, &fixture.evidence, review, cancellation)
}

fn compile_license(
    fixture: &Fixture,
    cancellation: &CancellationToken,
) -> Result<license::CompiledRuntimeAdmissionLicenseControl, RuntimeAdmissionStaticControlError> {
    let review = parse_canonical(
        &fixture.license_review,
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )?;
    license::compile_view(&fixture.binding, &fixture.evidence, review, cancellation)
}

fn fixture() -> Fixture {
    let source_bundle = ArtifactSetId::from_digest(Digest::sha256(b"source bundle"));
    let source_inputs = ArtifactSetId::from_digest(Digest::sha256(b"source inputs"));
    let source_manifest = Digest::sha256(b"source manifest");
    let build_plan = Digest::sha256(b"build plan");
    let source_report = Digest::sha256(b"source report");
    let runtime_package = runtime_id(&Digest::sha256(b"runtime package"));
    let foundation = compile_foundation(
        &source_bundle,
        &source_inputs,
        &source_manifest,
        &build_plan,
        &source_report,
        &runtime_package,
    );
    let snapshot = RuntimeAdmissionFoundationSnapshot {
        source_build_evidence_bundle_id: source_bundle,
        source_build_inputs_id: source_inputs.clone(),
        source_manifest_digest: source_manifest,
        build_plan_digest: build_plan,
        source_report_digest: source_report,
        runtime_package_manifest_id: runtime_package.clone(),
    };
    let ollama_digest = Digest::sha256(b"ollama normalized");
    let llama_digest = Digest::sha256(b"llama normalized");
    let source_provenance = canonical(&json!({
        "llama_cpp": provenance_source("llama", &llama_digest, "llama-revision", "llama-tag"),
        "ollama": provenance_source("ollama", &ollama_digest, "ollama-revision", "ollama-tag"),
        "retained_source_patches": [
            {"name":"patch-a","normalized_component":{"byte_size":7,"digest":Digest::sha256(b"patch-a")},"target_revision":"ollama-revision"},
            {"name":"patch-b","normalized_component":{"byte_size":7,"digest":Digest::sha256(b"patch-b")},"target_revision":"llama-revision"}
        ],
        "schema_version": 1
    }));
    let source_provenance_digest = Digest::sha256(&source_provenance);
    let builder_digest = Digest::sha256(b"builder");
    let build_provenance = canonical(&json!({
        "build_revision":"ollama-revision", "builder_digest":builder_digest,
        "reported_version":"fixture-v1", "schema_version":1,
        "source_inputs_id":source_inputs, "source_provenance_digest":source_provenance_digest,
        "target":"x86_64-linux-gnu"
    }));
    let parameters_digest = Digest::sha256(b"parameters");
    let tools_digest = Digest::sha256(b"tools");
    let transformation_bytes = canonical(&json!({
        "accelerator":"cpu_only",
        "build_steps":["extract-frozen-inputs","verify-self-contained-tools","apply-ollama-compatibility-patch","configure-llama-cpp-cpu","build-llama-cpp-cpu","install-llama-cpp-cpu","build-ollama-go-entrypoint","assemble-runtime-package"],
        "network_access":"denied", "parameters_digest":parameters_digest, "schema_version":1,
        "source_inputs_id":source_inputs, "tool_evidence_digest":tools_digest
    }));
    let license_a = license_member("legal/a.txt", b"license a");
    let license_b = license_member("legal/b.txt", b"license bb");
    let (components, cargo_lock, license_inventory, license_subject_reviews) =
        license_fixture::fixture_license_inputs(
            &llama_digest,
            &ollama_digest,
            &source_provenance,
            &source_provenance_digest,
            &parameters_digest,
            &tools_digest,
        );
    let facts = StaticEvidenceFacts {
        components,
        build_program_path: path("scripts/build"),
        build_program_digest: builder_digest.clone(),
        byte_identical: true,
        runtime_package_manifest_id: runtime_package,
        license_members: vec![license_a.clone(), license_b.clone()],
    };
    let (lineage_review, transformation_review, license_review) = fixture_reviews(
        &ollama_digest,
        &llama_digest,
        &builder_digest,
        &license_a,
        &license_b,
        &license_inventory,
        &license_subject_reviews,
    );
    let fake = fake_evidence(
        snapshot,
        facts,
        source_provenance,
        license_inventory,
        cargo_lock,
        build_provenance,
        transformation_bytes,
    );
    let binding = verify_foundation_view(&foundation, &fake, &CancellationToken::new())
        .expect("verify foundation");
    fake.calls.set(0);
    Fixture {
        evidence: fake,
        foundation,
        binding,
        lineage_review,
        transformation_review,
        license_review,
    }
}

fn compile_foundation(
    source_bundle: &ArtifactSetId,
    source_inputs: &ArtifactSetId,
    source_manifest: &Digest,
    build_plan: &Digest,
    source_report: &Digest,
    runtime_package: &RuntimePackageManifestId,
) -> RuntimeAdmissionEvidenceFoundation {
    RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
        source_bundle.clone(),
        source_inputs.clone(),
        source_manifest.clone(),
        build_plan.clone(),
        source_report.clone(),
        runtime_package.clone(),
    ))
    .expect("compile foundation")
}

fn fake_evidence(
    snapshot: RuntimeAdmissionFoundationSnapshot,
    facts: StaticEvidenceFacts,
    source_provenance: Vec<u8>,
    license_inventory: Vec<u8>,
    cargo_lock: Vec<u8>,
    build_provenance: Vec<u8>,
    transformation: Vec<u8>,
) -> FakeEvidence {
    FakeEvidence {
        snapshot,
        facts,
        source: BTreeMap::from([
            ("lineage/source/Cargo.lock".to_owned(), cargo_lock),
            (
                "metadata/source-provenance.json".to_owned(),
                source_provenance,
            ),
            ("legal/licenses.json".to_owned(), license_inventory),
        ]),
        evidence: BTreeMap::from([
            (
                "attempts/primary/provenance.json".to_owned(),
                build_provenance.clone(),
            ),
            (
                "attempts/rebuild/provenance.json".to_owned(),
                build_provenance,
            ),
            (
                "attempts/primary/transformation.json".to_owned(),
                transformation.clone(),
            ),
            (
                "attempts/rebuild/transformation.json".to_owned(),
                transformation,
            ),
        ]),
        calls: Cell::new(0),
        fail_at: Cell::new(None),
    }
}

fn fixture_reviews(
    ollama_digest: &Digest,
    llama_digest: &Digest,
    builder_digest: &Digest,
    license_a: &StaticLicenseMember,
    license_b: &StaticLicenseMember,
    license_inventory: &[u8],
    license_subjects: &[Value],
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let lineage_review = canonical(&json!({
        "disposition":"approved", "procedure_id":lineage::RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_ID,
        "procedure_version":1, "schema_version":1,
        "sources":[
            reviewed_source("ollama", ollama_digest, "ollama-revision", "ollama-tag", "ollama_source"),
            reviewed_source("llama", llama_digest, "llama-revision", "llama-tag", "llama_cpp_source")
        ]
    }));
    let transformation_review = canonical(&json!({
        "approved_build_program_digest":builder_digest,
        "approved_patch_digests":[Digest::sha256(b"patch-a"),Digest::sha256(b"patch-b")],
        "disposition":"approved", "procedure_id":transformation::RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_ID,
        "procedure_version":1, "schema_version":1
    }));
    let license_review = canonical(&json!({
        "disposition":"approved",
        "inventory_digest":Digest::sha256(license_inventory),
        "procedure_id":license::RUNTIME_ADMISSION_LICENSE_PROCEDURE_ID,
        "procedure_version":2,
        "runtime_license_members":[license_value(license_a),license_value(license_b)],
        "schema_version":2,
        "subjects":license_subjects
    }));
    (lineage_review, transformation_review, license_review)
}

fn fake_from(
    original: &FakeEvidence,
    snapshot: RuntimeAdmissionFoundationSnapshot,
) -> FakeEvidence {
    FakeEvidence {
        snapshot,
        facts: original.facts.clone(),
        source: original.source.clone(),
        evidence: original.evidence.clone(),
        calls: Cell::new(0),
        fail_at: Cell::new(None),
    }
}

fn component(
    relative_path: &str,
    byte_size: u64,
    digest: Digest,
    revision: &str,
    source_locator: &str,
    roles: Vec<RuntimeSourceBuildInputRole>,
) -> StaticInputComponent {
    StaticInputComponent {
        relative_path: path(relative_path),
        byte_size,
        digest,
        name: relative_path.to_owned(),
        revision: revision.to_owned(),
        source_locator: source_locator.to_owned(),
        roles,
    }
}

fn provenance_source(locator: &str, digest: &Digest, revision: &str, tag: &str) -> Value {
    json!({
        "acquired_archive":{"byte_size":100,"digest":Digest::sha256(format!("{locator}-acquired").as_bytes()),"locator":locator},
        "normalized_component":{"byte_size":if locator == "ollama" {17} else {16},"digest":digest},
        "revision":revision, "upstream_tag":tag
    })
}

fn reviewed_source(locator: &str, digest: &Digest, revision: &str, tag: &str, role: &str) -> Value {
    json!({
        "acquired_archive_byte_size":100,
        "acquired_archive_digest":Digest::sha256(format!("{locator}-acquired").as_bytes()),
        "normalized_component_byte_size":if locator == "ollama" {17} else {16},
        "normalized_component_digest":digest, "revision":revision, "role":role,
        "source_locator":locator, "upstream_tag":tag
    })
}

fn license_member(relative_path: &str, bytes: &[u8]) -> StaticLicenseMember {
    StaticLicenseMember {
        relative_path: path(relative_path),
        byte_size: bytes.len() as u64,
        digest: Digest::sha256(bytes),
    }
}

fn license_value(member: &StaticLicenseMember) -> Value {
    json!({"byte_size":member.byte_size,"digest":member.digest,"relative_path":member.relative_path})
}

fn update_component_digest(
    evidence: &mut FakeEvidence,
    role: RuntimeSourceBuildInputRole,
    bytes: &[u8],
) {
    let component = evidence
        .facts
        .components
        .iter_mut()
        .find(|component| component.roles.contains(&role))
        .expect("fixture role");
    component.byte_size = bytes.len() as u64;
    component.digest = Digest::sha256(bytes);
}

fn replace_license_inventory(fixture: &mut Fixture, value: &Value) {
    let bytes = canonical(value);
    fixture
        .evidence
        .source
        .insert("legal/licenses.json".to_owned(), bytes.clone());
    update_component_digest(
        &mut fixture.evidence,
        RuntimeSourceBuildInputRole::LicenseEvidence,
        &bytes,
    );
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value.to_owned()).expect("fixture path")
}

fn runtime_id(digest: &Digest) -> RuntimePackageManifestId {
    serde_json::from_value(json!(digest)).expect("fixture package id")
}

fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("fixture JSON")
}

fn assert_domain_separated(identity: &Digest, bytes: &[u8]) {
    assert_ne!(identity, &Digest::sha256(bytes));
}
