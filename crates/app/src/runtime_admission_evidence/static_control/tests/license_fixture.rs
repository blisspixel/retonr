use rewrite_ollama_package::RuntimeSourceBuildInputRole;
use rewrite_types::Digest;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::{StaticInputComponent, canonical, component};

pub(super) fn fixture_license_inputs(
    llama_digest: &Digest,
    ollama_digest: &Digest,
    source_provenance: &[u8],
    source_provenance_digest: &Digest,
    parameters_digest: &Digest,
    tools_digest: &Digest,
) -> (Vec<StaticInputComponent>, Vec<u8>, Vec<u8>, Vec<Value>) {
    let cargo_lock = cargo_lock();
    let mut components = fixture_components(
        llama_digest,
        ollama_digest,
        source_provenance,
        source_provenance_digest,
        parameters_digest,
        tools_digest,
        &cargo_lock,
    );
    let (license_inventory, subjects) = license_inventory(&components);
    components.push(component(
        "legal/licenses.json",
        license_inventory.len() as u64,
        Digest::sha256(&license_inventory),
        "review-v2",
        "licenses",
        vec![RuntimeSourceBuildInputRole::LicenseEvidence],
    ));
    (components, cargo_lock, license_inventory, subjects)
}

fn cargo_lock() -> Vec<u8> {
    concat!(
        "version = 4\n\n",
        "[[package]]\n",
        "name = \"fixture-dependency\"\n",
        "version = \"1.2.3\"\n",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        "checksum = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n\n",
        "[[package]]\n",
        "name = \"fixture-workspace\"\n",
        "version = \"0.1.0\"\n",
    )
    .as_bytes()
    .to_vec()
}

fn fixture_components(
    llama_digest: &Digest,
    ollama_digest: &Digest,
    source_provenance: &[u8],
    source_provenance_digest: &Digest,
    parameters_digest: &Digest,
    tools_digest: &Digest,
    cargo_lock: &[u8],
) -> Vec<StaticInputComponent> {
    vec![
        component(
            "lineage/source/Cargo.lock",
            cargo_lock.len() as u64,
            Digest::sha256(cargo_lock),
            "workspace-candidate-v1",
            "cargo-lock",
            vec![RuntimeSourceBuildInputRole::CargoLockfile],
        ),
        component(
            "sources/llama.tar",
            16,
            llama_digest.clone(),
            "llama-revision",
            "llama",
            vec![RuntimeSourceBuildInputRole::LlamaCppSource],
        ),
        component(
            "sources/ollama.tar",
            17,
            ollama_digest.clone(),
            "ollama-revision",
            "ollama",
            vec![RuntimeSourceBuildInputRole::OllamaSource],
        ),
        component(
            "metadata/source-provenance.json",
            source_provenance.len() as u64,
            source_provenance_digest.clone(),
            "v1",
            "provenance",
            vec![RuntimeSourceBuildInputRole::SourceProvenance],
        ),
        component(
            "patches/a.patch",
            7,
            Digest::sha256(b"patch-a"),
            "v1",
            "patch-a",
            vec![RuntimeSourceBuildInputRole::SourcePatch],
        ),
        component(
            "patches/b.patch",
            7,
            Digest::sha256(b"patch-b"),
            "v1",
            "patch-b",
            vec![RuntimeSourceBuildInputRole::SourcePatch],
        ),
        component(
            "metadata/parameters.json",
            5,
            parameters_digest.clone(),
            "v1",
            "parameters",
            vec![RuntimeSourceBuildInputRole::BuildParameters],
        ),
        component(
            "metadata/tools.json",
            5,
            tools_digest.clone(),
            "v1",
            "tools",
            vec![RuntimeSourceBuildInputRole::ToolEvidence],
        ),
    ]
}

fn license_inventory(components: &[StaticInputComponent]) -> (Vec<u8>, Vec<Value>) {
    let mut materials = Vec::new();
    let mut subjects = Vec::new();
    let mut reviews = Vec::new();
    for component in components {
        let key = format!("component:{}", component.relative_path.as_str());
        let (material, material_id) = license_material(&key);
        let subject = json!({
            "byte_size":component.byte_size,
            "declared_spdx_expression":"NOASSERTION",
            "digest":component.digest,
            "kind":"component",
            "material_ids":[material_id],
            "name":component.name,
            "relative_path":component.relative_path,
            "revision":component.revision,
            "roles":component.roles,
            "source_locator":component.source_locator
        });
        materials.push(material);
        reviews.push(license_subject_review(&key, &subject));
        subjects.push(subject);
    }
    push_cargo_subjects(&mut materials, &mut subjects, &mut reviews);
    materials.sort_by(|left, right| material_order(left).cmp(&material_order(right)));
    (
        canonical(&json!({
            "materials":materials,
            "schema_version":2,
            "status":"pending_review",
            "subjects":subjects
        })),
        reviews,
    )
}

fn push_cargo_subjects(
    materials: &mut Vec<Value>,
    subjects: &mut Vec<Value>,
    reviews: &mut Vec<Value>,
) {
    for (name, version, source, checksum) in [
        (
            "fixture-dependency",
            "1.2.3",
            Some("registry+https://github.com/rust-lang/crates.io-index"),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
        ("fixture-workspace", "0.1.0", None, None),
    ] {
        let key = format!("cargo:{name}@{version}:{}", source.unwrap_or("path"));
        let (material, material_id) = license_material(&key);
        let subject = json!({
            "checksum":checksum,
            "declared_spdx_expression":"NOASSERTION",
            "kind":"cargo_package",
            "material_ids":[material_id],
            "name":name,
            "source":source,
            "version":version
        });
        materials.push(material);
        reviews.push(license_subject_review(&key, &subject));
        subjects.push(subject);
    }
}

fn material_order(value: &Value) -> (&str, &str) {
    (
        value["subject_key"].as_str().expect("material key"),
        value["relative_path"].as_str().expect("material path"),
    )
}

fn license_material(subject_key: &str) -> (Value, Digest) {
    let content = format!("exact legal material for {subject_key}\n");
    let byte_size = content.len() as u64;
    let digest = Digest::sha256(content.as_bytes());
    let id = sequence_digest(
        b"retained-program-license-evidence/material/v1",
        [
            subject_key.as_bytes(),
            b"license_text",
            b"LICENSE",
            byte_size.to_string().as_bytes(),
            digest.as_str().as_bytes(),
        ],
    );
    (
        json!({
            "byte_size":byte_size,
            "content":content,
            "digest":digest,
            "kind":"license_text",
            "relative_path":"LICENSE",
            "subject_key":subject_key
        }),
        id,
    )
}

fn license_subject_review(subject_key: &str, subject: &Value) -> Value {
    let bytes = canonical(subject);
    let identity = sequence_digest(
        b"retonr:runtime-admission:license-inventory-subject:v2",
        [subject_key.as_bytes(), bytes.as_slice()],
    );
    json!({
        "disposition":"approved_build_only",
        "inventory_subject_id":identity,
        "subject_key":subject_key
    })
}

fn sequence_digest<'a>(domain: &[u8], values: impl IntoIterator<Item = &'a [u8]>) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for value in values {
        hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(value);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .expect("SHA-256 formatting is valid")
}
