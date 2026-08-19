use std::process::ExitCode;

use rewrite_app::{ArtifactRepositoryError, ArtifactRepositoryErrorKind};

use crate::contract::{
    ArtifactSelectionDto, CommandName, EXIT_CANCELLED, EXIT_COMPATIBILITY, EXIT_OPERATIONAL,
    EXIT_POLICY, EXIT_RECOVERY_REQUIRED, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode,
    ManifestInputError,
};

pub(crate) struct ModelFailure {
    pub(crate) command: CommandName,
    pub(crate) body: ErrorBody,
    pub(crate) exit_code: ExitCode,
    pub(crate) message: &'static str,
}

impl ModelFailure {
    pub(crate) fn artifact_not_found(command: CommandName) -> Self {
        Self::new(
            command,
            ErrorCategory::Policy,
            ErrorCode::ArtifactNotFound,
            EXIT_POLICY,
            false,
            "selected artifact was not found",
        )
    }

    pub(crate) fn missing_data_directory(command: CommandName) -> Self {
        Self::new(
            command,
            ErrorCategory::Usage,
            ErrorCode::InvalidInvocation,
            EXIT_USAGE,
            false,
            "model commands require --data-dir",
        )
    }

    pub(crate) fn confirmation_required() -> Self {
        Self::confirmation_required_for(CommandName::ModelRemove)
    }

    pub(crate) fn confirmation_required_for(command: CommandName) -> Self {
        Self::new(
            command,
            ErrorCategory::Policy,
            ErrorCode::ConfirmationRequired,
            EXIT_POLICY,
            false,
            "removal requires --yes",
        )
    }

    pub(crate) fn migration_confirmation_required() -> Self {
        Self::new(
            CommandName::ModelMigrate,
            ErrorCategory::Policy,
            ErrorCode::ConfirmationRequired,
            EXIT_POLICY,
            false,
            "repository migration requires --yes",
        )
    }

    pub(crate) fn recovery_confirmation_required() -> Self {
        Self::recovery_confirmation_required_for(CommandName::ModelRecoverRemoval)
    }

    pub(crate) fn recovery_confirmation_required_for(command: CommandName) -> Self {
        Self::new(
            command,
            ErrorCategory::Policy,
            ErrorCode::ConfirmationRequired,
            EXIT_POLICY,
            false,
            "removal recovery requires --yes",
        )
    }

    pub(crate) fn manifest(command: CommandName, error: ManifestInputError) -> Self {
        let (category, code, exit_code) = match error {
            ManifestInputError::Io(_) => (
                ErrorCategory::Operational,
                ErrorCode::InputUnreadable,
                EXIT_OPERATIONAL,
            ),
            ManifestInputError::InvalidLimit => (
                ErrorCategory::Operational,
                ErrorCode::OperationalFailure,
                EXIT_OPERATIONAL,
            ),
            ManifestInputError::TooLarge => (
                ErrorCategory::Compatibility,
                ErrorCode::ResourceLimitExceeded,
                EXIT_COMPATIBILITY,
            ),
            ManifestInputError::UnsupportedSchema => (
                ErrorCategory::Compatibility,
                ErrorCode::Unsupported,
                EXIT_COMPATIBILITY,
            ),
            ManifestInputError::InvalidJson | ManifestInputError::InvalidManifest => {
                (ErrorCategory::Usage, ErrorCode::InvalidManifest, EXIT_USAGE)
            }
        };
        Self::new(
            command,
            category,
            code,
            exit_code,
            false,
            "artifact manifest could not be accepted",
        )
    }

    pub(crate) fn repository(command: CommandName, error: &ArtifactRepositoryError) -> Self {
        let (category, mut code, exit, retryable, mut message) =
            repository_error_details(error.kind());
        if command == CommandName::ModelRecoverRemoval
            && matches!(error, ArtifactRepositoryError::RemovalRecoveryNotPending)
        {
            code = ErrorCode::RemovalRecoveryNotPending;
            message = "artifact has no prepared removal to recover";
        }
        if command == CommandName::ModelRecoverSetRemoval
            && matches!(error, ArtifactRepositoryError::SetRemovalRecoveryNotPending)
        {
            code = ErrorCode::RemovalRecoveryNotPending;
            message = "artifact set has no prepared removal to recover";
        }
        let mut failure = Self::new(command, category, code, exit, retryable, message);
        if let Some(key) = error.recovery_key() {
            failure.body = failure
                .body
                .with_recovery_selection(ArtifactSelectionDto::from(key));
        }
        if let Some(key) = error.set_recovery_key() {
            failure.body = failure
                .body
                .with_set_recovery_selection(crate::contract::ArtifactSetSelectionDto::from(key));
        }
        if let Some(backup_key) = error.migration_backup_key() {
            failure.body = failure
                .body
                .with_migration_backup_key(backup_key.as_str().to_owned());
        }
        failure
    }

    fn new(
        command: CommandName,
        category: ErrorCategory,
        code: ErrorCode,
        exit_code: u8,
        retryable: bool,
        message: &'static str,
    ) -> Self {
        Self {
            command,
            body: ErrorBody::new(category, code, retryable),
            exit_code: ExitCode::from(exit_code),
            message,
        }
    }
}

