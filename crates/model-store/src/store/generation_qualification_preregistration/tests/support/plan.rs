use rewrite_model::{
    ArtifactId, ExpectedOutput, GenerationDeterministicCaseContractV1,
    GenerationDeterministicCaseContractV1Input, ReferenceJudgment,
};
use rewrite_types::{Digest, ReasonCode, RewriteStatus};

pub(super) fn deterministic_case_contract() -> GenerationDeterministicCaseContractV1 {
    GenerationDeterministicCaseContractV1::new(GenerationDeterministicCaseContractV1Input {
        case_key: "case".to_owned(),
        source_artifact_id: ArtifactId::from_digest(digest("case source")),
        source_digest: digest("case source"),
        source_byte_count: 11,
        language_digest: digest("language"),
        mode_digest: digest("mode"),
        format_digest: digest("format"),
        evaluation_category: "fidelity".to_owned(),
        protected_terms: vec!["case".to_owned()],
        reference_judgment: ReferenceJudgment::Unacceptable,
        expected_status: RewriteStatus::Abstained,
        expected_reason: Some(ReasonCode::ProtectedValueChanged),
        expected_output: ExpectedOutput::Source,
        rubric_clause_ids: vec!["fidelity".to_owned()],
    })
    .expect("deterministic case contract")
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}
