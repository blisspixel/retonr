//! Unit tests for verified generation qualification compiler and authority.

use super::*;

#[test]
fn error_display_and_equality() {
    let variants = [
        (
            VerifiedGenerationQualificationError::Cancelled,
            "verified generation qualification was cancelled",
        ),
        (
            VerifiedGenerationQualificationError::NotQualified,
            "generation qualification record status is not qualified",
        ),
        (
            VerifiedGenerationQualificationError::HumanAdjudicationNotPassed,
            "generation qualification human adjudication did not pass",
        ),
        (
            VerifiedGenerationQualificationError::ResourcePhaseNotPassed,
            "generation qualification resource phase did not pass",
        ),
        (
            VerifiedGenerationQualificationError::RepeatabilityNotPassed,
            "generation qualification repeatability phase did not pass",
        ),
        (
            VerifiedGenerationQualificationError::ReceiptNotCompleted,
            "generation qualification operation receipt is not completed",
        ),
        (
            VerifiedGenerationQualificationError::ScopeMismatch,
            "generation qualification scope does not match",
        ),
        (
            VerifiedGenerationQualificationError::JoinAuthority,
            "generation qualification live authority failed",
        ),
        (
            VerifiedGenerationQualificationError::DurableVerification,
            "generation qualification durable verification failed",
        ),
    ];

    for (error, expected) in variants {
        assert_eq!(error, error);
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn compiler_traits_and_debug() {
    let compiler = VerifiedGenerationQualificationCompiler;
    let copy = compiler;
    assert_eq!(format!("{compiler:?}"), format!("{copy:?}"));
    assert!(format!("{compiler:?}").contains("VerifiedGenerationQualificationCompiler"));
}
