use rewrite_model::GenerationSuiteManifestV1;
use rewrite_types::{CancellationToken, Digest};

use super::*;

pub(crate) mod support;
use support::{Fixture, PRIMARY_SOURCE, digest, material_limits};

#[test]
fn complete_material_is_ordered_callback_scoped_and_redacted() {
    let fixture = Fixture::primary();
    let material = fixture.verify().expect("material verifies");
    assert_eq!(material.suite(), &fixture.suite);
    assert_eq!(material.cases(), fixture.cases);
    assert_eq!(material.contracts(), fixture.contracts);
    assert_eq!(material.case_count(), 1);
    assert_eq!(material.total_source_bytes(), PRIMARY_SOURCE.len() as u64);
    assert_eq!(
        material.case_material_set_digest().as_str(),
        "97a51deaefc036e6f3390b45ee02bd4e5b320bd807eca00c8514f4b82fc8cfb8"
    );
    let observed = material
        .with_case_source_bytes(0, &CancellationToken::new(), |bytes| {
            assert_eq!(bytes, PRIMARY_SOURCE);
            Digest::sha256(bytes)
        })
        .expect("callback completes");
    assert_eq!(&observed, fixture.cases[0].source_digest());
    material
        .revalidate(&CancellationToken::new())
        .expect("material remains exact");

    let debug = format!("{material:?}");
    assert!(debug.contains("case_material_set_digest"));
    assert!(debug.contains("case_count: 1"));
    assert!(!debug.contains("Retain Acme"));
    assert!(!debug.contains("protected-literal"));
    assert!(!debug.contains(fixture.root.to_string_lossy().as_ref()));
}

#[test]
fn semantic_order_and_suite_identity_change_the_material_digest() {
    let forward_fixture = Fixture::pair();
    let forward = forward_fixture.verify().expect("forward material verifies");
    let reversed_fixture = Fixture::reversed_pair();
    let reversed = reversed_fixture
        .verify()
        .expect("reversed material verifies");
    assert_ne!(
        forward.case_material_set_digest(),
        reversed.case_material_set_digest()
    );
    assert_ne!(
        forward.suite().suite_manifest_id(),
        reversed.suite().suite_manifest_id()
    );

    let changed_protocol_fixture = Fixture::pair_with_protocol("other suite protocol");
    let changed_protocol = changed_protocol_fixture
        .verify()
        .expect("changed protocol material verifies");
    assert_eq!(forward.cases(), changed_protocol.cases());
    assert_ne!(
        forward.case_material_set_digest(),
        changed_protocol.case_material_set_digest()
    );
}

#[test]
fn all_source_traversal_uses_one_complete_material_bracket() {
    let fixture = Fixture::pair();
    let material = fixture.verify().expect("material verifies");
    assert_eq!(material.revalidation_call_count(), 0);
    let digests = material
        .with_all_case_source_bytes(&CancellationToken::new(), |index, bytes| {
            (index, Digest::sha256(bytes))
        })
        .expect("all sources traverse");
    assert_eq!(digests.len(), 2);
    assert_eq!(digests[0], (0, fixture.cases[0].source_digest().clone()));
    assert_eq!(digests[1], (1, fixture.cases[1].source_digest().clone()));
    assert_eq!(material.revalidation_call_count(), 2);
}

#[test]
fn fallible_all_source_traversal_stops_and_still_closes_the_bracket() {
    let fixture = Fixture::pair();
    let material = fixture.verify().expect("material verifies");
    let callback_count = std::cell::Cell::new(0_usize);
    let result = material.try_with_all_case_source_bytes(
        &CancellationToken::new(),
        |index, _bytes| -> Result<(), std::io::Error> {
            callback_count.set(callback_count.get() + 1);
            Err(std::io::Error::other(format!("reject {index}")))
        },
    );
    assert!(matches!(
        result,
        Err(VerifiedGenerationCaseMaterialTraversalError::Callback {
            semantic_index: 0,
            ..
        })
    ));
    assert_eq!(callback_count.get(), 1);
    assert_eq!(material.revalidation_call_count(), 2);
}

