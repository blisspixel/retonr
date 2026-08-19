use std::{io, process::ExitCode};

use rewrite_app::{AppError, ClaimExtractionError, EngineError, ProtectionError};

use crate::contract::{
    CommandName, EXIT_CANCELLED, EXIT_COMPATIBILITY, EXIT_OPERATIONAL, EXIT_USAGE, ErrorBody,
    ErrorCategory, ErrorCode,
};

#[derive(Debug)]
pub(crate) struct RunFailure {
    pub command: CommandName,
    pub body: ErrorBody,
    pub exit_code: ExitCode,
    pub message: &'static str,
}

impl RunFailure {
    pub fn usage() -> Self {
        Self::usage_for(CommandName::Cli)
    }

    pub fn usage_for(command: CommandName) -> Self {
        Self {
            command,
            body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
            exit_code: ExitCode::from(crate::contract::EXIT_USAGE),
            message: "command input is invalid",
        }
    }

    pub fn operational(command: CommandName) -> Self {
        Self {
            command,
            body: ErrorBody::new(
                ErrorCategory::Operational,
                ErrorCode::OperationalFailure,
                true,
            ),
            exit_code: ExitCode::from(EXIT_OPERATIONAL),
            message: "operation failed",
        }
    }

    pub fn cancelled(command: CommandName) -> Self {
        Self {
            command,
            body: ErrorBody::new(
                ErrorCategory::Cancelled,
                ErrorCode::OperationCancelled,
                false,
            ),
            exit_code: ExitCode::from(EXIT_CANCELLED),
            message: "operation was cancelled",
        }
    }

    pub fn check_read(error: &io::Error) -> Self {
        Self::input_read(CommandName::Check, error)
    }