fn repository_error_details(
    kind: ArtifactRepositoryErrorKind,
) -> (ErrorCategory, ErrorCode, u8, bool, &'static str) {
    use ArtifactRepositoryErrorKind as Kind;
    match kind {
        Kind::InvalidInput => invalid_input_details(),
        Kind::NotInitialized => (
            ErrorCategory::Operational,
            ErrorCode::RepositoryNotInitialized,
            EXIT_OPERATIONAL,
            false,
            "artifact repository is not initialized",
        ),
        Kind::InUse => (
            ErrorCategory::Operational,
            ErrorCode::RepositoryInUse,
            EXIT_OPERATIONAL,
            true,
            "artifact repository is in use",
        ),
        Kind::ResourceLimit => (
            ErrorCategory::Compatibility,
            ErrorCode::ResourceLimitExceeded,
            EXIT_COMPATIBILITY,
            false,
            "artifact operation reached a configured resource limit",
        ),
        Kind::NotFound => (
            ErrorCategory::Policy,
            ErrorCode::ArtifactNotFound,
            EXIT_POLICY,
            false,
            "selected artifact was not found",
        ),
        Kind::StaleSelection => (
            ErrorCategory::Policy,
            ErrorCode::StaleInstallation,
            EXIT_POLICY,
            false,
            "selected installation generation is stale",
        ),
        Kind::ActiveArtifact => (
            ErrorCategory::Policy,
            ErrorCode::ArtifactActive,
            EXIT_POLICY,
            false,
            "active artifact cannot be removed",
        ),
        Kind::Conflict => (
            ErrorCategory::Policy,
            ErrorCode::ArtifactConflict,
            EXIT_POLICY,
            false,
            "artifact bytes, manifest, or immutable state conflict",
        ),
        Kind::ConcurrentModification => (
            ErrorCategory::Operational,
            ErrorCode::ConcurrentModification,
            EXIT_OPERATIONAL,
            true,
            "artifact storage or state changed during the operation",
        ),
        Kind::CorruptState => (
            ErrorCategory::Operational,
            ErrorCode::CorruptState,
            EXIT_OPERATIONAL,
            false,
            "artifact repository state failed integrity validation",
        ),
        Kind::IncompatibleState => (
            ErrorCategory::Compatibility,
            ErrorCode::IncompatibleState,
            EXIT_COMPATIBILITY,
            false,
            "artifact repository state schema is incompatible",
        ),
        Kind::RecoveryRequired => (
            ErrorCategory::Recovery,
            ErrorCode::ArtifactRemovalRecoveryRequired,
            EXIT_RECOVERY_REQUIRED,
            false,
            "artifact removal requires exact recovery",
        ),
        Kind::Cancelled => (
            ErrorCategory::Cancelled,
            ErrorCode::OperationCancelled,
            EXIT_CANCELLED,
            false,
            "artifact operation was cancelled",
        ),
        Kind::Operational => (
            ErrorCategory::Operational,
            ErrorCode::OperationalFailure,
            EXIT_OPERATIONAL,
            false,
            "artifact operation failed",
        ),
    }
}

const fn invalid_input_details() -> (ErrorCategory, ErrorCode, u8, bool, &'static str) {
    (
        ErrorCategory::Usage,
        ErrorCode::InvalidInvocation,
        EXIT_USAGE,
        false,
        "artifact request is invalid",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_repository_kind_has_a_stable_nonempty_mapping() {
        for (kind, expected_exit) in [
            (ArtifactRepositoryErrorKind::InvalidInput, EXIT_USAGE),
            (
                ArtifactRepositoryErrorKind::NotInitialized,
                EXIT_OPERATIONAL,
            ),
            (ArtifactRepositoryErrorKind::InUse, EXIT_OPERATIONAL),
            (
                ArtifactRepositoryErrorKind::ResourceLimit,
                EXIT_COMPATIBILITY,
            ),
            (ArtifactRepositoryErrorKind::NotFound, EXIT_POLICY),
            (ArtifactRepositoryErrorKind::StaleSelection, EXIT_POLICY),
            (ArtifactRepositoryErrorKind::ActiveArtifact, EXIT_POLICY),
            (ArtifactRepositoryErrorKind::Conflict, EXIT_POLICY),
            (
                ArtifactRepositoryErrorKind::ConcurrentModification,
                EXIT_OPERATIONAL,
            ),
            (ArtifactRepositoryErrorKind::CorruptState, EXIT_OPERATIONAL),
            (
                ArtifactRepositoryErrorKind::IncompatibleState,
                EXIT_COMPATIBILITY,
            ),
            (
                ArtifactRepositoryErrorKind::RecoveryRequired,
                EXIT_RECOVERY_REQUIRED,
            ),
            (ArtifactRepositoryErrorKind::Cancelled, EXIT_CANCELLED),
            (ArtifactRepositoryErrorKind::Operational, EXIT_OPERATIONAL),
        ] {
            let (category, code, exit, _, message) = repository_error_details(kind);
            assert_eq!(exit, expected_exit);
            assert!(!message.is_empty());
            let encoded = serde_json::to_string(&ErrorBody::new(category, code, false))
                .expect("serialize mapped error");
            assert!(encoded.contains("category"));
        }
    }

    #[test]
    fn manifest_failures_retain_usage_compatibility_and_io_categories() {
        for (error, exit) in [
            (ManifestInputError::InvalidJson, EXIT_USAGE),
            (ManifestInputError::InvalidManifest, EXIT_USAGE),
            (ManifestInputError::UnsupportedSchema, EXIT_COMPATIBILITY),
            (ManifestInputError::TooLarge, EXIT_COMPATIBILITY),
            (ManifestInputError::InvalidLimit, EXIT_OPERATIONAL),
            (
                ManifestInputError::Io(std::io::ErrorKind::PermissionDenied),
                EXIT_OPERATIONAL,
            ),
        ] {
            let failure = ModelFailure::manifest(CommandName::ModelImport, error);
            assert_eq!(failure.exit_code, ExitCode::from(exit));
            assert_eq!(failure.command, CommandName::ModelImport);
        }
    }
}
