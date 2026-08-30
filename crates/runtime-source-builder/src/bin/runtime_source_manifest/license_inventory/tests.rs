use std::fs;

use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

use super::*;

struct Fixture {
    components: TempDir,
    review: TempDir,
    expected_subjects: usize,
}

impl Fixture {
    fn new() -> Self {
        let components = tempdir().expect("components");
        for specification in COMPONENT_SPECS
            .iter()
            .filter(|specification| specification.relative_path != INVENTORY_PATH)
        {
            let path = components.path().join(specification.relative_path);
            fs::create_dir_all(path.parent().expect("component parent")).expect("component path");
            let content = if specification.relative_path == "lineage/source/Cargo.lock" {
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
            } else {
                format!("exact fixture bytes for {}\n", specification.relative_path).into_bytes()
            };
            fs::write(path, content).expect("component");
        }
        let review = tempdir().expect("review");
        let material = review.path().join("materials/shared/LICENSE");
        fs::create_dir_all(material.parent().expect("material parent")).expect("material path");
        fs::write(&material, b"Exact retained legal material.\n").expect("material");
        let mut expected = expected_components(components.path()).expect("components measure");
        expected.extend(expected_cargo_packages(components.path()).expect("lock parses"));
        let expected_subjects = expected.len();
        write_review(review.path(), &expected);
        Self {
            components,
            review,
            expected_subjects,
        }
    }

    fn review_value(&self) -> Value {
        let bytes = fs::read(self.review.path().join(REVIEW_PATH)).expect("review bytes");
        serde_json::from_slice(bytes.strip_suffix(b"\n").expect("final LF")).expect("review JSON")
    }

    fn replace_review(&self, value: &Value) {
        let mut bytes = serde_json::to_vec(value).expect("review JSON");
        bytes.push(b'\n');
        fs::write(self.review.path().join(REVIEW_PATH), bytes).expect("review write");
    }
}

