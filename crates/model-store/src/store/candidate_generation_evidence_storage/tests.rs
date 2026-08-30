use rewrite_model::{
    ArtifactSetRelativePath, CandidateGenerationEvidenceBundleId, GenerationQualificationPlanId,
    PlannedCandidateAttemptId,
};
use rewrite_types::Digest;
use serde::de::DeserializeOwned;

use super::*;

fn id<T: DeserializeOwned>(label: &str) -> T {
    serde_json::from_value(serde_json::Value::String(
        Digest::sha256(label.as_bytes()).as_str().to_owned(),
    ))
    .expect("typed digest ID")
}

#[test]
fn canonical_reference_is_derived_only_from_typed_ids() {
    let root =
        CandidateGenerationEvidenceStorageRootId::new(Digest::sha256(b"storage root").as_str())
            .expect("root ID");
    let plan: GenerationQualificationPlanId = id("plan");
    let attempt: PlannedCandidateAttemptId = id("attempt");
    let bundle: CandidateGenerationEvidenceBundleId = id("bundle");
    let limits =
        CandidateGenerationEvidenceStorageV1Limits::new(8_193, 256, 268_435_456).expect("limits");
    let storage = CandidateGenerationEvidenceBundleStorageV1::new(
        root.clone(),
        plan.clone(),
        attempt.clone(),
        bundle.clone(),
        limits,
    )
    .expect("storage reference");
    assert_eq!(
        storage.relative_reference().as_str(),
        format!(
            "bundles/v1/{}/{}/{}",
            plan.digest().as_str(),
            attempt.digest().as_str(),
            bundle.digest().as_str(),
        ),
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleStorageV1::from_stored_reference(
            root,
            plan,
            attempt,
            bundle,
            storage.relative_reference().clone(),
            limits,
        )
        .expect("reload reference"),
        storage,
    );
}

#[test]
fn stored_reference_rejects_aliases_and_substitution() {
    let root =
        CandidateGenerationEvidenceStorageRootId::new(Digest::sha256(b"storage root").as_str())
            .expect("root ID");
    let plan: GenerationQualificationPlanId = id("plan");
    let attempt: PlannedCandidateAttemptId = id("attempt");
    let bundle: CandidateGenerationEvidenceBundleId = id("bundle");
    let limits = CandidateGenerationEvidenceStorageV1Limits::new(1, 1, 1).expect("limits");
    for reference in [
        format!(
            "bundles/v1/{}/{}/{}",
            plan.digest().as_str().to_ascii_uppercase(),
            attempt.digest().as_str(),
            bundle.digest().as_str(),
        ),
        format!(
            "bundles/v1/extra/{}/{}/{}",
            plan.digest().as_str(),
            attempt.digest().as_str(),
            bundle.digest().as_str(),
        ),
        format!(
            "bundles/v1/{}/{}/{}",
            id::<GenerationQualificationPlanId>("foreign plan")
                .digest()
                .as_str(),
            attempt.digest().as_str(),
            bundle.digest().as_str(),
        ),
    ] {
        let Ok(reference) = ArtifactSetRelativePath::new(reference) else {
            continue;
        };
        assert_eq!(
            CandidateGenerationEvidenceBundleStorageV1::from_stored_reference(
                root.clone(),
                plan.clone(),
                attempt.clone(),
                bundle.clone(),
                reference,
                limits,
            ),
            Err(CandidateGenerationEvidenceStorageContractError::InvalidReference),
        );
    }
}

#[test]
fn root_and_every_limit_enforce_exact_bounds() {
    for invalid in [
        String::new(),
        "a".repeat(63),
        "A".repeat(64),
        "g".repeat(64),
    ] {
        assert_eq!(
            CandidateGenerationEvidenceStorageRootId::new(invalid),
            Err(CandidateGenerationEvidenceStorageContractError::InvalidRootId),
        );
    }
    assert!(CandidateGenerationEvidenceStorageV1Limits::new(8_193, 256, 268_435_456).is_ok());
    for invalid in [
        CandidateGenerationEvidenceStorageV1Limits::new(0, 1, 1),
        CandidateGenerationEvidenceStorageV1Limits::new(8_194, 1, 1),
        CandidateGenerationEvidenceStorageV1Limits::new(1, 0, 1),
        CandidateGenerationEvidenceStorageV1Limits::new(1, 257, 1),
        CandidateGenerationEvidenceStorageV1Limits::new(1, 1, 0),
        CandidateGenerationEvidenceStorageV1Limits::new(1, 1, 268_435_457),
    ] {
        assert_eq!(
            invalid,
            Err(CandidateGenerationEvidenceStorageContractError::InvalidLimit),
        );
    }
}
