use std::{io, process::ExitCode};

use rewrite_app::{AppError, EngineError, ProtectionError};

use crate::contract::{
    CommandName, EXIT_CANCELLED, EXIT_COMPATIBILITY, EXIT_OPERATIONAL, EXIT_USAGE, ErrorBody,
    ErrorCategory, ErrorCode,
};

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
        if error.kind() == io::ErrorKind::InvalidInput {
            return Self {
                command: CommandName::Check,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "input must be a regular file",
            };
        }
        if error.kind() == io::ErrorKind::InvalidData {
            Self {
                command: CommandName::Check,
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
                command: CommandName::Check,
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
        match error {
            AppError::CandidateTooLarge { .. } => Self {
                command: CommandName::Check,
                body: ErrorBody::new(
                    ErrorCategory::Compatibility,
                    ErrorCode::ResourceLimitExceeded,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "input exceeds the supported byte limit",
            },
            AppError::TextAdapter(_) => Self {
                command: CommandName::Check,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "source text is not a supported UTF-8 document",
            },
            AppError::Engine(EngineError::Protection(ProtectionError::ResourceLimit))
            | AppError::Protection(ProtectionError::ResourceLimit) => Self {
                command: CommandName::Check,
                body: ErrorBody::new(
                    ErrorCategory::Compatibility,
                    ErrorCode::ResourceLimitExceeded,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "input exceeds the supported protection limit",
            },
            AppError::Engine(EngineError::Protection(ProtectionError::MatcherBuild))
            | AppError::Protection(ProtectionError::MatcherBuild) => {
                Self::operational(CommandName::Check)
            }
            AppError::Engine(_) | AppError::Protection(_) => Self {
                command: CommandName::Check,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "command input is invalid",
            },
            AppError::Grounded(_) => Self::operational(CommandName::Check),
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
}
