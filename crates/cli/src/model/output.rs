use rewrite_app::{
    ArtifactInventoryReport, ArtifactRepositoryImportDisposition, ArtifactRepositoryImportResult,
    ArtifactRepositoryPendingOperations, ArtifactRepositoryReconciliationResult,
    ArtifactRepositoryRemovalResult, OrphanManifestAssociation, RegisteredArtifactBytes,
};
use rewrite_model::{ArtifactManifest, ArtifactRole};
use serde::Serialize;

use crate::contract::ArtifactSelectionDto;

mod migration;

pub(crate) struct ModelOutput {
    pub(crate) value: serde_json::Value,
    pub(crate) text: String,
    findings: bool,
}

impl ModelOutput {
    pub(crate) fn import(result: &ArtifactRepositoryImportResult) -> Self {
        let disposition = match result.disposition {
            ArtifactRepositoryImportDisposition::Imported => "imported",
            ArtifactRepositoryImportDisposition::AlreadyPresent => "already_present",
        };
        Self::selection(&result.key, disposition)
    }

    pub(crate) fn reconcile(result: &ArtifactRepositoryReconciliationResult) -> Self {
        let disposition = match result.disposition {
            rewrite_app::ArtifactReconciliationDisposition::Registered => "registered",
            rewrite_app::ArtifactReconciliationDisposition::AlreadyRegistered => {
                "already_registered"
            }
        };
        Self::selection(&result.key, disposition)
    }

    pub(crate) fn remove(result: &ArtifactRepositoryRemovalResult) -> Self {
        let disposition = match result.disposition {
            rewrite_app::ArtifactRemovalDisposition::Removed => "removed",
            rewrite_app::ArtifactRemovalDisposition::Recovered => "recovered",
            rewrite_app::ArtifactRemovalDisposition::AlreadyRemoved => "already_removed",
        };
        Self::selection(&result.key, disposition)
    }

    pub(crate) fn inventory(report: ArtifactInventoryReport) -> Self {
        let result = InventoryResult::from_report(report);
        let findings = result.health == InventoryHealth::Findings;
        let mut text = format!(
            "health: {}\nregistered: {}\npending_removals: {}\nmanifest_only: {}\nverified_orphans: {}\ncontent_address_conflicts: {}\noversized_files: {}\nmalformed_names: {}\nindirect_entries: {}\nnon_regular_entries: {}\nempty_files: {}\naliased_files: {}\nstorage_entries: {}\nverified_bytes: {}\n",
            result.health.name(),
            result.registered.len(),
            result.pending_removals.len(),
            result.manifest_only.len(),
            result.verified_orphans.len(),
            result.content_address_conflicts.len(),
            result.oversized_files.len(),
            result.unexpected_entries.malformed_names,
            result.unexpected_entries.indirect_entries,
            result.unexpected_entries.non_regular_entries,
            result.unexpected_entries.empty_files,
            result.unexpected_entries.aliased_files,
            result.storage_entry_count,
            result.verified_bytes,
        );
        for entry in &result.registered {
            use std::fmt::Write as _;
            writeln!(
                text,
                "registered {} generation={} bytes={} status={}",
                entry.selection.artifact_id,
                entry.selection.installation_generation,
                entry.manifest.byte_size,
                entry.bytes.name()
            )
            .expect("writing to a String cannot fail");
        }
        for entry in &result.pending_removals {
            use std::fmt::Write as _;
            writeln!(
                text,
                "pending_removal {} generation={} status={}",
                entry.selection.artifact_id,
                entry.selection.installation_generation,
                entry.bytes.name()
            )
            .expect("writing to a String cannot fail");
        }
        Self {
            value: serde_json::to_value(result).expect("inventory DTO serialization is infallible"),
            text,
            findings,
        }
    }

