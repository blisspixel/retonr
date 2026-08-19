use rewrite_app::{
    ArtifactReconciliationDisposition, ArtifactRepositorySetImportResult,
    ArtifactRepositorySetReconciliationResult, ArtifactSetImportDisposition,
};
use serde::Serialize;

use crate::contract::ArtifactSetSelectionDto;

use super::ModelOutput;

impl ModelOutput {
    pub(crate) fn set_remove(result: &rewrite_app::ArtifactRepositorySetRemovalResult) -> Self {
        let disposition = match result.disposition {
            rewrite_app::ArtifactRemovalDisposition::Removed => "removed",
            rewrite_app::ArtifactRemovalDisposition::Recovered => "recovered",
            rewrite_app::ArtifactRemovalDisposition::AlreadyRemoved => "already_removed",
        };
        set_selection_output(&result.key, disposition)
    }

    pub(crate) fn set_import(result: &ArtifactRepositorySetImportResult) -> Self {
        let disposition = match result.disposition {
            ArtifactSetImportDisposition::Imported => "imported",
            ArtifactSetImportDisposition::RegisteredExisting => "registered_existing",
            ArtifactSetImportDisposition::AlreadyPresent => "already_present",
        };
        set_selection_output(&result.key, disposition)
    }

    pub(crate) fn set_reconcile(result: &ArtifactRepositorySetReconciliationResult) -> Self {
        let disposition = match result.disposition {
            ArtifactReconciliationDisposition::Registered => "registered",
            ArtifactReconciliationDisposition::AlreadyRegistered => "already_registered",
        };
        set_selection_output(&result.key, disposition)
    }
}

fn set_selection_output(
    key: &rewrite_app::ArtifactSetInstallationKey,
    disposition: &'static str,
) -> ModelOutput {
    let selection = ArtifactSetSelectionDto::from(key);
    let text = format!(
        "disposition: {disposition}\nartifact_set_id: {}\ninstallation_generation: {}\n",
        selection.artifact_set_id, selection.installation_generation
    );
    let result = SetSelectionResult {
        selection,
        disposition,
    };
    ModelOutput {
        value: serde_json::to_value(result).expect("set-selection DTO serialization is infallible"),
        text,
        findings: false,
    }
}

#[derive(Serialize)]
struct SetSelectionResult {
    selection: ArtifactSetSelectionDto,
    disposition: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_set_import_disposition_has_a_stable_name() {
        assert_eq!(
            [
                ArtifactSetImportDisposition::Imported,
                ArtifactSetImportDisposition::RegisteredExisting,
                ArtifactSetImportDisposition::AlreadyPresent,
            ]
            .map(|disposition| match disposition {
                ArtifactSetImportDisposition::Imported => "imported",
                ArtifactSetImportDisposition::RegisteredExisting => "registered_existing",
                ArtifactSetImportDisposition::AlreadyPresent => "already_present",
            }),
            ["imported", "registered_existing", "already_present"]
        );
    }
}
