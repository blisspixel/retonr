use rewrite_model::RuntimePackageManifestId;
use rewrite_types::Digest;
use serde_json::json;

use super::*;

const REVIEWED_VERSION: OllamaVersion = OllamaVersion::new(0, 16, 2);

#[test]
fn exact_version_parser_accepts_only_canonical_stable_versions() {
    let version = "0.16.2".parse::<OllamaVersion>().expect("valid version");

    assert_eq!(version.major(), 0);
    assert_eq!(version.minor(), 16);
    assert_eq!(version.patch(), 2);
    assert_eq!(version.to_string(), "0.16.2");
    assert_eq!(serde_json::to_value(version).expect("serialize"), "0.16.2");
}

#[test]
fn exact_version_parser_rejects_malformed_and_prerelease_versions() {
    let oversized = "1".repeat(MAX_VERSION_BYTES + 1);
    for value in [
        "",
        "0.16",
        "0.16.2.1",
        "v0.16.2",
        "0.16.2-rc.1",
        "0.16.2+build",
        "00.16.2",
        "0.016.2",
        "0.16.02",
        "0.a.2",
        "0.16.2 ",
        "4294967296.16.2",
        "0.16.\u{0662}",
        &oversized,
    ] {
        assert_eq!(value.parse::<OllamaVersion>(), Err(OllamaVersionParseError));
    }
}

#[test]
fn production_policy_has_feature_floor_and_empty_reviewed_allowlist() {
    let unavailable = "0.16.1".parse::<OllamaVersion>().expect("valid version");
    let feature_floor = "0.16.2".parse::<OllamaVersion>().expect("valid version");
    let future = "1.0.0".parse::<OllamaVersion>().expect("valid version");

    assert_eq!(
        OllamaCloudDisableFeaturePolicy::feature_floor(),
        OLLAMA_CLOUD_DISABLE_FEATURE_FLOOR
    );
    assert_eq!(OllamaCloudDisableFeaturePolicy::reviewed_runtime_count(), 0);
    let runtime = runtime_package_id("reviewed runtime");
    assert_eq!(
        OllamaCloudDisableFeaturePolicy::assess(unavailable, &runtime),
        OllamaCloudDisableVersionStatus::FeatureUnavailable
    );
    assert_eq!(
        OllamaCloudDisableFeaturePolicy::assess(feature_floor, &runtime),
        OllamaCloudDisableVersionStatus::Unreviewed
    );
    assert_eq!(
        OllamaCloudDisableFeaturePolicy::assess(future, &runtime),
        OllamaCloudDisableVersionStatus::Unreviewed
    );
    assert_eq!(
        assess_runtime(
            feature_floor,
            &runtime,
            &[ReviewedCloudDisableRuntime {
                version: feature_floor,
                runtime_package_manifest_id: runtime.clone(),
            }]
        ),
        OllamaCloudDisableVersionStatus::Reviewed
    );
}

#[test]
fn production_observation_rejects_unavailable_and_unreviewed_versions() {
    assert_eq!(
        OllamaCloudDisableEvidence::observe(
            &runtime_package_id("runtime"),
            "0.16.1",
            "0.16.1",
            &["1"],
            valid_log()
        ),
        Err(OllamaCloudDisableEvidenceError::FeatureUnavailable)
    );
    assert_eq!(
        OllamaCloudDisableEvidence::observe(
            &runtime_package_id("runtime"),
            "0.16.2",
            "0.16.2",
            &["1"],
            valid_log()
        ),
        Err(OllamaCloudDisableEvidenceError::UnreviewedRuntimePackage)
    );
    assert_eq!(
        OllamaCloudDisableEvidence::observe(
            &runtime_package_id("runtime"),
            "99.0.0",
            "99.0.0",
            &["1"],
            valid_log()
        ),
        Err(OllamaCloudDisableEvidenceError::UnreviewedRuntimePackage)
    );
}

