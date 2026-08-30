use super::*;

#[test]
fn compiler_derives_the_exact_canonical_report_from_both_outputs() {
    for different in [false, true] {
        let fixture = fixture(different);
        let compiled = compile_fixture(&fixture).expect("compile exact report");
        assert_eq!(compiled.canonical_bytes(), fixture.report);
        assert_eq!(compiled.is_byte_identical(), !different);
        let verified =
            verify_bytes(&fixture, compiled.canonical_bytes()).expect("compiled report verifies");
        assert_eq!(verified.is_byte_identical(), !different);
    }
}

#[test]
fn compiler_limits_and_cancellation_win_before_unbounded_input() {
    let fixture = fixture(false);
    let cancelled = compile_runtime_source_build_report(
        &fixture.source_inputs,
        &fixture.plan,
        &RuntimeSourceBuildReportLimits::default(),
        |_attempt, _path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildReportOpenError> {
            panic!("cancellation must win before evidence open")
        },
        |_attempt, _path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
            panic!("cancellation must win before member open")
        },
        || true,
    );
    assert_eq!(cancelled, Err(RuntimeSourceBuildReportError::Cancelled));

    let limits = RuntimeSourceBuildReportLimits {
        maximum_evidence_bytes: 1,
        maximum_total_evidence_bytes: 1,
        ..RuntimeSourceBuildReportLimits::default()
    };
    let limited = compile_runtime_source_build_report(
        &fixture.source_inputs,
        &fixture.plan,
        &limits,
        |attempt, path| {
            fixture
                .evidence
                .get(&(attempt, path.as_str().to_owned()))
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimeSourceBuildReportOpenError)
        },
        |attempt, path| open_member(&fixture, attempt, path),
        || false,
    );
    assert_eq!(limited, Err(RuntimeSourceBuildReportError::LimitExceeded));
}

#[test]
fn compiler_and_verifier_reject_semantically_relabeled_source_manifest_before_io() {
    let fixture = fixture(false);
    let (_source_id, source_bytes, _components) = controlled_source_fixture();
    for (pointer, replacement) in [
        ("/components/0/name", json!("relabeled-helper")),
        ("/components/0/revision", json!("relabeled-revision")),
        (
            "/components/0/source_locator",
            json!("urn:fixture:relabeled-helper"),
        ),
        (
            "/components/1/roles",
            json!(["source_patch", "license_evidence"]),
        ),
    ] {
        let mut relabeled_value: Value =
            serde_json::from_slice(&source_bytes).expect("source manifest value");
        *relabeled_value
            .pointer_mut(pointer)
            .expect("source manifest pointer") = replacement;
        let relabeled = RuntimeSourceBuildInputManifest::parse(
            &canonical(&relabeled_value),
            RuntimeSourceBuildInputLimits::default(),
        )
        .expect("relabeled source manifest");
        assert_eq!(
            relabeled.artifact_set().artifact_set_id(),
            fixture.source_inputs.artifact_set().artifact_set_id(),
            "{pointer}"
        );
        assert_ne!(
            relabeled.manifest_digest(),
            fixture.source_inputs.manifest_digest(),
            "{pointer}"
        );

        assert_eq!(
            compile_runtime_source_build_report(
                &relabeled,
                &fixture.plan,
                &RuntimeSourceBuildReportLimits::default(),
                |_attempt, _path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildReportOpenError> {
                    panic!("manifest binding must win before evidence open")
                },
                |_attempt, _path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
                    panic!("manifest binding must win before member open")
                },
                || false,
            ),
            Err(RuntimeSourceBuildReportError::InvalidPlanBinding),
            "{pointer}"
        );
        assert_eq!(
            verify_runtime_source_build_report(
                &fixture.report,
                &relabeled,
                &fixture.plan,
                &RuntimeSourceBuildReportLimits::default(),
                |_attempt, _path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildReportOpenError> {
                    panic!("manifest binding must win before evidence open")
                },
                |_attempt, _path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
                    panic!("manifest binding must win before member open")
                },
                || false,
            ),
            Err(RuntimeSourceBuildReportError::InvalidPlanBinding),
            "{pointer}"
        );
    }
}

#[test]
fn verifies_two_byte_identical_controlled_build_outputs() {
    let fixture = fixture(false);
    let verified = verify(&fixture).expect("exact two-build report verifies");
    assert!(verified.is_byte_identical());
    assert_eq!(
        verified.report().build_plan_digest(),
        fixture.plan.plan_digest()
    );
    assert_eq!(
        verified.report().source_build_inputs_id(),
        fixture.plan.source_inputs_id()
    );
    assert_eq!(
        verified.primary().artifact_set(),
        verified.rebuild().artifact_set()
    );
    assert_eq!(
        verified.primary().runtime_package(),
        verified.rebuild().runtime_package()
    );
}

