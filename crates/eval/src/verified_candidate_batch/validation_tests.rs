use super::{
    CandidateRelationshipChecks, FixedRelationshipChecks, VerifiedCandidateBatchError,
    require_relationship,
};
use crate::VerifiedCandidateBatchRelationship as Relationship;

fn assert_relationship(result: &Result<(), VerifiedCandidateBatchError>, expected: Relationship) {
    assert!(matches!(
        result,
        Err(VerifiedCandidateBatchError::Relationship(observed)) if *observed == expected
    ));
}

fn passing_fixed_checks() -> FixedRelationshipChecks {
    FixedRelationshipChecks {
        planned_attempt: true,
        generation_system: true,
        generation_request: true,
        structured_request: true,
        precursor: true,
        managed_evidence: true,
        cleanup: true,
        effective_package: true,
        response: true,
        response_usage: true,
        attempt_record: true,
        receipt_compilation: true,
    }
}

#[test]
fn fixed_join_rejects_each_independent_substitution() {
    macro_rules! reject {
        ($field:ident, $relationship:ident) => {{
            let mut checks = passing_fixed_checks();
            checks.$field = false;
            assert_relationship(&checks.validate(), Relationship::$relationship);
        }};
    }

    assert!(passing_fixed_checks().validate().is_ok());
    reject!(planned_attempt, PlannedAttempt);
    reject!(generation_system, GenerationSystem);
    reject!(generation_request, GenerationRequest);
    reject!(structured_request, StructuredRequest);
    reject!(precursor, Precursor);
    reject!(managed_evidence, ManagedEvidence);
    reject!(cleanup, Cleanup);
    reject!(effective_package, EffectivePackage);
    reject!(response, Response);
    reject!(response_usage, ResponseUsage);
    reject!(attempt_record, AttemptRecord);
    reject!(receipt_compilation, ReceiptCompilation);
}

#[test]
fn candidate_join_rejects_ordinal_bytes_and_artifact_substitution() {
    assert_relationship(
        &require_relationship(false, Relationship::CandidateCount),
        Relationship::CandidateCount,
    );
    for (checks, relationship) in [
        (
            CandidateRelationshipChecks {
                ordinal: false,
                bytes: true,
                artifact: true,
            },
            Relationship::CandidateOrdinal,
        ),
        (
            CandidateRelationshipChecks {
                ordinal: true,
                bytes: false,
                artifact: true,
            },
            Relationship::CandidateBytes,
        ),
        (
            CandidateRelationshipChecks {
                ordinal: true,
                bytes: true,
                artifact: false,
            },
            Relationship::CandidateArtifact,
        ),
    ] {
        assert_relationship(&checks.validate(), relationship);
    }
}
