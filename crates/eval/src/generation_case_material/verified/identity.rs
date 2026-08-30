use rewrite_model::{GenerationCaseManifestV1, GenerationSuiteManifestV1};
use rewrite_types::Digest;

use super::{
    GENERATION_CASE_MATERIAL_SET_DIGEST_DOMAIN, MAX_GENERATION_CASE_MATERIAL_IDENTITY_BYTES,
    VerifiedGenerationCaseMaterialError,
};

pub(super) fn derive_case_material_set_digest(
    suite: &GenerationSuiteManifestV1,
    cases: &[GenerationCaseManifestV1],
) -> Result<Digest, VerifiedGenerationCaseMaterialError> {
    let per_case_bytes = 64_usize
        .checked_mul(4)
        .and_then(|value| value.checked_add(8))
        .ok_or(VerifiedGenerationCaseMaterialError::CanonicalIdentityTooLarge)?;
    let length = GENERATION_CASE_MATERIAL_SET_DIGEST_DOMAIN
        .len()
        .checked_add(64 + 8)
        .and_then(|value| {
            cases
                .len()
                .checked_mul(per_case_bytes)
                .and_then(|cases_length| value.checked_add(cases_length))
        })
        .filter(|length| *length <= MAX_GENERATION_CASE_MATERIAL_IDENTITY_BYTES)
        .ok_or(VerifiedGenerationCaseMaterialError::CanonicalIdentityTooLarge)?;
    let mut canonical = Vec::with_capacity(length);
    canonical.extend_from_slice(GENERATION_CASE_MATERIAL_SET_DIGEST_DOMAIN);
    append_digest(&mut canonical, suite.suite_manifest_id().digest());
    append_count(&mut canonical, cases.len())?;
    for case in cases {
        append_digest(&mut canonical, case.case_id().digest());
        append_digest(&mut canonical, case.source_artifact_id().digest());
        append_digest(&mut canonical, case.source_digest());
        append_u64(&mut canonical, case.source_byte_count());
        append_digest(&mut canonical, case.case_contract_digest());
    }
    debug_assert_eq!(canonical.len(), length);
    Ok(Digest::sha256(&canonical))
}

fn append_count(
    output: &mut Vec<u8>,
    count: usize,
) -> Result<(), VerifiedGenerationCaseMaterialError> {
    let count = u64::try_from(count)
        .map_err(|_| VerifiedGenerationCaseMaterialError::CanonicalIdentityTooLarge)?;
    append_u64(output, count);
    Ok(())
}

fn append_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn append_digest(output: &mut Vec<u8>, value: &Digest) {
    output.extend_from_slice(value.as_str().as_bytes());
}
