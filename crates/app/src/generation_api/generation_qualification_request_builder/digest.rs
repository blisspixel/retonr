use rewrite_grounded::{GROUNDED_POLICY_SCHEMA_VERSION, GROUNDED_PROMPT_CONTENT_BOUNDARY_V1};
use rewrite_inference::candidate_output_contract;
use rewrite_model::{GenerationCaseManifestV1, GenerationCaseRequestProfileV1};
use rewrite_types::{Digest, RewriteUnitId};

use super::{
    GENERATION_QUALIFICATION_PROMPT_TEMPLATE_V1,
    GENERATION_QUALIFICATION_REQUEST_BUILDER_SCHEMA_VERSION,
    GenerationQualificationRequestBuildError, GenerationQualificationRequestBuilderBindingsV1,
};

/// Domain for the typed whole-document planner contract.
pub const GENERATION_QUALIFICATION_PLANNER_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-qualification-request-planner:v1\0";
/// Domain for the exact grounded prompt-construction contract.
pub const GENERATION_QUALIFICATION_PROMPT_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-qualification-grounded-prompt:v1\0";
/// Domain for the provider-neutral fixed request policy.
pub const GENERATION_QUALIFICATION_REQUEST_POLICY_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-qualification-request-policy:v1\0";
/// Domain for the complete grounded generation strategy composition.
pub const GENERATION_QUALIFICATION_STRATEGY_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-qualification-grounded-strategy:v1\0";
/// Domain for one exact rendered grounded request and its case binding.
pub const GENERATION_QUALIFICATION_GROUNDED_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-qualification-grounded-request:v1\0";

const PROMPT_FIELDS: &[&[u8]] = &[
    b"schema_version",
    b"content_boundary",
    b"masked_source",
    b"protected_sentinels",
    b"rewrite_mode",
    b"style_status",
    b"style_context",
    b"required_candidate_count",
];

pub(super) fn builder_bindings(
    profile: &GenerationCaseRequestProfileV1,
    validator_equality_binding: &Digest,
    adapter_equality_binding: &Digest,
) -> Result<GenerationQualificationRequestBuilderBindingsV1, GenerationQualificationRequestBuildError>
{
    let planner_digest = planner_digest(profile)?;
    let prompt_digest = prompt_digest();
    let output_schema_digest = candidate_output_contract().schema_digest;
    let request_policy_digest = request_policy_digest(&output_schema_digest);
    let strategy_digest = strategy_digest(
        &planner_digest,
        validator_equality_binding,
        adapter_equality_binding,
        &prompt_digest,
        &output_schema_digest,
        &request_policy_digest,
    );
    Ok(GenerationQualificationRequestBuilderBindingsV1 {
        strategy_digest,
        planner_digest,
        prompt_digest,
        output_schema_digest,
        request_policy_digest,
        language_digest: profile.language_digest(),
        mode_digest: profile.rewrite_mode_digest(),
        format_digest: profile.format_digest(),
    })
}

fn planner_digest(
    profile: &GenerationCaseRequestProfileV1,
) -> Result<Digest, GenerationQualificationRequestBuildError> {
    let profile = profile
        .to_canonical_json_bytes()
        .map_err(|_| GenerationQualificationRequestBuildError::ProfileMismatch)?;
    let mut material = GENERATION_QUALIFICATION_PLANNER_DIGEST_DOMAIN.to_vec();
    append_u32(
        &mut material,
        GENERATION_QUALIFICATION_REQUEST_BUILDER_SCHEMA_VERSION,
    );
    append_bytes(&mut material, &profile)?;
    Ok(Digest::sha256(&material))
}

fn prompt_digest() -> Digest {
    let mut material = GENERATION_QUALIFICATION_PROMPT_DIGEST_DOMAIN.to_vec();
    append_u32(
        &mut material,
        GENERATION_QUALIFICATION_REQUEST_BUILDER_SCHEMA_VERSION,
    );
    append_u32(&mut material, GROUNDED_POLICY_SCHEMA_VERSION);
    append_bytes_infallible(
        &mut material,
        GENERATION_QUALIFICATION_PROMPT_TEMPLATE_V1.as_bytes(),
    );
    material.push(b'\n');
    append_bytes_infallible(
        &mut material,
        GROUNDED_PROMPT_CONTENT_BOUNDARY_V1.as_bytes(),
    );
    append_u32(
        &mut material,
        u32::try_from(PROMPT_FIELDS.len()).expect("fixed prompt field count fits u32"),
    );
    for field in PROMPT_FIELDS {
        append_bytes_infallible(&mut material, field);
    }
    material.push(0); // compact JSON
    material.push(0); // no trailing newline
    Digest::sha256(&material)
}

fn request_policy_digest(output_schema_digest: &Digest) -> Digest {
    let mut material = GENERATION_QUALIFICATION_REQUEST_POLICY_DIGEST_DOMAIN.to_vec();
    append_u32(
        &mut material,
        GENERATION_QUALIFICATION_REQUEST_BUILDER_SCHEMA_VERSION,
    );
    material.push(1); // exact candidate count
    material.extend_from_slice(&0.0_f32.to_bits().to_be_bytes());
    material.extend_from_slice(&1.0_f32.to_bits().to_be_bytes());
    material.push(0); // seed comes from the selected planned attempt
    material.push(0); // reasoning disabled
    material.push(0); // every limit comes from the exact operation policy
    append_digest(&mut material, output_schema_digest);
    Digest::sha256(&material)
}

fn strategy_digest(
    planner_digest: &Digest,
    validator_equality_binding: &Digest,
    adapter_equality_binding: &Digest,
    prompt_digest: &Digest,
    output_schema_digest: &Digest,
    request_policy_digest: &Digest,
) -> Digest {
    let mut material = GENERATION_QUALIFICATION_STRATEGY_DIGEST_DOMAIN.to_vec();
    append_u32(
        &mut material,
        GENERATION_QUALIFICATION_REQUEST_BUILDER_SCHEMA_VERSION,
    );
    for digest in [
        planner_digest,
        validator_equality_binding,
        adapter_equality_binding,
        prompt_digest,
        output_schema_digest,
        request_policy_digest,
    ] {
        append_digest(&mut material, digest);
    }
    Digest::sha256(&material)
}

pub(super) fn grounded_request_digest(
    case: &GenerationCaseManifestV1,
    case_contract_digest: &Digest,
    prompt_digest: &Digest,
    unit_id: &RewriteUnitId,
    rendered_input: &str,
) -> Result<Digest, GenerationQualificationRequestBuildError> {
    let mut material = GENERATION_QUALIFICATION_GROUNDED_REQUEST_DIGEST_DOMAIN.to_vec();
    append_u32(
        &mut material,
        GENERATION_QUALIFICATION_REQUEST_BUILDER_SCHEMA_VERSION,
    );
    append_digest(&mut material, case.case_id().digest());
    append_digest(&mut material, case_contract_digest);
    append_digest(&mut material, prompt_digest);
    append_bytes(&mut material, unit_id.as_str().as_bytes())?;
    append_bytes(&mut material, rendered_input.as_bytes())?;
    Ok(Digest::sha256(&material))
}

fn append_digest(output: &mut Vec<u8>, digest: &Digest) {
    output.extend_from_slice(digest.as_str().as_bytes());
}

fn append_bytes(
    output: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), GenerationQualificationRequestBuildError> {
    let length = u64::try_from(value.len())
        .map_err(|_| GenerationQualificationRequestBuildError::RequestMismatch)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

fn append_bytes_infallible(output: &mut Vec<u8>, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("fixed contract byte length fits u64");
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
}

fn append_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}