#[test]
fn well_hashed_false_and_malformed_execution_receipts_are_rejected() {
    for bytes in [
        b"{}".to_vec(),
        mutated_primary_receipt(&["canary", "passed"], json!(false)),
        mutated_primary_receipt(
            &["output_tree", "digest"],
            json!(Digest::sha256(b"false output commitment")),
        ),
        mutated_primary_receipt(&["target_status"], json!({"kind": "exit_code", "value": 1})),
    ] {
        assert_invalid_primary_receipt(&bytes);
    }
}

#[test]
fn missing_nonzero_and_signaled_guardian_helper_statuses_are_rejected() {
    let fixture = fixture(false);
    let key = (
        RuntimeSourceBuildAttempt::Primary,
        "attempts/primary/execution-receipt.json".to_owned(),
    );
    let mut missing: Value = serde_json::from_slice(
        fixture
            .evidence
            .get(&key)
            .expect("primary execution receipt"),
    )
    .expect("receipt value");
    missing
        .as_object_mut()
        .expect("receipt object")
        .remove("guardian_helper_status");
    for bytes in [
        canonical(&missing),
        mutated_primary_receipt(
            &["guardian_helper_status"],
            json!({"kind": "exit_code", "value": 7}),
        ),
        mutated_primary_receipt(
            &["guardian_helper_status"],
            json!({"kind": "signal", "value": 9}),
        ),
    ] {
        assert_invalid_primary_receipt(&bytes);
    }
}

#[test]
fn receipt_policy_timeout_plan_program_and_helper_drift_are_rejected() {
    for (path, value) in [
        (&["isolation_policy", "maximum_processes"][..], json!(4_095)),
        (&["execution_timeout_seconds"][..], json!(10_799)),
        (
            &["build_plan_digest"][..],
            json!(Digest::sha256(b"wrong plan")),
        ),
        (
            &["build_program", "digest"][..],
            json!(Digest::sha256(b"wrong program")),
        ),
        (
            &["isolation_helper", "digest"][..],
            json!(Digest::sha256(b"wrong helper")),
        ),
    ] {
        assert_invalid_primary_receipt(&mutated_primary_receipt(path, value));
    }
}

fn mutated_primary_receipt(path: &[&str], value: Value) -> Vec<u8> {
    let fixture = fixture(false);
    let bytes = fixture
        .evidence
        .get(&(
            RuntimeSourceBuildAttempt::Primary,
            "attempts/primary/execution-receipt.json".to_owned(),
        ))
        .expect("primary execution receipt");
    execution_receipt::tests::mutate(bytes, path, value)
}

fn assert_invalid_primary_receipt(bytes: &[u8]) {
    let mut fixture = fixture(false);
    replace_execution_receipt(&mut fixture, RuntimeSourceBuildAttempt::Primary, bytes);
    assert_eq!(
        verify(&fixture),
        Err(RuntimeSourceBuildReportError::InvalidExecutionReceipt)
    );
}

#[test]
fn verifies_and_retains_a_nonidentical_rebuild_disposition() {
    let fixture = fixture(true);
    let verified = verify(&fixture).expect("different outputs still form verified evidence");
    assert!(!verified.is_byte_identical());
    assert_ne!(
        verified.primary().artifact_set(),
        verified.rebuild().artifact_set()
    );
    assert!(matches!(
        verified.report().comparison(),
        RuntimeSourceBuildComparison::Different { .. }
    ));
}

#[test]
fn mode_only_output_difference_is_not_byte_identical() {
    let mut fixture = fixture(false);
    fixture.rebuild_entrypoint_mode = 0o644;
    rebuild_report(&mut fixture);

    let compiled = compile_fixture(&fixture).expect("mode-different report compiles");
    assert_eq!(
        compiled.primary().artifact_set(),
        compiled.rebuild().artifact_set()
    );
    assert_eq!(
        compiled.primary().runtime_package(),
        compiled.rebuild().runtime_package()
    );
    assert_ne!(
        compiled.primary_output_tree(),
        compiled.rebuild_output_tree()
    );
    assert!(!compiled.is_byte_identical());

    let verified = verify(&fixture).expect("mode-different report verifies");
    assert!(!verified.is_byte_identical());
    assert_ne!(
        verified.primary_output_tree().digest(),
        verified.rebuild_output_tree().digest()
    );
    assert!(matches!(
        verified.report().comparison(),
        RuntimeSourceBuildComparison::Different { .. }
    ));
}

#[test]
fn incomplete_output_tree_closure_is_rejected_after_evidence_hashing() {
    let mut fixture = fixture(false);
    let evidence_key = (
        RuntimeSourceBuildAttempt::Primary,
        "attempts/primary/output-tree.json".to_owned(),
    );
    let mut tree: Value = serde_json::from_slice(
        fixture
            .evidence
            .get(&evidence_key)
            .expect("primary output tree"),
    )
    .expect("output tree value");
    tree["entries"]
        .as_array_mut()
        .expect("output tree entries")
        .retain(|entry| entry["relative_path"] != "transformation.json");
    let tree_bytes = canonical(&tree);
    fixture.evidence.insert(evidence_key, tree_bytes.clone());

    let mut report = report_value(&fixture);
    let record = &mut report["attempts"][0]["evidence"][4];
    record["byte_size"] = json!(u64::try_from(tree_bytes.len()).expect("tree length fits u64"));
    record["digest"] = json!(Digest::sha256(&tree_bytes));
    assert_eq!(
        verify_bytes(&fixture, &canonical(&report)),
        Err(RuntimeSourceBuildReportError::InvalidOutputTree)
    );
}

