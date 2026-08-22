use std::{cell::RefCell, rc::Rc};

use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{NativeLoadObservation, RuntimePackageManifest, RuntimePackageManifestId};
use rewrite_ollama::{
    OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES, OllamaCloudDisableFeaturePolicy,
    OllamaCloudDisableVersionStatus, OllamaModelBinding, OllamaObservedSessionError, OllamaVersion,
};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessLease, ExpectedExternalNativeComponent,
    NativeLoadObservationRequest, NativeManagedLinuxProcessLease, RetainedNativePackageMember,
};
use rewrite_types::{CancellationToken, Digest};

use crate::{
    LocalOllamaBoundPreflightError, LocalOllamaBoundPreflightPlan, LocalOllamaModelBindingEvidence,
    local_ollama_bound_preflight::ConnectionObservationSequence,
    local_ollama_model_binding::{
        LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, validate_local_ollama_model_binding_evidence,
    },
};

use super::super::{
    LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightLimits,
    validation::validate_process_binding,
};
use super::LocalOllamaManagedGenerationError;

pub(super) struct ManagedSessionObserver {
    pub(super) process: NativeManagedLinuxProcessLease,
    pub(super) connections: ConnectionObservationSequence,
}

pub(super) fn validate_generation_admission(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
) -> Result<(), LocalOllamaManagedGenerationError> {
    let runtime_version = plan
        .preflight
        .expected_runtime_version
        .parse::<OllamaVersion>()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    if OllamaCloudDisableFeaturePolicy::assess(
        runtime_version,
        &package.runtime_package_manifest_id(),
    ) != OllamaCloudDisableVersionStatus::Reviewed
    {
        return Err(LocalOllamaManagedGenerationError::RuntimeNotAdmitted);
    }
    Ok(())
}

pub(super) fn validate_generation_binding(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
    request: &StructuredCompletionRequest,
) -> Result<(), LocalOllamaManagedPreflightError> {
    let plan_digest = serde_json::to_vec(&plan.preflight)
        .map(|bytes| Digest::sha256(&bytes))
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let Some(planned_model) = plan.preflight.models.first() else {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    };
    let valid = package.reported_version() == LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION
        && plan.preflight.models.len() == 1
        && validate_local_ollama_model_binding_evidence(static_model)
        && static_model.preflight_plan_digest == plan_digest
        && model.reference() == planned_model.reference
        && model.inventory_digest() == &planned_model.inventory_digest
        && static_model.runtime_reference_digest == Digest::sha256(model.reference().as_bytes())
        && static_model.inventory_digest == *model.inventory_digest()
        && static_model.model_artifact_id == *model.artifact_id()
        && model.artifact_digest() == static_model.model_artifact_id.digest()
        && request.artifact_id == *model.artifact_id()
        && request.artifact_digest == *model.artifact_digest()
        && u64::try_from(request.input.len()).unwrap_or(u64::MAX)
            <= u64::from(OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES)
        && request.validate().is_ok();
    if !valid {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn reobserve_process(
    observer: &Rc<RefCell<ManagedSessionObserver>>,
    package: &RuntimePackageManifest,
    cancellation: &CancellationToken,
) -> Result<AttachedProcessEvidence, LocalOllamaManagedPreflightError> {
    let evidence = observer
        .try_borrow_mut()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?
        .process
        .reobserve(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Witness)?;
    validate_process_binding(&evidence, package)?;
    Ok(evidence)
}

pub(super) fn observe_native_load(
    observer: &Rc<RefCell<ManagedSessionObserver>>,
    package: &RuntimePackageManifest,
    package_id: &RuntimePackageManifestId,
    retained_members: &[RetainedNativePackageMember],
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    cancellation: &CancellationToken,
) -> Result<NativeLoadObservation, LocalOllamaManagedPreflightError> {
    observer
        .try_borrow_mut()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?
        .process
        .observe_native_load(
            &NativeLoadObservationRequest {
                package,
                expected_package_id: package_id,
                retained_package_members: retained_members,
                expected_external_components: external_components,
                limits: limits.native_load,
            },
            cancellation,
        )
        .map_err(LocalOllamaManagedPreflightError::NativeLoad)
}

pub(super) fn map_session_error(
    error: OllamaObservedSessionError<LocalOllamaBoundPreflightError>,
) -> LocalOllamaManagedGenerationError {
    match error {
        OllamaObservedSessionError::Session(error) => {
            LocalOllamaManagedGenerationError::Session(error)
        }
        OllamaObservedSessionError::Observation(error) => {
            LocalOllamaManagedPreflightError::BoundObservation(error).into()
        }
    }
}

#[cfg(test)]
mod tests {
    use rewrite_inference::{
        ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, SamplingParameters,
        StructuredCompletionRequest, candidate_output_contract,
    };
    use rewrite_ollama::OllamaModelBinding;
    use rewrite_types::Digest;

    use super::{validate_generation_admission, validate_generation_binding};
    use crate::{
        LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION, LocalOllamaBoundPreflightPlan,
        local_ollama_managed_preflight::test_support::package_for_version,
        local_ollama_model_binding::{
            LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, tests::exact_binding_fixture,
        },
    };

    fn request(model: &OllamaModelBinding) -> StructuredCompletionRequest {
        StructuredCompletionRequest {
            schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
            artifact_id: model.artifact_id().clone(),
            artifact_digest: model.artifact_digest().clone(),
            input: "bounded fixture".to_owned(),
            output: candidate_output_contract(),
            source_byte_count: 15,
            source_byte_limit: 1024,
            input_byte_limit: 2048,
            context_token_limit: 2048,
            output_token_limit: 256,
            output_byte_limit: 4096,
            sampling: SamplingParameters {
                temperature: 0.0,
                top_p: 1.0,
                seed: Some(7),
            },
            reasoning: ReasoningPolicy::Disabled,
        }
    }

    #[test]
    fn exact_static_model_artifact_and_distinct_inventory_bind() {
        let package = package_for_version(LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION);
        let (preflight, evidence, model) = exact_binding_fixture();
        let plan = LocalOllamaBoundPreflightPlan {
            schema_version: LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION,
            preflight,
            maximum_entrypoint_bytes: 1024,
            maximum_session_body_bytes: 4 * 1024 * 1024,
            expected_entrypoint_digest: Some(package.entrypoint().artifact_id().digest().clone()),
        };
        let request = request(&model);

        validate_generation_binding(&package, &plan, &evidence, &model, &request)
            .expect("exact distinct identities bind");
        assert_ne!(model.artifact_digest(), model.inventory_digest());

        let wrong_inventory = Digest::sha256(b"wrong inventory");
        let wrong_model = OllamaModelBinding::new_with_inventory(
            model.reference(),
            model.artifact_id().clone(),
            model.artifact_digest().clone(),
            wrong_inventory,
        )
        .expect("structurally valid wrong binding");
        assert!(
            validate_generation_binding(&package, &plan, &evidence, &wrong_model, &request)
                .is_err()
        );
        assert!(matches!(
            validate_generation_admission(&package, &plan),
            Err(super::LocalOllamaManagedGenerationError::RuntimeNotAdmitted)
        ));

        let mut oversized = request;
        oversized.input = "x".repeat(
            usize::try_from(rewrite_ollama::OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES)
                .expect("input limit")
                + 1,
        );
        oversized.input_byte_limit = u64::try_from(oversized.input.len()).expect("input length");
        assert!(oversized.validate().is_ok());
        assert!(
            validate_generation_binding(&package, &plan, &evidence, &model, &oversized).is_err()
        );
    }
}