    pub(crate) fn pending_operations(result: &ArtifactRepositoryPendingOperations) -> Self {
        let artifact_removals: Vec<_> = result
            .artifact_removals
            .iter()
            .map(ArtifactSelectionDto::from)
            .collect();
        let mut text = format!("pending_artifact_removals: {}\n", artifact_removals.len());
        for selection in &artifact_removals {
            use std::fmt::Write as _;
            writeln!(
                text,
                "artifact_removal {} generation={}",
                selection.artifact_id, selection.installation_generation
            )
            .expect("writing to a String cannot fail");
        }
        let value = serde_json::to_value(PendingOperationsResult { artifact_removals })
            .expect("pending-operations DTO serialization is infallible");
        Self {
            value,
            text,
            findings: false,
        }
    }

    pub(crate) const fn has_findings(&self) -> bool {
        self.findings
    }

    fn selection(key: &rewrite_app::ArtifactInstallationKey, disposition: &'static str) -> Self {
        let selection = ArtifactSelectionDto::from(key);
        let text = format!(
            "disposition: {disposition}\nartifact_id: {}\ninstallation_generation: {}\n",
            selection.artifact_id, selection.installation_generation
        );
        let result = SelectionResult {
            selection,
            disposition,
        };
        Self {
            value: serde_json::to_value(result).expect("selection DTO serialization is infallible"),
            text,
            findings: false,
        }
    }
}

#[derive(Serialize)]
struct SelectionResult {
    selection: ArtifactSelectionDto,
    disposition: &'static str,
}

#[derive(Serialize)]
struct PendingOperationsResult {
    artifact_removals: Vec<ArtifactSelectionDto>,
}

#[derive(Serialize)]
struct InventoryResult {
    health: InventoryHealth,
    registered: Vec<RegisteredSummary>,
    pending_removals: Vec<PendingSummary>,
    manifest_only: Vec<ManifestSummary>,
    verified_orphans: Vec<OrphanSummary>,
    content_address_conflicts: Vec<ConflictSummary>,
    oversized_files: Vec<OversizedSummary>,
    unexpected_entries: UnexpectedSummary,
    storage_entry_count: String,
    verified_bytes: String,
}

impl InventoryResult {
    fn from_report(report: ArtifactInventoryReport) -> Self {
        let registered: Vec<_> = report
            .registered
            .into_iter()
            .map(|entry| RegisteredSummary {
                manifest: ManifestSummary::from_manifest(&entry.manifest),
                selection: ArtifactSelectionDto::from(&entry.installation),
                active_roles: entry
                    .active_bindings
                    .iter()
                    .map(|binding| role_name(binding.role))
                    .collect(),
                bytes: RegisteredBytesSummary::from(entry.bytes),
            })
            .collect();
        let pending_removals: Vec<_> = report
            .pending_removals
            .into_iter()
            .map(|entry| PendingSummary {
                selection: ArtifactSelectionDto::from(&entry.selection),
                bytes: RegisteredBytesSummary::from(entry.bytes),
            })
            .collect();
        let manifest_only: Vec<_> = report
            .manifest_only
            .iter()
            .map(ManifestSummary::from_manifest)
            .collect();
        let verified_orphans: Vec<_> = report
            .verified_orphans
            .into_iter()
            .map(|entry| OrphanSummary {
                artifact_id: entry.artifact_id.digest().as_str().to_owned(),
                byte_size: entry.byte_size.to_string(),
                manifest_association: OrphanAssociationSummary::from(entry.manifest),
            })
            .collect();
        let content_address_conflicts: Vec<_> = report
            .content_address_conflicts
            .into_iter()
            .map(|entry| ConflictSummary {
                claimed_artifact_id: entry.claimed_artifact_id.digest().as_str().to_owned(),
                byte_size: entry.byte_size.to_string(),
            })
            .collect();
        let oversized_files: Vec<_> = report
            .oversized_files
            .into_iter()
            .map(|entry| OversizedSummary {
                claimed_artifact_id: entry.claimed_artifact_id.digest().as_str().to_owned(),
                byte_size: entry.byte_size.to_string(),
            })
            .collect();
        let unexpected_entries = UnexpectedSummary {
            malformed_names: report.unexpected_entries.malformed_names.to_string(),
            indirect_entries: report.unexpected_entries.indirect_entries.to_string(),
            non_regular_entries: report.unexpected_entries.non_regular_entries.to_string(),
            empty_files: report.unexpected_entries.empty_files.to_string(),
            aliased_files: report.unexpected_entries.aliased_files.to_string(),
        };
        let has_unexpected = report.unexpected_entries.malformed_names != 0
            || report.unexpected_entries.indirect_entries != 0
            || report.unexpected_entries.non_regular_entries != 0
            || report.unexpected_entries.empty_files != 0
            || report.unexpected_entries.aliased_files != 0;
        let clean = pending_removals.is_empty()
            && manifest_only.is_empty()
            && verified_orphans.is_empty()
            && content_address_conflicts.is_empty()
            && oversized_files.is_empty()
            && !has_unexpected
            && registered
                .iter()
                .all(|entry| matches!(entry.bytes, RegisteredBytesSummary::Verified));
        Self {
            health: if clean {
                InventoryHealth::Clean
            } else {
                InventoryHealth::Findings
            },
            registered,
            pending_removals,
            manifest_only,
            verified_orphans,
            content_address_conflicts,
            oversized_files,
            unexpected_entries,
            storage_entry_count: report.storage_entry_count.to_string(),
            verified_bytes: report.verified_bytes.to_string(),
        }
    }
}