#[test]
fn evidence_open_size_digest_and_cancellation_fail_distinctly() {
    let fixture = fixture(false);
    assert_eq!(
        verify_runtime_source_build_report(
            &fixture.report,
            &fixture.source_inputs,
            &fixture.plan,
            &RuntimeSourceBuildReportLimits::default(),
            |_attempt, _path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildReportOpenError> {
                Err(RuntimeSourceBuildReportOpenError)
            },
            |_attempt, _path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
            || false
        ),
        Err(RuntimeSourceBuildReportError::EvidenceUnavailable)
    );
    for mode in ["short", "long", "digest"] {
        let result = verify_runtime_source_build_report(
            &fixture.report,
            &fixture.source_inputs,
            &fixture.plan,
            &RuntimeSourceBuildReportLimits::default(),
            |attempt, path| {
                let mut bytes = fixture
                    .evidence
                    .get(&(attempt, path.as_str().to_owned()))
                    .cloned()
                    .ok_or(RuntimeSourceBuildReportOpenError)?;
                if attempt == RuntimeSourceBuildAttempt::Primary
                    && path.as_str().ends_with("runtime-layout.json")
                {
                    match mode {
                        "short" => {
                            bytes.pop();
                        }
                        "long" => bytes.push(0),
                        "digest" => *bytes.last_mut().expect("nonempty evidence") ^= 1,
                        _ => unreachable!(),
                    }
                }
                Ok(Cursor::new(bytes))
            },
            |attempt, path| open_member(&fixture, attempt, path),
            || false,
        );
        assert_eq!(
            result,
            Err(match mode {
                "short" | "long" => RuntimeSourceBuildReportError::EvidenceSizeMismatch,
                "digest" => RuntimeSourceBuildReportError::EvidenceDigestMismatch,
                _ => unreachable!(),
            })
        );
    }
    assert_eq!(
        verify_runtime_source_build_report(
            &fixture.report,
            &fixture.source_inputs,
            &fixture.plan,
            &RuntimeSourceBuildReportLimits::default(),
            |_attempt, _path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildReportOpenError> {
                panic!("cancellation must win before evidence open")
            },
            |_attempt, _path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
                panic!("cancellation must win before member open")
            },
            || true
        ),
        Err(RuntimeSourceBuildReportError::Cancelled)
    );
}

#[test]
fn limits_fail_before_openers_and_runtime_source_binding_is_rechecked() {
    let baseline = fixture(false);
    for limits in [
        RuntimeSourceBuildReportLimits {
            report_bytes: 0,
            ..RuntimeSourceBuildReportLimits::default()
        },
        RuntimeSourceBuildReportLimits {
            maximum_evidence_bytes: 1,
            ..RuntimeSourceBuildReportLimits::default()
        },
        RuntimeSourceBuildReportLimits {
            maximum_total_evidence_bytes: 1,
            ..RuntimeSourceBuildReportLimits::default()
        },
    ] {
        assert_eq!(
            verify_runtime_source_build_report(
                &baseline.report,
                &baseline.source_inputs,
                &baseline.plan,
                &limits,
                |_attempt, _path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildReportOpenError> {
                    panic!("invalid limits must win")
                },
                |_attempt, _path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
                    panic!("invalid limits must win")
                },
                || false
            ),
            Err(RuntimeSourceBuildReportError::LimitExceeded)
        );
    }

    for (pointer, replacement) in [
        (
            "/transformation/source_artifact_set_id",
            json!(Digest::sha256(b"wrong source")),
        ),
        (
            "/source/provenance_digest",
            json!(Digest::sha256(b"wrong provenance")),
        ),
        (
            "/transformation/parameters_digest",
            json!(Digest::sha256(b"wrong parameters")),
        ),
        (
            "/transformation/tool_evidence_digest",
            json!(Digest::sha256(b"wrong tools")),
        ),
        (
            "/transformation/log_digest",
            json!(Digest::sha256(b"wrong standard output")),
        ),
        (
            "/source/revision",
            json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
        (
            "/build_revision",
            json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
    ] {
        let mut unbound = fixture(false);
        let key = (
            RuntimeSourceBuildAttempt::Primary,
            "attempts/primary/runtime-layout.json".to_owned(),
        );
        let mut layout: Value = serde_json::from_slice(unbound.evidence.get(&key).expect("layout"))
            .expect("layout value");
        *layout.pointer_mut(pointer).expect("binding pointer") = replacement;
        unbound.evidence.insert(key, canonical(&layout));
        rebuild_report(&mut unbound);
        assert_eq!(
            verify(&unbound),
            Err(RuntimeSourceBuildReportError::InvalidRuntimeBinding),
            "{pointer}"
        );
    }
}