#[test]
fn same_version_substituted_runtime_package_is_not_reviewed() {
    let reviewed = runtime_package_id("reviewed runtime");
    let substituted = runtime_package_id("substituted runtime");
    let policy = [ReviewedCloudDisableRuntime {
        version: REVIEWED_VERSION,
        runtime_package_manifest_id: reviewed,
    }];
    assert_eq!(
        observe_with_policy(
            "0.16.2",
            "0.16.2",
            &["1"],
            valid_log(),
            &substituted,
            &policy,
        ),
        Err(OllamaCloudDisableEvidenceError::UnreviewedRuntimePackage)
    );
}

#[test]
fn observation_rejects_invalid_version_and_version_drift() {
    assert_eq!(
        observe_for_test("0.16.2-rc.1", "0.16.2", &["1"], valid_log()),
        Err(OllamaCloudDisableEvidenceError::InvalidVersion)
    );
    assert_eq!(
        observe_for_test("0.16.2", "0.16.3", &["1"], valid_log()),
        Err(OllamaCloudDisableEvidenceError::VersionDrift)
    );
}

#[test]
fn managed_environment_is_exact_bounded_and_unique() {
    assert_eq!(
        OllamaManagedCloudDisableEnvironment::parse(&["1"]),
        Ok(OllamaManagedCloudDisableEnvironment(()))
    );
    assert_eq!(
        OllamaManagedCloudDisableEnvironment::parse(&[]),
        Err(OllamaCloudDisableEvidenceError::MissingEnvironmentDeclaration)
    );
    assert_eq!(
        OllamaManagedCloudDisableEnvironment::parse(&["1", "1"]),
        Err(OllamaCloudDisableEvidenceError::DuplicateEnvironmentDeclaration)
    );
    assert_eq!(
        OllamaManagedCloudDisableEnvironment::parse(&["1", "0"]),
        Err(OllamaCloudDisableEvidenceError::ConflictingEnvironmentDeclaration)
    );
    assert_eq!(
        OllamaManagedCloudDisableEnvironment::parse(&["true"]),
        Err(OllamaCloudDisableEvidenceError::ConflictingEnvironmentDeclaration)
    );
    assert_eq!(
        OllamaManagedCloudDisableEnvironment::parse(
            &[&"x".repeat(MAX_ENVIRONMENT_VALUE_BYTES + 1)]
        ),
        Err(OllamaCloudDisableEvidenceError::OversizedEnvironmentDeclaration)
    );
    assert_eq!(
        OllamaManagedCloudDisableEnvironment::parse(&["1", "1", "1", "1", "1"]),
        Err(OllamaCloudDisableEvidenceError::OversizedEnvironmentDeclaration)
    );
}

#[test]
fn startup_marker_parser_accepts_documented_and_structured_forms() {
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse(CLOUD_DISABLED_MARKER),
        Ok(OllamaCloudDisableStartupMarker(()))
    );
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse(valid_log()),
        Ok(OllamaCloudDisableStartupMarker(()))
    );
}

#[test]
fn startup_marker_parser_rejects_missing_duplicate_and_conflicting_markers() {
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse("server started"),
        Err(OllamaCloudDisableEvidenceError::MissingStartupMarker)
    );
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse(&format!(
            "{CLOUD_DISABLED_MARKER}\n{CLOUD_DISABLED_MARKER}"
        )),
        Err(OllamaCloudDisableEvidenceError::DuplicateStartupMarker)
    );
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse(&format!(
            "{CLOUD_DISABLED_MARKER}\n{CLOUD_ENABLED_MARKER}"
        )),
        Err(OllamaCloudDisableEvidenceError::ConflictingStartupMarker)
    );
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse(&"x".repeat(MAX_STARTUP_LOG_BYTES + 1)),
        Err(OllamaCloudDisableEvidenceError::OversizedStartupLog)
    );
}

