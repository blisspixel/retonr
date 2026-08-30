#[cfg(test)]
use rewrite_model::{ExpectedOutput, ReferenceJudgment};
pub use rewrite_model::{
    GENERATION_DETERMINISTIC_CASE_CONTRACT_SCHEMA_VERSION,
    GenerationDeterministicCaseContractError, GenerationDeterministicCaseContractV1,
    GenerationDeterministicCaseContractV1Input,
    MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES,
    MAX_GENERATION_DETERMINISTIC_CASE_PROTECTED_TERMS,
    MAX_GENERATION_DETERMINISTIC_CASE_RUBRIC_CLAUSES,
};

mod verified;
#[cfg(test)]
pub(crate) use verified::tests::support as verified_material_test_support;
pub use verified::{
    GENERATION_CASE_MATERIAL_SET_DIGEST_DOMAIN, GenerationCaseMaterialLimits,
    MAX_GENERATION_CASE_MATERIAL_CASES, MAX_GENERATION_CASE_MATERIAL_IDENTITY_BYTES,
    MAX_GENERATION_CASE_MATERIAL_TOTAL_SOURCE_BYTES, VerifiedGenerationCaseMaterial,
    VerifiedGenerationCaseMaterialError, VerifiedGenerationCaseMaterialRelationship,
    VerifiedGenerationCaseMaterialTraversalError,
};

#[cfg(test)]
mod tests;
