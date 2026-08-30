use super::*;

#[test]
fn verified_foundation_rederives_every_subject() {
    let fixture = fixture();
    assert_eq!(
        fixture.binding.foundation_id(),
        fixture.foundation.foundation_id()
    );
    for index in 0..6 {
        let mut snapshot = fixture.evidence.snapshot.clone();
        let changed = Digest::sha256(format!("changed-{index}").as_bytes());
        match index {
            0 => snapshot.source_build_evidence_bundle_id = ArtifactSetId::from_digest(changed),
            1 => snapshot.source_build_inputs_id = ArtifactSetId::from_digest(changed),
            2 => snapshot.source_manifest_digest = changed,
            3 => snapshot.build_plan_digest = changed,
            4 => snapshot.source_report_digest = changed,
            5 => snapshot.runtime_package_manifest_id = runtime_id(&changed),
            _ => unreachable!(),
        }
        let changed = fake_from(&fixture.evidence, snapshot);
        assert!(matches!(
            verify_foundation_view(&fixture.foundation, &changed, &CancellationToken::new()),
            Err(RuntimeAdmissionFoundationBindingError::InvalidBinding)
        ));
    }
}

#[test]
fn all_three_controls_compile_then_independently_verify() {
    let fixture = fixture();
    let cancellation = CancellationToken::new();
    let lineage = compile_lineage(&fixture, &cancellation).expect("compile lineage");
    let transformation =
        compile_transformation(&fixture, &cancellation).expect("compile transformation");
    let license = compile_license(&fixture, &cancellation).expect("compile license");
    let verified_lineage = lineage::verify_view(
        lineage.canonical_bytes(),
        &fixture.binding,
        &fixture.evidence,
        &cancellation,
    )
    .expect("verify lineage");
    let verified_transformation = transformation::verify_view(
        transformation.canonical_bytes(),
        &fixture.binding,
        &fixture.evidence,
        &cancellation,
    )
    .expect("verify transformation");
    let verified_license = license::verify_view(
        license.canonical_bytes(),
        &fixture.binding,
        &fixture.evidence,
        &cancellation,
    )
    .expect("verify license");
    assert_eq!(verified_lineage.control_id(), lineage.control_id());
    assert_eq!(
        verified_transformation.control_id(),
        transformation.control_id()
    );
    assert_eq!(verified_license.control_id(), license.control_id());
    assert_domain_separated(lineage.control_id().digest(), lineage.canonical_bytes());
    assert_domain_separated(
        transformation.control_id().digest(),
        transformation.canonical_bytes(),
    );
    assert_domain_separated(license.control_id().digest(), license.canonical_bytes());
}

#[test]
fn control_encodings_are_exact_golden_and_bound_to_foundation() {
    let fixture = fixture();
    let cancellation = CancellationToken::new();
    let controls = [
        compile_lineage(&fixture, &cancellation)
            .expect("compile lineage")
            .canonical_bytes()
            .to_vec(),
        compile_transformation(&fixture, &cancellation)
            .expect("compile transformation")
            .canonical_bytes()
            .to_vec(),
        compile_license(&fixture, &cancellation)
            .expect("compile license")
            .canonical_bytes()
            .to_vec(),
    ];
    for (bytes, schema_version) in controls.into_iter().zip([1, 1, 2]) {
        let value: Value = serde_json::from_slice(&bytes).expect("control JSON");
        assert_eq!(serde_json::to_vec(&value).expect("canonical JSON"), bytes);
        assert_eq!(value["schema_version"], json!(schema_version));
        assert_eq!(value["status"], json!("passed"));
        assert_eq!(
            value["foundation_id"],
            json!(fixture.binding.foundation_id().digest())
        );
    }
}