#[test]
fn startup_marker_parser_preserves_stream_boundaries_and_aggregate_limits() {
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse_streams(
            b"server preparation complete",
            CLOUD_DISABLED_MARKER.as_bytes(),
        ),
        Ok(OllamaCloudDisableStartupMarker(()))
    );
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse_streams(
            CLOUD_DISABLED_MARKER.as_bytes(),
            CLOUD_DISABLED_MARKER.as_bytes(),
        ),
        Err(OllamaCloudDisableEvidenceError::DuplicateStartupMarker)
    );
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse_streams(&[0xff], b""),
        Err(OllamaCloudDisableEvidenceError::InvalidStartupLogEncoding)
    );
    assert_eq!(
        OllamaCloudDisableStartupMarker::parse_streams(&vec![b'x'; MAX_STARTUP_LOG_BYTES], b"x",),
        Err(OllamaCloudDisableEvidenceError::OversizedStartupLog)
    );
}

#[test]
fn evidence_is_redacted_and_cannot_claim_isolation_or_qualification() {
    let evidence =
        observe_for_test("0.16.2", "0.16.2", &["1"], valid_log()).expect("reviewed test evidence");

    assert_eq!(evidence.runtime_version(), REVIEWED_VERSION);
    assert_eq!(
        evidence.runtime_package_manifest_id(),
        &runtime_package_id("reviewed runtime")
    );
    assert!(evidence.version_reviewed());
    assert!(evidence.provider_declared());
    assert_eq!(
        evidence.provider_declaration_status(),
        OllamaProviderDeclarationStatus::Observed
    );
    assert!(!evidence.network_isolation_enforced());
    assert_eq!(
        evidence.network_isolation_status(),
        OllamaNetworkIsolationStatus::NotEnforced
    );
    assert!(!evidence.qualified());
    assert_eq!(
        evidence.qualification_status(),
        OllamaProviderQualificationStatus::NotQualified
    );

    let serialized = serde_json::to_value(&evidence).expect("serialize evidence");
    assert_eq!(
        serialized,
        json!({
            "schema_version": 1,
            "runtime_version": "0.16.2",
            "runtime_package_manifest_id": runtime_package_id("reviewed runtime"),
            "feature_floor": "0.16.2",
            "version_reviewed": true,
            "declaration_source": "managed_environment",
            "marker_source": "managed_startup_output",
            "provider_declaration": "observed",
            "network_isolation": "not_enforced",
            "qualification": "not_qualified"
        })
    );
    let rendered = serialized.to_string();
    assert!(!rendered.contains("OLLAMA_NO_CLOUD"));
    assert!(!rendered.contains(CLOUD_DISABLED_MARKER));
    assert!(!rendered.contains("routes.go"));
}

#[test]
fn reviewed_observation_propagates_environment_and_marker_failures() {
    assert_eq!(
        observe_for_test("0.16.2", "0.16.2", &[], valid_log()),
        Err(OllamaCloudDisableEvidenceError::MissingEnvironmentDeclaration)
    );
    assert_eq!(
        observe_for_test("0.16.2", "0.16.2", &["1"], "server started"),
        Err(OllamaCloudDisableEvidenceError::MissingStartupMarker)
    );
}

fn observe_for_test(
    version_before: &str,
    version_after: &str,
    managed_environment_values: &[&str],
    startup_log: &str,
) -> Result<OllamaCloudDisableEvidence, OllamaCloudDisableEvidenceError> {
    let runtime = runtime_package_id("reviewed runtime");
    observe_with_policy(
        version_before,
        version_after,
        managed_environment_values,
        startup_log,
        &runtime,
        &[ReviewedCloudDisableRuntime {
            version: REVIEWED_VERSION,
            runtime_package_manifest_id: runtime.clone(),
        }],
    )
}

fn runtime_package_id(seed: &str) -> RuntimePackageManifestId {
    serde_json::from_value(json!(Digest::sha256(seed.as_bytes())))
        .expect("fixture runtime-package manifest identity")
}

fn valid_log() -> &'static str {
    "time=2026-08-21T00:00:00Z level=INFO source=routes.go:1 msg=\"Ollama cloud disabled: true\""
}
