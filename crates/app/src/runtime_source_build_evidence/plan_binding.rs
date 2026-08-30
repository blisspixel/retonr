use rewrite_model::ArtifactSetId;
use rewrite_ollama_package::{RuntimeSourceBuildPlan, VerifiedRuntimeSourceBuildInputs};
use rewrite_types::Digest;
use serde::Deserialize;
use serde_json::json;

use super::RuntimeSourceBuildEvidenceBundleError;
use crate::ExecutableRuntimeSourceBuildBundleLease;

pub(super) const MAXIMUM_PLAN_BINDING_BYTES: usize = 1_024;
const PLAN_BINDING_SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanBindingWire {
    build_plan_digest: Digest,
    retained_program_closure_id: Digest,
    schema_version: u32,
    source_build_inputs_id: ArtifactSetId,
    source_manifest_digest: Digest,
}

pub(super) fn canonical_bytes(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
) -> Result<Option<Vec<u8>>, RuntimeSourceBuildEvidenceBundleError> {
    let plan = bundle.plan();
    let Some(closure_id) = plan.retained_program_closure_id() else {
        #[cfg(all(test, target_os = "linux"))]
        return Ok(None);
        #[cfg(not(all(test, target_os = "linux")))]
        return Err(RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding);
    };
    serde_json::to_vec(&json!({
        "build_plan_digest": plan.plan_digest(),
        "retained_program_closure_id": closure_id,
        "schema_version": PLAN_BINDING_SCHEMA_VERSION,
        "source_build_inputs_id": plan.source_inputs_id(),
        "source_manifest_digest": plan.source_manifest_digest()
    }))
    .map(Some)
    .map_err(|_| RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding)
}

pub(super) fn parse(
    bytes: &[u8],
    inputs: &VerifiedRuntimeSourceBuildInputs,
) -> Result<RuntimeSourceBuildPlan, RuntimeSourceBuildEvidenceBundleError> {
    let wire = parse_wire(bytes)?;
    if wire.schema_version != PLAN_BINDING_SCHEMA_VERSION
        || wire.source_build_inputs_id != inputs.manifest().artifact_set().artifact_set_id()
        || wire.source_manifest_digest != *inputs.manifest().manifest_digest()
    {
        return Err(RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding);
    }
    let plan = RuntimeSourceBuildPlan::for_verified_retained_program_closure(
        inputs,
        wire.retained_program_closure_id,
    )?;
    if wire.build_plan_digest != *plan.plan_digest() {
        return Err(RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding);
    }
    Ok(plan)
}

fn parse_wire(bytes: &[u8]) -> Result<PlanBindingWire, RuntimeSourceBuildEvidenceBundleError> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_PLAN_BINDING_BYTES {
        return Err(RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding);
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding)?;
    if serde_json::to_vec(&value)
        .map_err(|_| RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding)?
        != bytes
    {
        return Err(RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding);
    }
    serde_json::from_value(value)
        .map_err(|_| RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: &[u8]) -> Digest {
        Digest::sha256(seed)
    }

    fn valid_bytes() -> Vec<u8> {
        serde_json::to_vec(&json!({
            "build_plan_digest": digest(b"plan"),
            "retained_program_closure_id": digest(b"closure"),
            "schema_version": 1,
            "source_build_inputs_id": ArtifactSetId::from_digest(digest(b"inputs")),
            "source_manifest_digest": digest(b"manifest")
        }))
        .expect("serialize fixed valid plan-binding fixture")
    }

    #[test]
    fn wire_requires_exact_canonical_bounded_schema() {
        assert!(parse_wire(&valid_bytes()).is_ok());
        for invalid in [
            Vec::new(),
            b"{}".to_vec(),
            b"{\"schema_version\":1,\"schema_version\":1}".to_vec(),
            [b" ".as_slice(), valid_bytes().as_slice()].concat(),
            vec![b'x'; MAXIMUM_PLAN_BINDING_BYTES + 1],
        ] {
            assert!(matches!(
                parse_wire(&invalid),
                Err(RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding)
            ));
        }
    }

    #[test]
    fn wire_rejects_unknown_fields() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&valid_bytes()).expect("parse fixed valid plan-binding fixture");
        value["extra"] = json!(true);
        let bytes = serde_json::to_vec(&value).expect("serialize modified plan-binding fixture");
        assert!(matches!(
            parse_wire(&bytes),
            Err(RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding)
        ));
    }
}
