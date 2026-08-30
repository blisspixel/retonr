use super::*;

#[test]
fn canonical_schema_attempt_order_and_policy_are_fail_closed() {
    let fixture = fixture(false);
    let pretty = serde_json::to_vec_pretty(
        &serde_json::from_slice::<Value>(&fixture.report).expect("report value"),
    )
    .expect("pretty report");
    assert_eq!(
        verify_bytes(&fixture, &pretty),
        Err(RuntimeSourceBuildReportError::NoncanonicalEncoding)
    );
    assert_eq!(
        verify_bytes(&fixture, br#"{"schema_version":2,"schema_version":2}"#),
        Err(RuntimeSourceBuildReportError::InvalidEncoding)
    );
    for (pointer, replacement, error) in [
        (
            "/schema_version",
            json!(1),
            RuntimeSourceBuildReportError::UnsupportedSchema,
        ),
        (
            "/schema_version",
            json!(2),
            RuntimeSourceBuildReportError::UnsupportedSchema,
        ),
        (
            "/attempts/0/network_access",
            json!("allowed"),
            RuntimeSourceBuildReportError::InvalidEncoding,
        ),
        (
            "/attempts/0/accelerator",
            json!("gpu"),
            RuntimeSourceBuildReportError::InvalidEncoding,
        ),
    ] {
        let mut value = report_value(&fixture);
        *value.pointer_mut(pointer).expect("fixture pointer") = replacement;
        assert_eq!(verify_bytes(&fixture, &canonical(&value)), Err(error));
    }
    let mut reordered = report_value(&fixture);
    reordered["attempts"]
        .as_array_mut()
        .expect("attempts")
        .swap(0, 1);
    assert_eq!(
        verify_bytes(&fixture, &canonical(&reordered)),
        Err(RuntimeSourceBuildReportError::InvalidAttempt)
    );
    let mut evidence_order = report_value(&fixture);
    evidence_order["attempts"][0]["evidence"]
        .as_array_mut()
        .expect("evidence")
        .swap(0, 1);
    assert_eq!(
        verify_bytes(&fixture, &canonical(&evidence_order)),
        Err(RuntimeSourceBuildReportError::InvalidAttempt)
    );
}

#[test]
fn plan_and_comparison_cannot_diverge_from_reconstructed_bytes() {
    let fixture = fixture(false);
    for pointer in [
        "/source_build_inputs_id",
        "/build_plan_digest",
        "/attempts/0/environment_digest",
        "/attempts/1/build_arguments_digest",
    ] {
        let mut value = report_value(&fixture);
        *value.pointer_mut(pointer).expect("fixture pointer") = json!(Digest::sha256(b"wrong"));
        assert_eq!(
            verify_bytes(&fixture, &canonical(&value)),
            Err(RuntimeSourceBuildReportError::InvalidPlanBinding),
            "{pointer}"
        );
    }
    let mut comparison = report_value(&fixture);
    comparison["comparison"]["artifact_set_id"] = json!(Digest::sha256(b"wrong set"));
    assert_eq!(
        verify_bytes(&fixture, &canonical(&comparison)),
        Err(RuntimeSourceBuildReportError::InvalidComparison)
    );

    let mut attempt_tree = report_value(&fixture);
    attempt_tree["attempts"][0]["output_tree_digest"] = json!(Digest::sha256(b"wrong output tree"));
    assert_eq!(
        verify_bytes(&fixture, &canonical(&attempt_tree)),
        Err(RuntimeSourceBuildReportError::InvalidOutputTree)
    );

    let mut comparison_tree = report_value(&fixture);
    comparison_tree["comparison"]["output_tree_digest"] =
        json!(Digest::sha256(b"wrong output tree"));
    assert_eq!(
        verify_bytes(&fixture, &canonical(&comparison_tree)),
        Err(RuntimeSourceBuildReportError::InvalidComparison)
    );
}

#[test]
fn output_tree_codec_is_strict_canonical_and_structure_preserving() {
    let fixture = fixture(false);
    let tree = output_tree(&fixture, RuntimeSourceBuildAttempt::Primary);
    assert_eq!(
        RuntimeSourceBuildOutputTree::parse(tree.canonical_bytes()),
        Ok(tree.clone())
    );

    let pretty = serde_json::to_vec_pretty(
        &serde_json::from_slice::<Value>(tree.canonical_bytes()).expect("output tree value"),
    )
    .expect("pretty output tree");
    assert_eq!(
        RuntimeSourceBuildOutputTree::parse(&pretty),
        Err(RuntimeSourceBuildReportError::NoncanonicalOutputTree)
    );

    let mut unknown = serde_json::from_slice::<Value>(tree.canonical_bytes()).expect("tree value");
    unknown["unknown"] = json!(true);
    assert_eq!(
        RuntimeSourceBuildOutputTree::parse(&canonical(&unknown)),
        Err(RuntimeSourceBuildReportError::InvalidOutputTree)
    );

    let mut excessive_mode =
        serde_json::from_slice::<Value>(tree.canonical_bytes()).expect("tree value");
    excessive_mode["entries"][0]["unix_mode"] = json!(0o1_000);
    assert_eq!(
        RuntimeSourceBuildOutputTree::parse(&canonical(&excessive_mode)),
        Err(RuntimeSourceBuildReportError::InvalidOutputTree)
    );

    let mut reordered =
        serde_json::from_slice::<Value>(tree.canonical_bytes()).expect("tree value");
    reordered["entries"]
        .as_array_mut()
        .expect("entries")
        .reverse();
    assert_eq!(
        RuntimeSourceBuildOutputTree::parse(&canonical(&reordered)),
        Err(RuntimeSourceBuildReportError::NoncanonicalOutputTree)
    );

    let incomplete = RuntimeSourceBuildOutputTree::compile(vec![
        RuntimeSourceBuildOutputTreeEntry::regular_file(
            ArtifactSetRelativePath::new("bin/tool").expect("member path"),
            0o755,
            4,
            Digest::sha256(b"tool"),
        )
        .expect("file entry"),
    ]);
    assert_eq!(
        incomplete,
        Err(RuntimeSourceBuildReportError::InvalidOutputTree)
    );
}

#[test]
fn evidence_paths_are_bound_to_attempt_and_kind_before_opening() {
    let fixture = fixture(false);
    let mut cross_kind = report_value(&fixture);
    swap_evidence_bindings(
        &mut cross_kind,
        "/attempts/0/evidence/1",
        "/attempts/0/evidence/2",
    );
    assert_eq!(
        verify_bytes(&fixture, &canonical(&cross_kind)),
        Err(RuntimeSourceBuildReportError::InvalidAttempt)
    );

    let mut cross_attempt = report_value(&fixture);
    swap_evidence_bindings(
        &mut cross_attempt,
        "/attempts/0/evidence/1",
        "/attempts/1/evidence/1",
    );
    assert_eq!(
        verify_bytes(&fixture, &canonical(&cross_attempt)),
        Err(RuntimeSourceBuildReportError::InvalidAttempt)
    );
}

#[test]
fn every_attempt_and_evidence_kind_has_one_canonical_path() {
    for (attempt, name) in [
        (RuntimeSourceBuildAttempt::Primary, "primary"),
        (RuntimeSourceBuildAttempt::Rebuild, "rebuild"),
    ] {
        for (kind, file) in RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS.into_iter().zip([
            "runtime-layout.json",
            "sbom.json",
            "provenance.json",
            "transformation.json",
            "output-tree.json",
            "execution-receipt.json",
            "stdout.bin",
            "stderr.bin",
        ]) {
            assert_eq!(
                canonical_evidence_path(attempt, kind)
                    .expect("canonical evidence path")
                    .as_str(),
                format!("attempts/{name}/{file}")
            );
        }
    }
}

fn swap_evidence_bindings(value: &mut Value, left: &str, right: &str) {
    let left_value = value.pointer(left).expect("left evidence").clone();
    let right_value = value.pointer(right).expect("right evidence").clone();
    for field in ["relative_path", "byte_size", "digest"] {
        value
            .pointer_mut(&format!("{left}/{field}"))
            .expect("left evidence field")
            .clone_from(&right_value[field]);
        value
            .pointer_mut(&format!("{right}/{field}"))
            .expect("right evidence field")
            .clone_from(&left_value[field]);
    }
}