#[derive(Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum InventoryHealth {
    Clean,
    Findings,
}

impl InventoryHealth {
    const fn name(&self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Findings => "findings",
        }
    }
}

#[derive(Serialize)]
struct ManifestSummary {
    artifact_id: String,
    byte_size: String,
}

impl ManifestSummary {
    fn from_manifest(manifest: &ArtifactManifest) -> Self {
        Self {
            artifact_id: manifest.artifact_id.digest().as_str().to_owned(),
            byte_size: manifest.byte_size.to_string(),
        }
    }
}

#[derive(Serialize)]
struct RegisteredSummary {
    manifest: ManifestSummary,
    selection: ArtifactSelectionDto,
    active_roles: Vec<&'static str>,
    bytes: RegisteredBytesSummary,
}

#[derive(Serialize)]
struct PendingSummary {
    selection: ArtifactSelectionDto,
    bytes: RegisteredBytesSummary,
}

#[derive(Serialize)]
struct OrphanSummary {
    artifact_id: String,
    byte_size: String,
    manifest_association: OrphanAssociationSummary,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum OrphanAssociationSummary {
    NoManifest,
    MatchingManifest { manifest: ManifestSummary },
    ManifestSizeConflict { manifest: ManifestSummary },
}

impl From<OrphanManifestAssociation> for OrphanAssociationSummary {
    fn from(value: OrphanManifestAssociation) -> Self {
        match value {
            OrphanManifestAssociation::NoManifest => Self::NoManifest,
            OrphanManifestAssociation::MatchingManifest(manifest) => Self::MatchingManifest {
                manifest: ManifestSummary::from_manifest(&manifest),
            },
            OrphanManifestAssociation::ManifestSizeConflict { manifest } => {
                Self::ManifestSizeConflict {
                    manifest: ManifestSummary::from_manifest(&manifest),
                }
            }
        }
    }
}

#[derive(Serialize)]
struct ConflictSummary {
    claimed_artifact_id: String,
    byte_size: String,
}

#[derive(Serialize)]
struct OversizedSummary {
    claimed_artifact_id: String,
    byte_size: String,
}

#[derive(Serialize)]
struct UnexpectedSummary {
    malformed_names: String,
    indirect_entries: String,
    non_regular_entries: String,
    empty_files: String,
    aliased_files: String,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum RegisteredBytesSummary {
    Verified,
    Missing,
    UnsafeEntry,
    AliasedEntry,
    StateLayoutConflict,
    SizeConflict { observed_bytes: String },
    DigestConflict,
    TooLargeToVerify { observed_bytes: String },
}

impl From<RegisteredArtifactBytes> for RegisteredBytesSummary {
    fn from(value: RegisteredArtifactBytes) -> Self {
        match value {
            RegisteredArtifactBytes::Verified => Self::Verified,
            RegisteredArtifactBytes::Missing => Self::Missing,
            RegisteredArtifactBytes::UnsafeEntry => Self::UnsafeEntry,
            RegisteredArtifactBytes::AliasedEntry => Self::AliasedEntry,
            RegisteredArtifactBytes::StateLayoutConflict => Self::StateLayoutConflict,
            RegisteredArtifactBytes::SizeConflict { observed_bytes } => Self::SizeConflict {
                observed_bytes: observed_bytes.to_string(),
            },
            RegisteredArtifactBytes::DigestConflict { .. } => Self::DigestConflict,
            RegisteredArtifactBytes::TooLargeToVerify { observed_bytes } => {
                Self::TooLargeToVerify {
                    observed_bytes: observed_bytes.to_string(),
                }
            }
        }
    }
}

impl RegisteredBytesSummary {
    const fn name(&self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Missing => "missing",
            Self::UnsafeEntry => "unsafe_entry",
            Self::AliasedEntry => "aliased_entry",
            Self::StateLayoutConflict => "state_layout_conflict",
            Self::SizeConflict { .. } => "size_conflict",
            Self::DigestConflict => "digest_conflict",
            Self::TooLargeToVerify { .. } => "too_large_to_verify",
        }
    }
}

const fn role_name(role: ArtifactRole) -> &'static str {
    role.machine_name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_byte_status_is_named_and_content_redacted() {
        let digest = rewrite_types::Digest::sha256(b"unknown bytes");
        for (value, name) in [
            (RegisteredArtifactBytes::Verified, "verified"),
            (RegisteredArtifactBytes::Missing, "missing"),
            (RegisteredArtifactBytes::UnsafeEntry, "unsafe_entry"),
            (RegisteredArtifactBytes::AliasedEntry, "aliased_entry"),
            (
                RegisteredArtifactBytes::StateLayoutConflict,
                "state_layout_conflict",
            ),
            (
                RegisteredArtifactBytes::SizeConflict { observed_bytes: 7 },
                "size_conflict",
            ),
            (
                RegisteredArtifactBytes::DigestConflict {
                    observed_digest: digest.clone(),
                },
                "digest_conflict",
            ),
            (
                RegisteredArtifactBytes::TooLargeToVerify { observed_bytes: 9 },
                "too_large_to_verify",
            ),
        ] {
            let summary = RegisteredBytesSummary::from(value);
            assert_eq!(summary.name(), name);
            let encoded = serde_json::to_string(&summary).expect("serialize byte status");
            assert!(!encoded.contains(digest.as_str()));
        }
    }

    #[test]
    fn every_role_has_a_stable_name() {
        let names = ArtifactRole::ALL.map(role_name);
        assert_eq!(
            names,
            [
                "generation",
                "embedding",
                "speech_recognition",
                "voice_activity_detection",
                "speech_synthesis",
                "voice",
                "claim_extraction",
            ]
        );
    }

    #[test]
    fn pending_operation_output_is_actionable_and_redacted() {
        let key = rewrite_app::ArtifactInstallationKey::new(
            rewrite_model::ArtifactId::from_digest(rewrite_types::Digest::sha256(b"artifact")),
            7,
        )
        .expect("valid fixture key");
        let output = ModelOutput::pending_operations(&ArtifactRepositoryPendingOperations {
            artifact_removals: vec![key],
        });
        assert!(output.text.contains("pending_artifact_removals: 1"));
        assert!(output.text.contains("generation=7"));
        assert_eq!(
            output.value["artifact_removals"][0]["installation_generation"],
            "7"
        );
        assert!(!output.text.contains("artifact-storage"));
        assert!(!output.value.to_string().contains("artifact-storage"));
    }
}
