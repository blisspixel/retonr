use rewrite_model::{
    CandidateJudgeEvidenceClassV1, CandidateJudgeOrderPolicyV1, ManagedLocalJudgeEvidenceClassV1,
    ManagedLocalJudgeReceiptSuccessStatusV1,
};

pub(super) fn digest_text(value: &rewrite_types::Digest) -> &str {
    value.as_str()
}

pub(super) fn order_policy(value: CandidateJudgeOrderPolicyV1) -> &'static str {
    match value {
        CandidateJudgeOrderPolicyV1::BothOrders => "both_orders",
    }
}

pub(super) fn receipt_success(value: ManagedLocalJudgeReceiptSuccessStatusV1) -> &'static str {
    match value {
        ManagedLocalJudgeReceiptSuccessStatusV1::Succeeded => "succeeded",
    }
}

pub(super) fn receipt_evidence(value: ManagedLocalJudgeEvidenceClassV1) -> &'static str {
    match value {
        ManagedLocalJudgeEvidenceClassV1::ManagedLocalJudgeTriage => "managed_local_judge_triage",
    }
}

pub(super) fn join_evidence(value: CandidateJudgeEvidenceClassV1) -> &'static str {
    match value {
        CandidateJudgeEvidenceClassV1::ManagedLocalJudgeTriage => "managed_local_judge_triage",
    }
}