#[test]
fn parser_rejects_noncanonical_unknown_and_rebound_control_bytes() {
    let fixture = fixture();
    let cancellation = CancellationToken::new();
    let compiled = compile_lineage(&fixture, &cancellation).expect("compile lineage");
    let mut spaced = compiled.canonical_bytes().to_vec();
    spaced.insert(1, b' ');
    assert!(matches!(
        lineage::verify_view(&spaced, &fixture.binding, &fixture.evidence, &cancellation),
        Err(RuntimeAdmissionStaticControlError::InvalidEncoding)
    ));
    let mut unknown: Value = serde_json::from_slice(compiled.canonical_bytes()).expect("JSON");
    unknown["unknown"] = json!(true);
    assert!(
        lineage::verify_view(
            &serde_json::to_vec(&unknown).expect("encode"),
            &fixture.binding,
            &fixture.evidence,
            &cancellation,
        )
        .is_err()
    );
    let mut rebound: Value = serde_json::from_slice(compiled.canonical_bytes()).expect("JSON");
    rebound["foundation_id"] = json!(Digest::sha256(b"other foundation"));
    assert!(matches!(
        lineage::verify_view(
            &serde_json::to_vec(&rebound).expect("encode"),
            &fixture.binding,
            &fixture.evidence,
            &cancellation,
        ),
        Err(RuntimeAdmissionStaticControlError::InvalidBinding)
    ));
}

#[test]
fn lineage_rejects_revision_and_review_digest_mutations() {
    let mut mutated = fixture();
    let cancellation = CancellationToken::new();
    let original = mutated
        .evidence
        .source
        .get("metadata/source-provenance.json")
        .expect("source provenance");
    let mut provenance: Value = serde_json::from_slice(original).expect("JSON");
    provenance["ollama"]["revision"] = json!("wrong-revision");
    let bytes = serde_json::to_vec(&provenance).expect("encode");
    mutated
        .evidence
        .source
        .insert("metadata/source-provenance.json".to_owned(), bytes.clone());
    update_component_digest(
        &mut mutated.evidence,
        RuntimeSourceBuildInputRole::SourceProvenance,
        &bytes,
    );
    assert!(compile_lineage(&mutated, &cancellation).is_err());

    let fixture = fixture();
    let mut review: Value = serde_json::from_slice(&fixture.lineage_review).expect("JSON");
    review["sources"][0]["acquired_archive_digest"] = json!(Digest::sha256(b"wrong"));
    let parsed = parse_canonical(
        &serde_json::to_vec(&review).expect("encode"),
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )
    .expect("review shape");
    assert!(
        lineage::compile_view(&fixture.binding, &fixture.evidence, parsed, &cancellation).is_err()
    );

    let mut inconsistent_patch = super::fixture();
    let source_bytes = inconsistent_patch
        .evidence
        .source
        .get("metadata/source-provenance.json")
        .expect("source provenance");
    let mut provenance: Value = serde_json::from_slice(source_bytes).expect("JSON");
    provenance["retained_source_patches"][0]["target_revision"] = json!("unbound-revision");
    let source_bytes = serde_json::to_vec(&provenance).expect("encode");
    inconsistent_patch.evidence.source.insert(
        "metadata/source-provenance.json".to_owned(),
        source_bytes.clone(),
    );
    update_component_digest(
        &mut inconsistent_patch.evidence,
        RuntimeSourceBuildInputRole::SourceProvenance,
        &source_bytes,
    );
    for relative_path in [
        "attempts/primary/provenance.json",
        "attempts/rebuild/provenance.json",
    ] {
        let bytes = inconsistent_patch
            .evidence
            .evidence
            .get_mut(relative_path)
            .expect("build provenance");
        let mut build: Value = serde_json::from_slice(bytes).expect("JSON");
        build["source_provenance_digest"] = json!(Digest::sha256(&source_bytes));
        *bytes = serde_json::to_vec(&build).expect("encode");
    }
    assert!(compile_lineage(&inconsistent_patch, &cancellation).is_err());
}