fn write_review(root: &Path, expected: &[ExpectedSubject<'_>]) {
    let subjects = expected
        .iter()
        .map(|subject| {
            json!({
                "declared_spdx_expression": "NOASSERTION",
                "materials": [{
                    "kind": "license_text",
                    "relative_path": "LICENSE",
                    "source_path": "shared/LICENSE"
                }],
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
    .expect("review");
    bytes.push(b'\n');
    fs::write(root.join(REVIEW_PATH), bytes).expect("review file");
}

#[test]
fn compiles_exact_sorted_schema_two_inventory_deterministically() {
    let fixture = Fixture::new();
    let first = compile(fixture.components.path(), fixture.review.path()).expect("inventory");
    let second = compile(fixture.components.path(), fixture.review.path()).expect("inventory");
    assert_eq!(first, second);
    let value = serde_json::from_slice::<Value>(&first).expect("inventory JSON");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["status"], "pending_review");
    assert_eq!(
        value["subjects"].as_array().expect("subjects").len(),
        fixture.expected_subjects
    );
    assert_eq!(
        value["materials"].as_array().expect("materials").len(),
        fixture.expected_subjects
    );
    let referenced = value["subjects"]
        .as_array()
        .expect("subjects")
        .iter()
        .flat_map(|subject| {
            subject["material_ids"]
                .as_array()
                .expect("material IDs")
                .iter()
                .map(|id| id.as_str().expect("material ID").to_owned())
        })
        .collect::<BTreeSet<_>>();
    let mut previous = None;
    for material in value["materials"].as_array().expect("materials") {
        let key = format!(
            "{}\0{}",
            material["subject_key"].as_str().expect("subject key"),
            material["relative_path"].as_str().expect("relative path")
        );
        assert!(previous.as_ref().is_none_or(|prior: &String| prior < &key));
        previous = Some(key);
        let content = material["content"].as_str().expect("content");
        assert_eq!(
            material["byte_size"],
            u64::try_from(content.len()).expect("length")
        );
        assert_eq!(
            material["digest"],
            Digest::sha256(content.as_bytes()).as_str()
        );
        let kind = match material["kind"].as_str().expect("material kind") {
            "license_text" => MaterialKind::LicenseText,
            value => panic!("unexpected fixture material kind: {value}"),
        };
        let record = Material {
            byte_size: u64::try_from(content.len()).expect("length"),
            content: content.to_owned(),
            digest: Digest::sha256(content.as_bytes()),
            kind,
            relative_path: material["relative_path"]
                .as_str()
                .expect("relative path")
                .to_owned(),
            subject_key: material["subject_key"]
                .as_str()
                .expect("subject key")
                .to_owned(),
        };
        assert!(referenced.contains(material_id(&record).as_str()));
    }
    assert_eq!(referenced.len(), fixture.expected_subjects);
}

#[test]
fn exact_review_identity_rejects_component_drift_and_subject_reordering() {
    let fixture = Fixture::new();
    let changed = fixture.components.path().join("scripts/build");
    fs::write(&changed, b"changed exact component\n").expect("change component");
    assert!(compile(fixture.components.path(), fixture.review.path()).is_err());

    let fixture = Fixture::new();
    let mut review = fixture.review_value();
    review["subjects"]
        .as_array_mut()
        .expect("subjects")
        .swap(0, 1);
    fixture.replace_review(&review);
    assert!(compile(fixture.components.path(), fixture.review.path()).is_err());
}

#[test]
fn review_and_material_boundaries_fail_closed() {
    let fixture = Fixture::new();
    let mut review = fixture.review_value();
    review["subjects"][0]["declared_spdx_expression"] = json!(" MIT");
    fixture.replace_review(&review);
    assert!(compile(fixture.components.path(), fixture.review.path()).is_err());

    let fixture = Fixture::new();
    let mut review = fixture.review_value();
    review["subjects"][0]["materials"][0]["source_path"] = json!("../escape");
    fixture.replace_review(&review);
    assert!(compile(fixture.components.path(), fixture.review.path()).is_err());

    let fixture = Fixture::new();
    fs::write(
        fixture.review.path().join("materials/shared/LICENSE"),
        [0xff, 0xfe],
    )
    .expect("invalid UTF-8");
    assert!(compile(fixture.components.path(), fixture.review.path()).is_err());
}

#[test]
fn every_legal_material_kind_retains_exact_distinct_bytes() {
    let fixture = Fixture::new();
    let mut review = fixture.review_value();
    let selections = [
        ("attribution", "ATTRIBUTION"),
        ("license_exception", "EXCEPTION"),
        ("license_text", "LICENSE"),
        ("notice", "NOTICE"),
        ("other_legal_text", "OTHER"),
    ]
    .map(|(kind, name)| {
        let source_path = format!("kinds/{name}");
        let path = fixture.review.path().join(MATERIAL_ROOT).join(&source_path);
        fs::create_dir_all(path.parent().expect("kind parent")).expect("kind directory");
        fs::write(&path, format!("exact {kind} bytes\n")).expect("kind material");
        json!({
            "kind": kind,
            "relative_path": name,
            "source_path": source_path
        })
    });
    review["subjects"][0]["materials"] = json!(selections);
    fixture.replace_review(&review);
    let inventory = compile(fixture.components.path(), fixture.review.path()).expect("inventory");
    let value: Value = serde_json::from_slice(&inventory).expect("inventory JSON");
    let first_key = value["subjects"][0]["kind"].as_str().expect("kind");
    assert_eq!(first_key, "component");
    for kind in [
        "attribution",
        "license_exception",
        "license_text",
        "notice",
        "other_legal_text",
    ] {
        assert!(
            value["materials"]
                .as_array()
                .expect("materials")
                .iter()
                .any(|material| material["kind"] == kind)
        );
    }
}

#[test]
fn review_json_must_be_unique_compact_and_lf_terminated() {
    let fixture = Fixture::new();
    let path = fixture.review.path().join(REVIEW_PATH);
    let value = fixture.review_value();
    fs::write(&path, serde_json::to_vec_pretty(&value).expect("pretty")).expect("pretty review");
    assert!(compile(fixture.components.path(), fixture.review.path()).is_err());
    fs::write(
        &path,
        b"{\"schema_version\":1,\"schema_version\":1,\"status\":\"pending_review\",\"subjects\":[]}\n",
    )
    .expect("duplicate review");
    assert!(compile(fixture.components.path(), fixture.review.path()).is_err());
}

#[test]
fn template_and_inventory_publish_without_replacement() {
    let fixture = Fixture::new();
    let destination_root = tempdir().expect("destinations");
    let template = destination_root.path().join("review.json");
    write_template(fixture.components.path(), &template).expect("template");
    assert!(write_template(fixture.components.path(), &template).is_err());
    let template_value: Value = serde_json::from_slice(
        fs::read(&template)
            .expect("template bytes")
            .strip_suffix(b"\n")
            .expect("template LF"),
    )
    .expect("template JSON");
    assert_eq!(
        template_value["subjects"]
            .as_array()
            .expect("template subjects")
            .len(),
        fixture.expected_subjects
    );
    assert!(
        template_value["subjects"][0]["materials"]
            .as_array()
            .expect("materials")
            .is_empty()
    );

    write(fixture.components.path(), fixture.review.path()).expect("inventory publish");
    assert!(write(fixture.components.path(), fixture.review.path()).is_err());
    let inventory = fs::read(fixture.components.path().join(INVENTORY_PATH)).expect("inventory");
    assert_eq!(
        inventory,
        compile(fixture.components.path(), fixture.review.path()).expect("same")
    );
}
