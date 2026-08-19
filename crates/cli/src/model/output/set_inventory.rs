use rewrite_app::{
    ArtifactSetInventoryReport, ArtifactSetTreeConflict, OversizedArtifactSet,
    RegisteredArtifactSetBytes, RegisteredArtifactSetInspection, UnexpectedArtifactSetEntryCounts,
    VerifiedArtifactSetOrphan,
};
use rewrite_model::ArtifactSetManifest;
use serde::Serialize;

use crate::contract::ArtifactSetSelectionDto;

use super::ModelOutput;

impl ModelOutput {
    pub(crate) fn set_inventory(report: ArtifactSetInventoryReport) -> Self {
        let result = SetInventoryResult::from_report(report);
        let findings = result.health == SetInventoryHealth::Findings;
        let mut text = format!(
            "health: {}\nregistered: {}\nmanifest_only: {}\nverified_orphans: {}\ntree_conflicts: {}\noversized_sets: {}\nmalformed_names: {}\nindirect_entries: {}\nnon_directory_entries: {}\nunregistered_roots: {}\nstorage_entries: {}\nverified_bytes: {}\n",
            result.health.name(),
            result.registered.len(),
            result.manifest_only.len(),
            result.verified_orphans.len(),
            result.tree_conflicts.len(),
            result.oversized_sets.len(),
            result.unexpected_entries.malformed_names,
            result.unexpected_entries.indirect_entries,
            result.unexpected_entries.non_directory_entries,
            result.unexpected_entries.unregistered_roots,
            result.storage_entry_count,
            result.verified_bytes,
        );
        for entry in &result.registered {
            use std::fmt::Write as _;
            writeln!(
                text,
                "registered {} generation={} members={} status={}",
                entry.selection.artifact_set_id,
                entry.selection.installation_generation,
                entry.manifest.member_count,
                entry.bytes.name()
            )
            .expect("writing to a String cannot fail");
        }
        Self {
            value: serde_json::to_value(result)
                .expect("set inventory DTO serialization is infallible"),
            text,
            findings,
        }
    }
}

#[derive(Serialize)]
struct SetInventoryResult {
    health: SetInventoryHealth,
    registered: Vec<RegisteredSetSummary>,
    manifest_only: Vec<SetManifestSummary>,
    verified_orphans: Vec<SetOrphanSummary>,
    tree_conflicts: Vec<SetConflictSummary>,
    oversized_sets: Vec<SetOversizedSummary>,
    unexpected_entries: SetUnexpectedSummary,
    storage_entry_count: String,
    verified_bytes: String,
}

impl SetInventoryResult {
    fn from_report(report: ArtifactSetInventoryReport) -> Self {
        let registered: Vec<_> = report
            .registered
            .into_iter()
            .map(RegisteredSetSummary::from)
            .collect();
        let manifest_only: Vec<_> = report
            .manifest_only
            .iter()
            .map(SetManifestSummary::from_manifest)
            .collect();
        let verified_orphans: Vec<_> = report
            .verified_orphans
            .into_iter()
            .map(SetOrphanSummary::from)
            .collect();
        let tree_conflicts: Vec<_> = report
            .tree_conflicts
            .into_iter()
            .map(SetConflictSummary::from)
            .collect();
        let oversized_sets: Vec<_> = report
            .oversized_sets
            .into_iter()
            .map(SetOversizedSummary::from)
            .collect();
        let unexpected_entries = SetUnexpectedSummary::from(report.unexpected_entries);
        let has_unexpected = report.unexpected_entries.malformed_names != 0
            || report.unexpected_entries.indirect_entries != 0
            || report.unexpected_entries.non_directory_entries != 0
            || report.unexpected_entries.unregistered_roots != 0;
        let clean = manifest_only.is_empty()
            && verified_orphans.is_empty()
            && tree_conflicts.is_empty()
            && oversized_sets.is_empty()
            && !has_unexpected
            && registered
                .iter()
                .all(|entry| matches!(entry.bytes, RegisteredSetBytesSummary::Verified));
        Self {
            health: if clean {
                SetInventoryHealth::Clean
            } else {
                SetInventoryHealth::Findings
            },
            registered,
            manifest_only,
            verified_orphans,
            tree_conflicts,
            oversized_sets,
            unexpected_entries,
            storage_entry_count: report.storage_entry_count.to_string(),
            verified_bytes: report.verified_bytes.to_string(),
        }
    }
}

#[derive(Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SetInventoryHealth {
    Clean,
    Findings,
}

impl SetInventoryHealth {
    const fn name(&self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Findings => "findings",
        }
    }
}

#[derive(Serialize)]
struct SetManifestSummary {
    artifact_set_id: String,
    member_count: String,
    byte_size: String,
}

impl SetManifestSummary {
    fn from_manifest(manifest: &ArtifactSetManifest) -> Self {
        Self {
            artifact_set_id: manifest.artifact_set_id().digest().as_str().to_owned(),
            member_count: manifest.members().len().to_string(),
            byte_size: manifest.total_byte_size().to_string(),
        }
    }
}

#[derive(Serialize)]
struct RegisteredSetSummary {
    manifest: SetManifestSummary,
    selection: ArtifactSetSelectionDto,
    bytes: RegisteredSetBytesSummary,
}