#[test]
fn transformation_rejects_builder_steps_and_nonidentical_output() {
    let cancellation = CancellationToken::new();
    let mut changed_builder = fixture();
    changed_builder.evidence.facts.build_program_digest = Digest::sha256(b"other builder");
    assert!(compile_transformation(&changed_builder, &cancellation).is_err());

    let mut nonidentical = fixture();
    nonidentical.evidence.facts.byte_identical = false;
    assert!(compile_transformation(&nonidentical, &cancellation).is_err());

    let mut fixture = fixture();
    let bytes = fixture
        .evidence
        .evidence
        .get_mut("attempts/rebuild/transformation.json")
        .expect("rebuild transformation");
    let mut value: Value = serde_json::from_slice(bytes).expect("JSON");
    value["build_steps"][0] = json!("different-step");
    *bytes = serde_json::to_vec(&value).expect("encode");
    assert!(compile_transformation(&fixture, &cancellation).is_err());
}

#[test]
fn license_rejects_schema_one_inventory_and_incomplete_runtime_texts() {
    let cancellation = CancellationToken::new();
    let mut pending = fixture();
    let inventory = canonical(&json!({
        "components":[{"component":"legacy","disposition":"reviewed","spdx_expression":"MIT"}],
        "schema_version":1,
        "status":"reviewed"
    }));
    pending
        .evidence
        .source
        .insert("legal/licenses.json".to_owned(), inventory.clone());
    update_component_digest(
        &mut pending.evidence,
        RuntimeSourceBuildInputRole::LicenseEvidence,
        &inventory,
    );
    assert!(compile_license(&pending, &cancellation).is_err());

    let mut fixture = fixture();
    fixture.evidence.facts.license_members.pop();
    assert!(compile_license(&fixture, &cancellation).is_err());
}

#[test]
fn license_inventory_rejects_subject_and_material_structure_mutations() {
    let cancellation = CancellationToken::new();
    for mutation in 0..12 {
        let mut fixture = fixture();
        let bytes = fixture
            .evidence
            .source
            .get("legal/licenses.json")
            .expect("license inventory");
        let mut inventory: Value = serde_json::from_slice(bytes).expect("inventory JSON");
        match mutation {
            0 => {
                inventory["subjects"]
                    .as_array_mut()
                    .expect("subjects")
                    .pop();
            }
            1 => {
                let duplicate = inventory["subjects"][0].clone();
                inventory["subjects"]
                    .as_array_mut()
                    .expect("subjects")
                    .push(duplicate);
            }
            2 => inventory["subjects"]
                .as_array_mut()
                .expect("subjects")
                .swap(0, 1),
            3 => inventory["materials"][0]["content"] = json!("drifted material"),
            4 => inventory["materials"][0]["digest"] = json!(Digest::sha256(b"wrong")),
            5 => inventory["materials"][0]["byte_size"] = json!(1),
            6 => inventory["materials"][0]["relative_path"] = json!("../escape"),
            7 => inventory["materials"][0]["kind"] = json!("unknown"),
            8 => inventory["unknown"] = json!(true),
            9 => {
                inventory["materials"]
                    .as_array_mut()
                    .expect("materials")
                    .pop();
            }
            10 => {
                let duplicate = inventory["materials"][0].clone();
                inventory["materials"]
                    .as_array_mut()
                    .expect("materials")
                    .push(duplicate);
            }
            11 => inventory["materials"]
                .as_array_mut()
                .expect("materials")
                .swap(0, 1),
            _ => unreachable!(),
        }
        replace_license_inventory(&mut fixture, &inventory);
        assert!(
            compile_license(&fixture, &cancellation).is_err(),
            "{mutation}"
        );
    }

    let mut fixture = fixture();
    let inventory = fixture
        .evidence
        .source
        .get("legal/licenses.json")
        .expect("inventory");
    let mut duplicate = b"{\"schema_version\":2,".to_vec();
    duplicate.extend_from_slice(&inventory[1..]);
    fixture
        .evidence
        .source
        .insert("legal/licenses.json".to_owned(), duplicate.clone());
    update_component_digest(
        &mut fixture.evidence,
        RuntimeSourceBuildInputRole::LicenseEvidence,
        &duplicate,
    );
    assert!(compile_license(&fixture, &cancellation).is_err());
}

