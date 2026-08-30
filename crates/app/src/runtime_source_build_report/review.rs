use rewrite_ollama_package::{
    RuntimePackageReviewCheck, RuntimePackageReviewCheckStatus, RuntimePackageReviewDispositionV2,
    RuntimePackageReviewV2Limits,
};
use rewrite_types::CancellationToken;

use crate::{RuntimeSourceBuildEvidenceBundleLease, RuntimeSourceBuildReviewCompiler};

pub(super) fn assert_blocked_build_stage_review(
    evidence: &RuntimeSourceBuildEvidenceBundleLease,
    cancellation: &CancellationToken,
) {
    let review = RuntimeSourceBuildReviewCompiler::compile_build_stage(
        evidence,
        &RuntimePackageReviewV2Limits::default(),
        cancellation,
    )
    .expect("compile and independently verify blocked build-stage review");
    assert!(!review.canonical_review_bytes().is_empty());
    assert!(review.verified().reconstructed_runtime().is_none());
    assert_eq!(
        review
            .verified()
            .review()
            .check_status(RuntimePackageReviewCheck::SourceLineage),
        RuntimePackageReviewCheckStatus::NotRun
    );
    assert_eq!(
        review
            .verified()
            .review()
            .check_status(RuntimePackageReviewCheck::Transformation),
        RuntimePackageReviewCheckStatus::NotRun
    );
    assert!(matches!(
        review.verified().review().disposition(),
        RuntimePackageReviewDispositionV2::NotAdmitted { blockers }
            if blockers == &[
                RuntimePackageReviewCheck::SourceLineage,
                RuntimePackageReviewCheck::Transformation,
                RuntimePackageReviewCheck::License,
                RuntimePackageReviewCheck::NativeClosure,
                RuntimePackageReviewCheck::ManagedStartup,
                RuntimePackageReviewCheck::CloudDisable,
            ]
    ));
}