#[test]
fn fallible_traversal_preserves_callback_source_and_final_cancellation() {
    let fixture = Fixture::pair();
    let material = fixture.verify().expect("material verifies");
    let cancellation = CancellationToken::new();
    let callback_token = cancellation.clone();
    let result = material.try_with_all_case_source_bytes(
        &cancellation,
        move |_index, _bytes| -> Result<(), std::io::Error> {
            callback_token.cancel();
            Err(std::io::Error::other("callback failure"))
        },
    );
    assert!(matches!(
        result,
        Err(
            VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
                semantic_index: 0,
                final_validation: Some(_),
                ..
            }
        )
    ));
    assert_eq!(material.revalidation_call_count(), 2);
}

#[test]
fn rejects_invalid_and_exceeded_hard_bounds() {
    for invalid in [
        GenerationCaseMaterialLimits {
            maximum_cases: 0,
            ..material_limits()
        },
        GenerationCaseMaterialLimits {
            maximum_cases: MAX_GENERATION_CASE_MATERIAL_CASES + 1,
            ..material_limits()
        },
        GenerationCaseMaterialLimits {
            maximum_total_source_bytes: 0,
            ..material_limits()
        },
        GenerationCaseMaterialLimits {
            maximum_total_source_bytes: MAX_GENERATION_CASE_MATERIAL_TOTAL_SOURCE_BYTES + 1,
            ..material_limits()
        },
    ] {
        let fixture = Fixture::primary();
        assert!(matches!(
            VerifiedGenerationCaseMaterial::verify(
                fixture.suite.clone(),
                fixture.cases.clone(),
                fixture.contracts.clone(),
                fixture.leases(),
                invalid,
                &CancellationToken::new(),
            ),
            Err(VerifiedGenerationCaseMaterialError::InvalidLimits)
        ));
    }

    let case_limited = Fixture::pair();
    assert!(matches!(
        VerifiedGenerationCaseMaterial::verify(
            case_limited.suite.clone(),
            case_limited.cases.clone(),
            case_limited.contracts.clone(),
            case_limited.leases(),
            GenerationCaseMaterialLimits {
                maximum_cases: 1,
                ..material_limits()
            },
            &CancellationToken::new(),
        ),
        Err(VerifiedGenerationCaseMaterialError::CaseLimitExceeded)
    ));

    let byte_limited = Fixture::primary();
    assert!(matches!(
        VerifiedGenerationCaseMaterial::verify(
            byte_limited.suite.clone(),
            byte_limited.cases.clone(),
            byte_limited.contracts.clone(),
            byte_limited.leases(),
            GenerationCaseMaterialLimits {
                maximum_total_source_bytes: PRIMARY_SOURCE.len() as u64 - 1,
                ..material_limits()
            },
            &CancellationToken::new(),
        ),
        Err(VerifiedGenerationCaseMaterialError::SourceByteLimitExceeded)
    ));
}

#[test]
fn requires_complete_one_to_one_collection_closure() {
    for missing in ["case", "contract", "lease"] {
        let fixture = Fixture::pair();
        let mut cases = fixture.cases.clone();
        let mut contracts = fixture.contracts.clone();
        let mut leases = fixture.leases();
        match missing {
            "case" => {
                cases.pop();
            }
            "contract" => {
                contracts.pop();
            }
            "lease" => {
                leases.pop();
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            VerifiedGenerationCaseMaterial::verify(
                fixture.suite.clone(),
                cases,
                contracts,
                leases,
                material_limits(),
                &CancellationToken::new(),
            ),
            Err(VerifiedGenerationCaseMaterialError::Relationship {
                relationship: VerifiedGenerationCaseMaterialRelationship::CollectionClosure,
                ..
            })
        ));
    }
}

