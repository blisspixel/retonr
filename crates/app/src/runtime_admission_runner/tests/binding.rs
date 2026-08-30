use super::*;

#[test]
fn reviewed_launch_mismatch_fails_the_actual_launch_binding() {
    let mut actual =
        LaunchSpec::new(std::env::current_exe().expect("absolute current test executable"));
    actual.insert_environment("OLLAMA_NO_CLOUD", "1");
    let actual_digest = actual.redacted_digest();
    assert_eq!(
        validate_actual_launch_binding(&actual, &actual_digest).expect("exact launch binding"),
        actual_digest
    );

    let mut cross_wired = actual;
    cross_wired.push_argument("from-another-isolation-run");
    assert!(matches!(
        validate_actual_launch_binding(&cross_wired, &actual_digest),
        Err(RuntimeAdmissionRunnerError::InvalidLaunchBinding)
    ));
}

#[test]
fn managed_startup_evidence_retains_actual_launch_and_policy_digests() {
    let package_id = runtime_package("0.16.2").runtime_package_manifest_id();
    let process_digest = Digest::sha256(b"managed process evidence");
    let launch_digest = Digest::sha256(b"actual retained launch");
    let policy_digest = Digest::sha256(b"actual retained isolation policy");
    let standard_output = b"Ollama cloud disabled: true\n";
    let standard_error = b"startup warning\n";
    let (_marker, evidence) = compile_managed_startup_evidence(
        standard_output,
        standard_error,
        false,
        false,
        package_id.clone(),
        process_digest.clone(),
        launch_digest.clone(),
        policy_digest.clone(),
    )
    .expect("compile exact managed startup evidence");

    assert_eq!(evidence.runtime_package_manifest_id(), &package_id);
    assert_eq!(evidence.process_evidence_digest(), &process_digest);
    assert_eq!(evidence.launch_spec_digest(), &launch_digest);
    assert_eq!(evidence.isolation_policy_digest(), &policy_digest);
    assert_eq!(
        evidence.standard_output_digest(),
        &Digest::sha256(standard_output)
    );
    assert_eq!(
        evidence.standard_error_digest(),
        &Digest::sha256(standard_error)
    );
    assert_eq!(
        evidence.standard_output_bytes(),
        standard_output.len() as u64
    );
    assert_eq!(evidence.standard_error_bytes(), standard_error.len() as u64);
}
