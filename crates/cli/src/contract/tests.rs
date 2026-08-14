use std::io::Cursor;

use serde_json::{Value, json};

use super::*;

fn valid_manifest_json() -> Vec<u8> {
    let artifact_digest = Digest::sha256(b"artifact");
    let license_digest = Digest::sha256(b"license");
    serde_json::to_vec(&json!({
        "schema_version": 1,
        "artifact_id": artifact_digest,
        "source": {
            "origin": "fixture/model",
            "revision": "fixture-revision"
        },
        "artifact_digest": Digest::sha256(b"artifact"),
        "byte_size": 8,
        "format": "gguf",
        "family": "fixture",
        "architecture": "transformer",
        "quantization": "q4",
        "tokenizer": null,
        "licenses": [{
            "component": "weights",
            "identifier": "Apache-2.0",
            "text_digest": license_digest
        }],
        "declared_capabilities": {
            "roles": ["generation"],
            "languages": ["en"],
            "context_tokens": 8192
        }
    }))
    .expect("serialize fixture manifest")
}

#[test]
fn success_and_error_envelopes_are_exact_and_content_free() {
    let success = SuccessEnvelope::new(CommandName::ModelRemove, json!({"disposition": "removed"}));
    assert_eq!(
        serde_json::to_value(success).expect("serialize success"),
        json!({
            "schema_version": 1,
            "command": "model.remove",
            "status": "ok",
            "result": {"disposition": "removed"}
        })
    );
    let error = ErrorEnvelope::new(
        CommandName::ModelRemove,
        ErrorBody::new(
            ErrorCategory::Recovery,
            ErrorCode::ArtifactRemovalRecoveryRequired,
            true,
        ),
    );
    let encoded = serde_json::to_string(&error).expect("serialize error");
    assert_eq!(
        serde_json::from_str::<Value>(&encoded).expect("parse error envelope"),
        json!({
            "schema_version": 1,
            "command": "model.remove",
            "status": "error",
            "error": {
                "category": "recovery",
                "code": "artifact_removal_recovery_required",
                "retryable": true
            }
        })
    );
    assert!(!encoded.contains("private path or content"));
}

#[test]
fn migration_backup_key_is_present_only_when_attached() {
    let plain = ErrorEnvelope::new(
        CommandName::ModelMigrate,
        ErrorBody::new(
            ErrorCategory::Operational,
            ErrorCode::OperationalFailure,
            false,
        ),
    );
    let plain = serde_json::to_value(plain).expect("serialize plain error");
    assert!(plain["error"].get("migration_backup_key").is_none());

    let recoverable = ErrorEnvelope::new(
        CommandName::ModelMigrate,
        ErrorBody::new(
            ErrorCategory::Operational,
            ErrorCode::OperationalFailure,
            false,
        )
        .with_migration_backup_key("migration-backup-v2-to-v3".to_owned()),
    );
    let recoverable = serde_json::to_value(recoverable).expect("serialize recoverable error");
    assert_eq!(
        recoverable["error"]["migration_backup_key"],
        "migration-backup-v2-to-v3"
    );
}

#[test]
fn exact_artifact_selection_round_trips_without_store_types() {
    let digest = Digest::sha256(b"artifact").to_string();
    let selection = ArtifactSelectionDto {
        artifact_id: digest.parse().expect("canonical artifact ID"),
        installation_generation: "7".parse().expect("positive generation"),
    };
    let encoded = serde_json::to_string(&selection).expect("serialize selection");
    assert_eq!(
        encoded,
        format!("{{\"artifact_id\":\"{digest}\",\"installation_generation\":\"7\"}}")
    );
    assert_eq!(
        serde_json::from_str::<ArtifactSelectionDto>(&encoded).expect("deserialize selection"),
        selection
    );
    assert!("0".parse::<InstallationGeneration>().is_err());
    assert!("01".parse::<InstallationGeneration>().is_err());
    assert!("+1".parse::<InstallationGeneration>().is_err());
    assert!("A".repeat(64).parse::<ArtifactIdArgument>().is_err());
}

#[test]
fn manifest_parser_is_bounded_strict_and_domain_validated() {
    let encoded = valid_manifest_json();
    let manifest = parse_manifest_bounded(Cursor::new(&encoded), encoded.len())
        .expect("valid exact-boundary manifest");
    assert_eq!(manifest.byte_size, 8);
    assert_eq!(
        parse_manifest_bounded(Cursor::new(&encoded), encoded.len() - 1),
        Err(ManifestInputError::TooLarge)
    );
    assert_eq!(
        parse_manifest_bounded(Cursor::new(&encoded), 0),
        Err(ManifestInputError::InvalidLimit)
    );

    let mut unknown: Value = serde_json::from_slice(&encoded).expect("fixture JSON");
    unknown["unknown"] = json!(true);
    assert_eq!(
        parse_manifest_bounded(
            Cursor::new(serde_json::to_vec(&unknown).expect("serialize unknown field")),
            MAX_MANIFEST_BYTES,
        ),
        Err(ManifestInputError::InvalidJson)
    );

    let mut invalid: Value = serde_json::from_slice(&encoded).expect("fixture JSON");
    invalid["schema_version"] = json!(0);
    assert_eq!(
        parse_manifest_bounded(
            Cursor::new(serde_json::to_vec(&invalid).expect("serialize invalid manifest")),
            MAX_MANIFEST_BYTES,
        ),
        Err(ManifestInputError::UnsupportedSchema)
    );
}