    pub fn input_read(command: CommandName, error: &io::Error) -> Self {
        if error.kind() == io::ErrorKind::InvalidInput {
            return Self {
                command,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "input must be a regular file",
            };
        }
        if error.kind() == io::ErrorKind::InvalidData {
            Self {
                command,
                body: ErrorBody::new(
                    ErrorCategory::Compatibility,
                    ErrorCode::ResourceLimitExceeded,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "input exceeds the supported byte limit",
            }
        } else {
            Self {
                command,
                body: ErrorBody::new(
                    ErrorCategory::Operational,
                    ErrorCode::InputUnreadable,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_OPERATIONAL),
                message: "input could not be read",
            }
        }
    }

    pub fn check_app(error: &AppError) -> Self {
        Self::app(CommandName::Check, error)
    }

    pub fn app(command: CommandName, error: &AppError) -> Self {
        match error {
            AppError::CandidateTooLarge { .. }
            | AppError::ClaimExtraction(ClaimExtractionError::TextTooLarge { .. }) => Self {
                command,
                body: ErrorBody::new(
                    ErrorCategory::Compatibility,
                    ErrorCode::ResourceLimitExceeded,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "input exceeds the supported byte limit",
            },
            AppError::TextAdapter(_) => Self {
                command,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "source text is not a supported UTF-8 document",
            },
            AppError::Engine(EngineError::Protection(ProtectionError::ResourceLimit))
            | AppError::Protection(ProtectionError::ResourceLimit) => Self {
                command,
                body: ErrorBody::new(
                    ErrorCategory::Compatibility,
                    ErrorCode::ResourceLimitExceeded,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "input exceeds the supported protection limit",
            },
            AppError::Engine(EngineError::Protection(ProtectionError::MatcherBuild))
            | AppError::Protection(ProtectionError::MatcherBuild)
            | AppError::Grounded(_)
            | AppError::GroundedRepository => Self::operational(command),
            AppError::Engine(_) | AppError::Protection(_) => Self {
                command,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "command input is invalid",
            },
            AppError::GroundedUnavailable => Self {
                command,
                body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "grounded rewrite requires a selected qualified local artifact",
            },
            AppError::GroundedSelectionMismatch => Self {
                command,
                body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "requested artifact is not the active qualified generation binding",
            },
            AppError::GroundedRuntimeUnavailable => Self {
                command,
                body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "grounded rewrite requires an attached local runtime",
            },
            AppError::ClaimExtraction(ClaimExtractionError::Cancelled) => Self::cancelled(command),
            AppError::ClaimExtraction(
                ClaimExtractionError::InvalidRequest | ClaimExtractionError::ManifestMismatch,
            ) => Self::usage_for(command),
            AppError::ClaimExtraction(ClaimExtractionError::Unavailable) => Self {
                command,
                body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "backend is not available for claim pair extraction",
            },
            AppError::ClaimExtraction(_) => Self::operational(command),
        }
    }

    /// A candidate that is not valid UTF-8 is the same defect class as a source.
    pub fn check_invalid_utf8() -> Self {
        Self {
            command: CommandName::Check,
            body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
            exit_code: ExitCode::from(EXIT_USAGE),
            message: "candidate text is not a supported UTF-8 document",
        }
    }

    /// Refuses to replace an existing destination file.
    pub fn output_exists() -> Self {
        Self::output_exists_for(CommandName::Check)
    }

    pub fn output_exists_for(command: CommandName) -> Self {
        Self {
            command,
            body: ErrorBody::new(ErrorCategory::Policy, ErrorCode::OutputExists, false),
            exit_code: ExitCode::from(crate::contract::EXIT_POLICY),
            message: "output destination already exists",
        }
    }

    pub fn from_model(error: crate::model::ModelFailure) -> Self {
        Self {
            command: error.command,
            body: error.body,
            exit_code: error.exit_code,
            message: error.message,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::ExitCode;

    use rewrite_app::{AppError, EngineError, ProtectionError};

    use super::RunFailure;
    use crate::contract::{
        CommandName, EXIT_CANCELLED, EXIT_COMPATIBILITY, ErrorBody, ErrorCategory, ErrorCode,
    };

    #[test]
    fn check_read_classifies_limit_and_missing_inputs() {
        let limit = RunFailure::check_read(&std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "input exceeds the supported byte limit",
        ));
        assert_eq!(limit.command, CommandName::Check);
        assert_eq!(
            limit.body,
            ErrorBody::new(
                ErrorCategory::Compatibility,
                ErrorCode::ResourceLimitExceeded,
                false
            )
        );

        let missing = RunFailure::check_read(&std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        ));
        assert_eq!(
            missing.body,
            ErrorBody::new(
                ErrorCategory::Operational,
                ErrorCode::InputUnreadable,
                false
            )
        );
    }

    #[test]
    fn check_app_maps_protection_limits_and_cancellation() {
        let limit = RunFailure::check_app(&AppError::Engine(EngineError::Protection(
            ProtectionError::ResourceLimit,
        )));
        assert_eq!(
            limit.body,
            ErrorBody::new(
                ErrorCategory::Compatibility,
                ErrorCode::ResourceLimitExceeded,
                false
            )
        );
        assert_eq!(limit.exit_code, ExitCode::from(EXIT_COMPATIBILITY));

        let cancelled = RunFailure::cancelled(CommandName::Check);
        assert_eq!(
            cancelled.body,
            ErrorBody::new(
                ErrorCategory::Cancelled,
                ErrorCode::OperationCancelled,
                false
            )
        );
        assert_eq!(cancelled.exit_code, ExitCode::from(EXIT_CANCELLED));
    }

    #[test]
    fn claim_extraction_failures_map_to_stable_exit_categories() {
        let cancelled = RunFailure::app(
            CommandName::Rewrite,
            &AppError::ClaimExtraction(rewrite_app::ClaimExtractionError::Cancelled),
        );
        assert_eq!(cancelled.exit_code, ExitCode::from(EXIT_CANCELLED));
        assert_eq!(
            cancelled.body,
            ErrorBody::new(
                ErrorCategory::Cancelled,
                ErrorCode::OperationCancelled,
                false
            )
        );

        let unavailable = RunFailure::app(
            CommandName::Rewrite,
            &AppError::ClaimExtraction(rewrite_app::ClaimExtractionError::Unavailable),
        );
        assert_eq!(unavailable.exit_code, ExitCode::from(EXIT_COMPATIBILITY));
        assert_eq!(
            unavailable.body,
            ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false)
        );
    }
}
