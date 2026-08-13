use super::{
    CandidateAssessment, EvidenceContractError, GateEvidence, GateResult, GateStatus,
    InvariantEvidenceSummary, MAX_GATES_PER_CANDIDATE, MAX_SEMANTIC_EVIDENCE_ITEMS,
    SemanticAssessment, SemanticEvidence, SemanticEvidenceCode,
};
use crate::{CandidateId, Digest, DocumentId, RewriteUnitId};

#[test]
fn validated_shapes_reject_malformed_deserialization() {
    let fail = GateResult::fail("literal", "missing", "protected value missing");
    assert_eq!(fail.validate(), Ok(()));
    let encoded = serde_json::to_string(&fail).expect("gate serializes");
    assert_eq!(
        serde_json::from_str::<GateResult>(&encoded).expect("gate deserializes"),
        fail
    );
    assert!(
        serde_json::from_str::<GateResult>(&encoded.replace("\"1\"", "\"bad version!\"")).is_err()
    );
    let oversized = "x".repeat(super::MAX_GATE_EVIDENCE_MESSAGE_BYTES + 1);
    assert!(
        serde_json::from_value::<GateEvidence>(serde_json::json!({
            "code": "bounded",
            "message": oversized
        }))
        .is_err()
    );
    assert_eq!(
        InvariantEvidenceSummary {
            declared_terms: 1,
            urls: 0,
            emails: 0,
            numbers: 0,
            total: 0,
        }
        .validate(),
        Err(EvidenceContractError::InvalidInvariantSummary)
    );
}

#[test]
fn semantic_codes_are_closed_and_confidence_is_bounded() {
    let finding = SemanticEvidence::new(SemanticEvidenceCode::UnsupportedMode, None);
    let assessment = SemanticAssessment {
        status: GateStatus::Uncertain,
        confidence: None,
        evidence: vec![finding.clone()],
    };
    assert_eq!(assessment.validate(), Ok(()));
    assert_eq!(assessment.evidence[0].code.as_str(), "unsupported_mode");
    assert!(
        serde_json::from_str::<SemanticEvidence>(r#"{"code":"private_diagnosis","details":null}"#)
            .is_err()
    );
    for confidence in [f32::NAN, 1.01] {
        assert_eq!(
            SemanticAssessment {
                status: GateStatus::Pass,
                confidence: Some(confidence),
                evidence: Vec::new(),
            }
            .validate(),
            Err(EvidenceContractError::InvalidConfidence)
        );
    }
    assert_eq!(
        SemanticAssessment {
            status: GateStatus::Pass,
            confidence: Some(1.0),
            evidence: Vec::new(),
        }
        .validate(),
        Err(EvidenceContractError::InconsistentSemanticFinding)
    );
    assert_eq!(
        SemanticAssessment {
            status: GateStatus::Uncertain,
            confidence: None,
            evidence: vec![finding.clone(); MAX_SEMANTIC_EVIDENCE_ITEMS],
        }
        .validate(),
        Ok(())
    );
    assert_eq!(
        SemanticAssessment {
            status: GateStatus::Uncertain,
            confidence: None,
            evidence: vec![finding; MAX_SEMANTIC_EVIDENCE_ITEMS + 1],
        }
        .validate(),
        Err(EvidenceContractError::TooManyFindings)
    );
}

#[test]
fn passing_assessment_rejects_negative_or_uncertain_finding_categories() {
    for code in [
        SemanticEvidenceCode::UnsupportedMode,
        SemanticEvidenceCode::LiteralTokensChanged,
        SemanticEvidenceCode::ClaimComparisonUncertain,
    ] {
        assert_eq!(
            SemanticAssessment {
                status: GateStatus::Pass,
                confidence: Some(1.0),
                evidence: vec![SemanticEvidence::new(code, None)],
            }
            .validate(),
            Err(EvidenceContractError::InconsistentSemanticFinding)
        );
    }
}

#[test]
fn candidate_deserialization_validates_nested_gates_and_bounds() {
    let unit = RewriteUnitId::new(&DocumentId::from_digest(&Digest::sha256(b"d")), 0);
    let assessment = CandidateAssessment {
        candidate_id: CandidateId::new(&unit, 0),
        unit_id: unit,
        eligible: true,
        gates: vec![GateResult::pass("shape")],
    };
    let encoded = serde_json::to_string(&assessment).expect("assessment serializes");
    assert_eq!(
        serde_json::from_str::<CandidateAssessment>(&encoded).expect("assessment deserializes"),
        assessment
    );
    let oversized = CandidateAssessment {
        gates: vec![GateResult::pass("shape"); MAX_GATES_PER_CANDIDATE + 1],
        ..assessment.clone()
    };
    assert_eq!(
        oversized.validate(),
        Err(EvidenceContractError::TooManyGates)
    );

    let other_unit = RewriteUnitId::new(&DocumentId::from_digest(&Digest::sha256(b"other")), 0);
    let mismatched = CandidateAssessment {
        unit_id: other_unit,
        ..assessment.clone()
    };
    assert_eq!(
        mismatched.validate(),
        Err(EvidenceContractError::InvalidCandidateScope)
    );
    let contradictory = CandidateAssessment {
        eligible: true,
        gates: vec![GateResult::fail("shape", "changed", "shape changed")],
        ..assessment
    };
    assert_eq!(
        contradictory.validate(),
        Err(EvidenceContractError::InconsistentEligibility)
    );
    assert!(
        serde_json::from_value::<CandidateAssessment>(serde_json::json!({
            "candidate_id": CandidateId::new(&contradictory.unit_id, 0),
            "unit_id": contradictory.unit_id,
            "eligible": true,
            "gates": vec![GateResult::pass("shape"); MAX_GATES_PER_CANDIDATE + 1]
        }))
        .is_err()
    );
}

#[test]
fn old_gate_evidence_without_details_remains_readable() {
    let decoded: GateEvidence =
        serde_json::from_str(r#"{"code":"legacy","message":"bounded legacy evidence"}"#)
            .expect("legacy evidence deserializes");
    assert!(decoded.details.is_none());
    assert_eq!(
        GateEvidence::new("Not Valid", "message", None),
        Err(EvidenceContractError::InvalidCode)
    );
}