#[test]
fn rejects_reordered_cases_contracts_and_source_leases() {
    let reordered_cases = Fixture::pair();
    let mut cases = reordered_cases.cases.clone();
    let mut contracts = reordered_cases.contracts.clone();
    cases.swap(0, 1);
    contracts.swap(0, 1);
    assert!(matches!(
        VerifiedGenerationCaseMaterial::verify(
            reordered_cases.suite.clone(),
            cases,
            contracts,
            reordered_cases.leases_in_order(&[1, 0]),
            material_limits(),
            &CancellationToken::new(),
        ),
        Err(VerifiedGenerationCaseMaterialError::Relationship {
            semantic_index: 0,
            relationship: VerifiedGenerationCaseMaterialRelationship::SuiteCase,
        })
    ));

    let reordered_contracts = Fixture::pair();
    let mut contracts = reordered_contracts.contracts.clone();
    contracts.swap(0, 1);
    assert!(matches!(
        VerifiedGenerationCaseMaterial::verify(
            reordered_contracts.suite.clone(),
            reordered_contracts.cases.clone(),
            contracts,
            reordered_contracts.leases(),
            material_limits(),
            &CancellationToken::new(),
        ),
        Err(VerifiedGenerationCaseMaterialError::Contract {
            semantic_index: 0,
            source: GenerationDeterministicCaseContractError::CaseManifestMismatch,
        })
    ));

    let reordered_leases = Fixture::pair();
    assert!(matches!(
        VerifiedGenerationCaseMaterial::verify(
            reordered_leases.suite.clone(),
            reordered_leases.cases.clone(),
            reordered_leases.contracts.clone(),
            reordered_leases.leases_in_order(&[1, 0]),
            material_limits(),
            &CancellationToken::new(),
        ),
        Err(VerifiedGenerationCaseMaterialError::Relationship {
            semantic_index: 0,
            relationship: VerifiedGenerationCaseMaterialRelationship::SourceLeaseCase,
        })
    ));
}

#[test]
fn cancellation_blocks_verification_revalidation_and_callback_results() {
    let cancelled_fixture = Fixture::primary();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        VerifiedGenerationCaseMaterial::verify(
            cancelled_fixture.suite.clone(),
            cancelled_fixture.cases.clone(),
            cancelled_fixture.contracts.clone(),
            cancelled_fixture.leases(),
            material_limits(),
            &cancelled,
        ),
        Err(VerifiedGenerationCaseMaterialError::Cancelled)
    ));

    let fixture = Fixture::primary();
    let material = fixture.verify().expect("material verifies");
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        material.revalidate(&cancelled),
        Err(VerifiedGenerationCaseMaterialError::Cancelled)
    ));
    assert!(matches!(
        material.with_case_source_bytes(1, &CancellationToken::new(), |_| 42),
        Err(VerifiedGenerationCaseMaterialError::CaseIndexOutOfRange)
    ));

    let callback_token = CancellationToken::new();
    let result = material.with_case_source_bytes(0, &callback_token, |bytes| {
        assert_eq!(bytes, PRIMARY_SOURCE);
        callback_token.cancel();
        42
    });
    assert!(matches!(
        result,
        Err(VerifiedGenerationCaseMaterialError::SourceLease {
            semantic_index: 0,
            ..
        })
    ));
}

#[test]
fn suite_case_identity_mismatch_is_distinct_from_collection_shape() {
    let fixture = Fixture::pair();
    let reversed_cases = {
        let mut cases = fixture.cases.clone();
        cases.reverse();
        cases
    };
    let mismatched_suite =
        GenerationSuiteManifestV1::new(digest("suite protocol"), &reversed_cases)
            .expect("mismatched suite is valid in its own order");
    assert!(matches!(
        VerifiedGenerationCaseMaterial::verify(
            mismatched_suite,
            fixture.cases.clone(),
            fixture.contracts.clone(),
            fixture.leases(),
            material_limits(),
            &CancellationToken::new(),
        ),
        Err(VerifiedGenerationCaseMaterialError::Relationship {
            semantic_index: 0,
            relationship: VerifiedGenerationCaseMaterialRelationship::SuiteCase,
        })
    ));
}

#[cfg(unix)]
#[test]
fn retained_source_drift_fails_material_revalidation() {
    use std::fs;

    let fixture = Fixture::primary();
    let material = fixture.verify().expect("material verifies");
    fs::write(&fixture.canonical_sources[0], b"Retain Acme 43 exactly.").expect("mutate source");
    assert!(matches!(
        material.revalidate(&CancellationToken::new()),
        Err(VerifiedGenerationCaseMaterialError::SourceLease {
            semantic_index: 0,
            ..
        })
    ));
}
