use super::*;

#[test]
fn every_terminal_stage_accepts_only_its_structural_combination() {
    let cases = [
        (
            GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
            StageState {
                ledger_status: GenerationQualificationPhaseStatusV1::Failed,
                repetition_completed: false,
                receipt_present: false,
                deterministic_status: None,
                join_present: false,
            },
        ),
        (
            GenerationRepeatabilityTerminalStageV1::DeterministicFailed,
            StageState {
                ledger_status: GenerationQualificationPhaseStatusV1::Passed,
                repetition_completed: true,
                receipt_present: true,
                deterministic_status: Some(CandidateDeterministicEvaluationStatusV1::Failed),
                join_present: false,
            },
        ),
        (
            GenerationRepeatabilityTerminalStageV1::JudgeFailed,
            StageState {
                ledger_status: GenerationQualificationPhaseStatusV1::Passed,
                repetition_completed: true,
                receipt_present: true,
                deterministic_status: Some(CandidateDeterministicEvaluationStatusV1::Passed),
                join_present: false,
            },
        ),
        (
            GenerationRepeatabilityTerminalStageV1::Passed,
            StageState {
                ledger_status: GenerationQualificationPhaseStatusV1::Passed,
                repetition_completed: true,
                receipt_present: true,
                deterministic_status: Some(CandidateDeterministicEvaluationStatusV1::Passed),
                join_present: true,
            },
        ),
    ];
    for (expected_index, (stage, state)) in cases.iter().copied().enumerate() {
        for (actual_index, (actual_stage, _)) in cases.iter().copied().enumerate() {
            assert_eq!(
                stage_state_matches(actual_stage, state),
                expected_index == actual_index
            );
        }
        let mut substituted = state;
        substituted.join_present = !substituted.join_present;
        assert!(!stage_state_matches(stage, substituted));
    }
    for (terminal_stage, mut evidence_state) in cases.iter().copied().skip(1) {
        evidence_state.ledger_status = GenerationQualificationPhaseStatusV1::Failed;
        assert!(stage_state_matches(terminal_stage, evidence_state));
        evidence_state.ledger_status = GenerationQualificationPhaseStatusV1::Skipped;
        assert!(!stage_state_matches(terminal_stage, evidence_state));
    }
    let mut invalid_candidate_failure = cases[0].1;
    invalid_candidate_failure.ledger_status = GenerationQualificationPhaseStatusV1::Passed;
    assert!(!stage_state_matches(
        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
        invalid_candidate_failure,
    ));
}

#[test]
fn target_side_substitution_never_cross_pairs_receipt_and_system() {
    let receipt_a = Digest::sha256(b"receipt a");
    let system_a = Digest::sha256(b"system a");
    let receipt_b = Digest::sha256(b"receipt b");
    let system_b = Digest::sha256(b"system b");
    let side_a = SideBinding {
        receipt: &receipt_a,
        system: &system_a,
    };
    let side_b = SideBinding {
        receipt: &receipt_b,
        system: &system_b,
    };
    assert!(target_side_matches(side_a, side_a, side_b));
    assert!(target_side_matches(side_b, side_a, side_b));
    assert!(!target_side_matches(
        SideBinding {
            receipt: &receipt_a,
            system: &system_b,
        },
        side_a,
        side_b,
    ));
    let substituted = Digest::sha256(b"substituted");
    assert!(!target_side_matches(
        SideBinding {
            receipt: &substituted,
            system: &system_a,
        },
        side_a,
        side_b,
    ));
}
