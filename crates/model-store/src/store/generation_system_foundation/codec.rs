use rewrite_model::{
    GenerationSystemRecordV1, GenerationSystemRecordV1Relations,
    MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES, MAX_GENERATION_SYSTEM_JSON_BYTES,
};
use rusqlite::Connection;

use crate::{StoreError, StoreResult};

use super::GenerationSystemFoundationV1Input;

pub(super) struct EncodedInput {
    pub(super) evidence: Vec<u8>,
    pub(super) system: Vec<u8>,
}

pub(super) fn canonical_input(
    input: GenerationSystemFoundationV1Input<'_>,
) -> StoreResult<EncodedInput> {
    let evidence = serde_json::to_vec(input.relations.effective_package_evidence_v2)?;
    require_blob_bound(evidence.len(), MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES)?;
    let decoded_evidence = rewrite_model::EffectivePackageEvidenceV2::from_json_bytes(
        &evidence,
        input.relations.model_artifact_set,
        input.relations.runtime_build,
        input.relations.effective_runtime_state,
    )
    .map_err(StoreError::InvalidEffectivePackageV2)?;
    if decoded_evidence != *input.relations.effective_package_evidence_v2
        || serde_json::to_vec(&decoded_evidence)? != evidence
    {
        return Err(StoreError::CorruptRecord);
    }
    let system = serde_json::to_vec(input.generation_system)?;
    require_blob_bound(system.len(), MAX_GENERATION_SYSTEM_JSON_BYTES)?;
    let decoded_system = GenerationSystemRecordV1::from_json_bytes(&system, input.relations)
        .map_err(StoreError::InvalidGenerationSystem)?;
    if decoded_system != *input.generation_system || serde_json::to_vec(&decoded_system)? != system
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(EncodedInput { evidence, system })
}

pub(super) fn require_exact_dependencies(
    connection: &Connection,
    relations: GenerationSystemRecordV1Relations<'_>,
) -> StoreResult<()> {
    use crate::store::evidence::read::{load_artifact_set, load_runtime_build, load_runtime_state};
    use crate::store::package_contracts::read::{load_model_package, load_runtime_package};

    let runtime_package = load_runtime_package(
        connection,
        relations
            .runtime_package_manifest
            .runtime_package_manifest_id()
            .digest()
            .as_str(),
    )?
    .ok_or(StoreError::MissingRecord)?;
    let runtime_build = load_runtime_build(
        connection,
        relations.runtime_build.runtime_build_id().digest().as_str(),
    )?
    .ok_or(StoreError::MissingRecord)?;
    let runtime_state = load_runtime_state(
        connection,
        relations
            .effective_runtime_state
            .effective_runtime_state_id()
            .digest()
            .as_str(),
    )?
    .ok_or(StoreError::MissingRecord)?;
    let model_set = load_artifact_set(
        connection,
        relations
            .model_artifact_set
            .artifact_set_id()
            .digest()
            .as_str(),
    )?
    .ok_or(StoreError::MissingRecord)?;
    let model_package = load_model_package(
        connection,
        relations
            .model_package_manifest
            .model_package_manifest_id()
            .digest()
            .as_str(),
    )?
    .ok_or(StoreError::MissingRecord)?;
    if runtime_package != *relations.runtime_package_manifest
        || runtime_build != *relations.runtime_build
        || runtime_state != *relations.effective_runtime_state
        || model_set != *relations.model_artifact_set
        || model_package != *relations.model_package_manifest
    {
        return Err(StoreError::ImmutableConflict);
    }
    Ok(())
}

fn require_blob_bound(length: usize, maximum: usize) -> StoreResult<()> {
    if length == 0 || length > maximum {
        Err(StoreError::RecordTooLarge)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_bound_rejects_empty_and_oversized_input() {
        assert!(matches!(
            require_blob_bound(0, 1),
            Err(StoreError::RecordTooLarge)
        ));
        assert!(matches!(
            require_blob_bound(2, 1),
            Err(StoreError::RecordTooLarge)
        ));
    }
}
