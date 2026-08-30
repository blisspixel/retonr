use std::{ffi::OsStr, path::Path};

use crate::ModelLicenseControlError;
use rewrite_runtime_attestor::MANAGED_OLLAMA_MODEL_ROOT;
use rewrite_runtime_isolation::IsolationError;

use super::{
    MANAGED_OLLAMA_V0_32_15_ENDPOINT, ManagedOllamaCloseError, ManagedOllamaLaunchError,
    ManagedOllamaModelAuthorityError, combine_close_results, managed_ollama_v0_32_15_launch_spec,
    mandatory_final_cancellation, validate_authorized_launch,
};
use rewrite_types::CancellationToken;

#[test]
fn closed_launch_has_exact_command_endpoint_and_environment() {
    let specification = managed_ollama_v0_32_15_launch_spec();
    assert_eq!(specification.executable(), Path::new("/bin/ollama"));
    assert_eq!(specification.arguments(), [OsStr::new("serve")]);
    assert_eq!(specification.environment_count(), 16);
    assert_eq!(specification.current_directory(), None);
    assert_eq!(
        MANAGED_OLLAMA_V0_32_15_ENDPOINT.to_string(),
        "127.0.0.1:11434"
    );
    assert_eq!(
        MANAGED_OLLAMA_MODEL_ROOT,
        rewrite_runtime_isolation::MANAGED_RUNTIME_INPUT_ROOT_V1
    );
    for (key, value) in [
        ("HOME", "/tmp"),
        ("TMPDIR", "/tmp"),
        ("OLLAMA_HOST", "127.0.0.1:11434"),
        ("OLLAMA_MODELS", MANAGED_OLLAMA_MODEL_ROOT),
        ("OLLAMA_NO_CLOUD", "1"),
        ("OLLAMA_NOPRUNE", "1"),
        ("OLLAMA_NUM_PARALLEL", "1"),
        ("OLLAMA_MAX_LOADED_MODELS", "1"),
        ("OLLAMA_MAX_QUEUE", "1"),
        ("OLLAMA_FLASH_ATTENTION", "0"),
        ("OLLAMA_VULKAN", "0"),
        ("CUDA_VISIBLE_DEVICES", "-1"),
        ("HIP_VISIBLE_DEVICES", "-1"),
        ("ROCR_VISIBLE_DEVICES", "-1"),
        ("GGML_VK_VISIBLE_DEVICES", "-1"),
        ("GPU_DEVICE_ORDINAL", "-1"),
    ] {
        assert_eq!(
            specification.environment_value(OsStr::new(key)),
            Some(OsStr::new(value))
        );
    }
}

#[test]
fn extra_provider_or_ollama_environment_changes_the_authorized_launch() {
    let expected = managed_ollama_v0_32_15_launch_spec();
    let expected_digest = expected.redacted_digest();
    validate_authorized_launch(&expected_digest, &expected_digest)
        .expect("exact admitted launch digest");
    for key in ["OLLAMA_DEBUG", "HSA_OVERRIDE_GFX_VERSION"] {
        let mut changed = expected.clone();
        changed.insert_environment(key, "1");
        assert_eq!(
            changed.environment_count(),
            expected.environment_count() + 1
        );
        assert_ne!(changed.redacted_digest(), expected.redacted_digest());
        assert!(matches!(
            validate_authorized_launch(&changed.redacted_digest(), &expected_digest),
            Err(ManagedOllamaLaunchError::UnauthorizedLaunch)
        ));
    }
}

#[test]
fn cancelled_operation_token_cannot_suppress_mandatory_close_work() {
    let operation = CancellationToken::new();
    operation.cancel();
    let final_cancellation = mandatory_final_cancellation(&operation);
    assert!(operation.is_cancelled());
    assert!(!final_cancellation.is_cancelled());
}

#[test]
fn final_cleanup_preserves_isolation_and_model_authority_failures() {
    let result = combine_close_results(
        Err(IsolationError::Cancelled),
        Err(ManagedOllamaModelAuthorityError::Revalidation(
            ModelLicenseControlError::Cancelled,
        )),
    );
    assert!(matches!(
        result,
        Err(ManagedOllamaCloseError::IsolationAndModelAuthority {
            isolation: IsolationError::Cancelled,
            authority: ManagedOllamaModelAuthorityError::Revalidation(
                ModelLicenseControlError::Cancelled
            ),
        })
    ));
}