#[test]
fn license_review_rejects_stale_incomplete_reordered_and_ambiguous_approval() {
    let cancellation = CancellationToken::new();
    for mutation in 0..8 {
        let mut fixture = fixture();
        let mut review: Value =
            serde_json::from_slice(&fixture.license_review).expect("review JSON");
        match mutation {
            0 => review["inventory_digest"] = json!(Digest::sha256(b"stale inventory")),
            1 => {
                review["subjects"].as_array_mut().expect("subjects").pop();
            }
            2 => review["subjects"]
                .as_array_mut()
                .expect("subjects")
                .swap(0, 1),
            3 => {
                review["subjects"][0]["inventory_subject_id"] =
                    json!(Digest::sha256(b"stale subject"));
            }
            4 => review["unknown"] = json!(true),
            5 => {
                let duplicate = review["subjects"][0].clone();
                review["subjects"]
                    .as_array_mut()
                    .expect("subjects")
                    .push(duplicate);
            }
            6 => review["procedure_version"] = json!(1),
            7 => review["schema_version"] = json!(1),
            _ => unreachable!(),
        }
        fixture.license_review = canonical(&review);
        assert!(
            compile_license(&fixture, &cancellation).is_err(),
            "{mutation}"
        );
    }

    let mut fixture = fixture();
    let marker = b"{\"disposition\":\"approved\",";
    let replacement = b"{\"disposition\":\"approved\",\"disposition\":\"approved\",";
    let offset = fixture
        .license_review
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("review prefix");
    fixture
        .license_review
        .splice(offset..offset + marker.len(), replacement.iter().copied());
    assert!(compile_license(&fixture, &cancellation).is_err());
}

#[test]
fn license_review_binds_exact_sorted_runtime_distribution_members() {
    let cancellation = CancellationToken::new();
    let mut reordered = fixture();
    reordered.evidence.facts.license_members.swap(0, 1);
    assert!(compile_license(&reordered, &cancellation).is_err());

    let mut additional = fixture();
    let mut review: Value =
        serde_json::from_slice(&additional.license_review).expect("review JSON");
    let duplicate = review["runtime_license_members"][0].clone();
    review["runtime_license_members"]
        .as_array_mut()
        .expect("runtime license members")
        .push(duplicate);
    additional.license_review = canonical(&review);
    assert!(compile_license(&additional, &cancellation).is_err());

    let mut review_reordered = fixture();
    let mut review: Value =
        serde_json::from_slice(&review_reordered.license_review).expect("review JSON");
    review["runtime_license_members"]
        .as_array_mut()
        .expect("runtime license members")
        .swap(0, 1);
    review_reordered.license_review = canonical(&review);
    assert!(compile_license(&review_reordered, &cancellation).is_err());

    let mut reviewed = fixture();
    let mut review: Value = serde_json::from_slice(&reviewed.license_review).expect("review JSON");
    review["subjects"][0]["disposition"] = json!("approved_runtime_distribution");
    reviewed.license_review = canonical(&review);
    assert!(compile_license(&reviewed, &cancellation).is_ok());
}

#[test]
fn cancellation_drift_and_review_caps_fail_closed() {
    let drifting = fixture();
    drifting.evidence.fail_at.set(Some(3));
    assert!(compile_lineage(&drifting, &CancellationToken::new()).is_err());

    let cancelled_fixture = fixture();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(compile_transformation(&cancelled_fixture, &cancelled).is_err());

    let oversized = vec![b' '; MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES + 1];
    assert!(matches!(
        parse_canonical::<Value>(&oversized, MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES),
        Err(RuntimeAdmissionStaticControlError::LimitExceeded)
    ));
}
