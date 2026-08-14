use rewrite_app::{ArtifactRepositoryMigrationDisposition, ArtifactRepositoryMigrationResult};
use serde::Serialize;

use super::ModelOutput;

impl ModelOutput {
    pub(crate) fn migration(result: &ArtifactRepositoryMigrationResult) -> Self {
        let disposition = match result.disposition {
            ArtifactRepositoryMigrationDisposition::AlreadyCurrent => "already_current",
            ArtifactRepositoryMigrationDisposition::Migrated => "migrated",
        };
        let backup_key = result
            .backup_key
            .as_ref()
            .map(|key| key.as_str().to_owned());
        migration_output(
            disposition,
            result.from_schema,
            result.to_schema,
            backup_key,
        )
    }
}

fn migration_output(
    disposition: &'static str,
    from_schema: u32,
    to_schema: u32,
    backup_key: Option<String>,
) -> ModelOutput {
    let mut text =
        format!("disposition: {disposition}\nfrom_schema: {from_schema}\nto_schema: {to_schema}\n");
    if let Some(backup_key) = &backup_key {
        use std::fmt::Write as _;
        writeln!(text, "backup_key: {backup_key}").expect("writing to a String cannot fail");
    }
    let value = serde_json::to_value(MigrationResult {
        disposition,
        from_schema,
        to_schema,
        backup_key,
    })
    .expect("migration DTO serialization is infallible");
    ModelOutput {
        value,
        text,
        findings: false,
    }
}

#[derive(Serialize)]
struct MigrationResult {
    disposition: &'static str,
    from_schema: u32,
    to_schema: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    backup_key: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrated_output_is_stable_and_contains_only_the_backup_key() {
        let output = migration_output(
            "migrated",
            2,
            3,
            Some("migration-backup-v2-to-v3".to_owned()),
        );
        assert_eq!(output.value["disposition"], "migrated");
        assert_eq!(output.value["from_schema"], 2);
        assert_eq!(output.value["to_schema"], 3);
        assert_eq!(output.value["backup_key"], "migration-backup-v2-to-v3");
        assert!(
            output
                .text
                .contains("backup_key: migration-backup-v2-to-v3")
        );
        assert!(!output.has_findings());
    }

    #[test]
    fn current_output_omits_an_absent_backup_key() {
        let output = ModelOutput::migration(&ArtifactRepositoryMigrationResult {
            from_schema: 3,
            to_schema: 3,
            disposition: ArtifactRepositoryMigrationDisposition::AlreadyCurrent,
            backup_key: None,
        });
        assert_eq!(output.value["disposition"], "already_current");
        assert!(output.value.get("backup_key").is_none());
        assert!(!output.text.contains("backup_key"));
    }
}