impl From<RegisteredArtifactSetInspection> for RegisteredSetSummary {
    fn from(value: RegisteredArtifactSetInspection) -> Self {
        Self {
            manifest: SetManifestSummary::from_manifest(&value.manifest),
            selection: ArtifactSetSelectionDto::from(&value.installation),
            bytes: RegisteredSetBytesSummary::from(value.bytes),
        }
    }
}

#[derive(Serialize)]
struct SetOrphanSummary {
    artifact_set_id: String,
    byte_size: String,
}

impl From<VerifiedArtifactSetOrphan> for SetOrphanSummary {
    fn from(value: VerifiedArtifactSetOrphan) -> Self {
        Self {
            artifact_set_id: value.artifact_set_id.digest().as_str().to_owned(),
            byte_size: value.byte_size.to_string(),
        }
    }
}

#[derive(Serialize)]
struct SetConflictSummary {
    artifact_set_id: String,
    byte_size: String,
}

impl From<ArtifactSetTreeConflict> for SetConflictSummary {
    fn from(value: ArtifactSetTreeConflict) -> Self {
        Self {
            artifact_set_id: value.artifact_set_id.digest().as_str().to_owned(),
            byte_size: value.byte_size.to_string(),
        }
    }
}

#[derive(Serialize)]
struct SetOversizedSummary {
    artifact_set_id: String,
    byte_size: String,
}

impl From<OversizedArtifactSet> for SetOversizedSummary {
    fn from(value: OversizedArtifactSet) -> Self {
        Self {
            artifact_set_id: value.artifact_set_id.digest().as_str().to_owned(),
            byte_size: value.byte_size.to_string(),
        }
    }
}

#[derive(Serialize)]
struct SetUnexpectedSummary {
    malformed_names: String,
    indirect_entries: String,
    non_directory_entries: String,
    unregistered_roots: String,
}

impl From<UnexpectedArtifactSetEntryCounts> for SetUnexpectedSummary {
    fn from(value: UnexpectedArtifactSetEntryCounts) -> Self {
        Self {
            malformed_names: value.malformed_names.to_string(),
            indirect_entries: value.indirect_entries.to_string(),
            non_directory_entries: value.non_directory_entries.to_string(),
            unregistered_roots: value.unregistered_roots.to_string(),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum RegisteredSetBytesSummary {
    Verified,
    Missing,
    UnsafeEntry,
    StateLayoutConflict,
    TreeMismatch,
    MemberSizeConflict { observed_bytes: String },
    MemberDigestConflict,
    TooLargeToVerify { observed_bytes: String },
}

impl From<RegisteredArtifactSetBytes> for RegisteredSetBytesSummary {
    fn from(value: RegisteredArtifactSetBytes) -> Self {
        match value {
            RegisteredArtifactSetBytes::Verified => Self::Verified,
            RegisteredArtifactSetBytes::Missing => Self::Missing,
            RegisteredArtifactSetBytes::UnsafeEntry => Self::UnsafeEntry,
            RegisteredArtifactSetBytes::StateLayoutConflict => Self::StateLayoutConflict,
            RegisteredArtifactSetBytes::TreeMismatch => Self::TreeMismatch,
            RegisteredArtifactSetBytes::MemberSizeConflict { observed_bytes } => {
                Self::MemberSizeConflict {
                    observed_bytes: observed_bytes.to_string(),
                }
            }
            RegisteredArtifactSetBytes::MemberDigestConflict => Self::MemberDigestConflict,
            RegisteredArtifactSetBytes::TooLargeToVerify { observed_bytes } => {
                Self::TooLargeToVerify {
                    observed_bytes: observed_bytes.to_string(),
                }
            }
        }
    }
}

impl RegisteredSetBytesSummary {
    const fn name(&self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Missing => "missing",
            Self::UnsafeEntry => "unsafe_entry",
            Self::StateLayoutConflict => "state_layout_conflict",
            Self::TreeMismatch => "tree_mismatch",
            Self::MemberSizeConflict { .. } => "member_size_conflict",
            Self::MemberDigestConflict => "member_digest_conflict",
            Self::TooLargeToVerify { .. } => "too_large_to_verify",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_set_byte_status_is_named_and_content_redacted() {
        for (value, name) in [
            (RegisteredArtifactSetBytes::Verified, "verified"),
            (RegisteredArtifactSetBytes::Missing, "missing"),
            (RegisteredArtifactSetBytes::UnsafeEntry, "unsafe_entry"),
            (
                RegisteredArtifactSetBytes::StateLayoutConflict,
                "state_layout_conflict",
            ),
            (RegisteredArtifactSetBytes::TreeMismatch, "tree_mismatch"),
            (
                RegisteredArtifactSetBytes::MemberSizeConflict { observed_bytes: 7 },
                "member_size_conflict",
            ),
            (
                RegisteredArtifactSetBytes::MemberDigestConflict,
                "member_digest_conflict",
            ),
            (
                RegisteredArtifactSetBytes::TooLargeToVerify { observed_bytes: 9 },
                "too_large_to_verify",
            ),
        ] {
            let summary = RegisteredSetBytesSummary::from(value);
            assert_eq!(summary.name(), name);
            let encoded = serde_json::to_string(&summary).expect("serialize set byte status");
            assert!(!encoded.contains("model/weights.bin"));
        }
    }
}
