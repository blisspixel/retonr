use std::{
    future::Future,
    path::Path,
    task::{Context, Poll, Waker},
};

use rewrite_app::{AppError, GroundedRewriteSelection};
use rewrite_model::ArtifactId;
use rewrite_types::CancellationToken;

use super::{BaselineDefinition, BaselineError, BaselineKind, BaselineReport, run_baseline};
use crate::EvaluationSuite;

/// Runs a baseline against a suite, attaching recovered fake-backend conformance
/// for generative kinds.
///
/// [`super::BaselineKind::NoRewrite`] never inspects a repository.
/// Generative kinds recover one generation binding and use the same in-process
/// fake backend as `rewrite`. They fail closed without that recovered binding
/// and never start a runtime or open a network path.
///
/// # Errors
///
/// Returns [`BaselineError`] for an invalid definition, a missing recovered
/// fake-qualified binding, or an artifact mismatch.
pub fn run_attached_baseline(
    definition: &BaselineDefinition,
    suite: &EvaluationSuite,
    data_directory: Option<&Path>,
    requested: Option<&ArtifactId>,
    cancellation: &CancellationToken,
) -> Result<BaselineReport, BaselineError> {
    definition.validate()?;
    if definition.kind == BaselineKind::NoRewrite {
        return super::run_offline_baseline(definition, suite);
    }
    let attached = GroundedRewriteSelection::require_ready(data_directory, requested)
        .map_err(|error| map_attach_error(&error))?;
    let policy = definition
        .inference
        .as_ref()
        .ok_or(BaselineError::InvalidConfiguration)?;
    if policy.artifact_id != *attached.artifact_id() {
        return Err(BaselineError::ArtifactUnavailable);
    }
    block_ready(run_baseline(
        definition,
        suite,
        Some(attached.backend()),
        cancellation,
    ))
}

fn map_attach_error(error: &AppError) -> BaselineError {
    match error {
        AppError::GroundedUnavailable | AppError::GroundedRuntimeUnavailable => {
            BaselineError::MissingBackend
        }
        AppError::GroundedSelectionMismatch => BaselineError::ArtifactUnavailable,
        _ => BaselineError::Discovery,
    }
}

fn block_ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("conformance-backed baseline must complete immediately"),
    }
}

#[cfg(test)]
mod tests {
    use rewrite_app::{ArtifactImportLimits, ArtifactRepository, OfflineArtifactImportRequest};
    use rewrite_inference::candidate_output_contract;
    use rewrite_model::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, ActivationId, ArtifactId, ArtifactManifest, ArtifactRole,
        ArtifactSource, DeclaredCapabilities, HardwareTier, InstalledArtifact, LicenseDecision,
        LicenseRecord, QUALIFICATION_SCHEMA_VERSION, QualificationRecord, QualificationStatus,
        RuntimeIdentity,
    };
    use rewrite_model_store::ArtifactStateStore;
    use rewrite_types::{CancellationToken, Digest, RewriteStatus};
    use tempfile::tempdir;

    use super::super::{
        BASELINE_SCHEMA_VERSION, BaselineDefinition, BaselineError, BaselineInferencePolicy,
        BaselineKind,
    };
    use super::run_attached_baseline;
    use crate::{EvaluationSuite, parse_suite};

    const ARTIFACT_BYTES: &[u8] = b"eval-conformance-baseline";

    fn suite() -> EvaluationSuite {
        parse_suite(
            r#"{
                "schema_version": 2,
                "cases": [{
                    "id": "case-1",
                    "category": "positive_literal",
                    "source": "Hello world",
                    "candidate": "Hello, world!",
                    "reference_judgment": "acceptable",
                    "expected_status": "rewritten",
                    "expected_reason": null,
                    "expected_output": "candidate"
                }]
            }"#,
        )
        .expect("valid suite")
    }

    fn definition(artifact_id: ArtifactId, digest: Digest) -> BaselineDefinition {
        let prompt_template = "Rewrite conservatively.".to_owned();
        let output = candidate_output_contract();
        BaselineDefinition {
            schema_version: BASELINE_SCHEMA_VERSION,
            id: "direct-conformance-v1".to_owned(),
            kind: BaselineKind::DirectPrompt,
            inference: Some(BaselineInferencePolicy {
                artifact_id,
                artifact_digest: digest,
                prompt_template_digest: Digest::sha256(prompt_template.as_bytes()),
                prompt_template,
                output,
                source_byte_limit: 4_096,
                input_byte_limit: 8_192,
                context_token_limit: 8_192,
                output_token_limit: 512,
                candidate_byte_limit: 4_096,
                sampling: rewrite_inference::SamplingParameters {
                    temperature: 0.0,
                    top_p: 1.0,
                    seed: Some(1),
                },
                reasoning: rewrite_inference::ReasoningPolicy::Disabled,
            }),
            style_description: None,
            retrieved_examples: Vec::new(),
        }
    }

    fn activate_fake_generation(data: &std::path::Path) -> (ArtifactId, Digest) {
        let source = data.parent().expect("temp parent").join("source.gguf");
        std::fs::write(&source, ARTIFACT_BYTES).expect("write artifact");
        let digest = Digest::sha256(ARTIFACT_BYTES);
        let artifact_id = ArtifactId::from_digest(digest.clone());
        ArtifactRepository::new(data)
            .expect("derive repository")
            .import(
                &OfflineArtifactImportRequest {
                    source,
                    manifest: ArtifactManifest {
                        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
                        artifact_id: artifact_id.clone(),
                        source: ArtifactSource {
                            origin: "fixture/model".to_owned(),
                            revision: "fixture".to_owned(),
                        },
                        artifact_digest: digest.clone(),
                        byte_size: u64::try_from(ARTIFACT_BYTES.len()).expect("fixture size"),
                        format: "gguf".to_owned(),
                        family: "fixture".to_owned(),
                        architecture: None,
                        quantization: None,
                        tokenizer: None,
                        licenses: vec![LicenseRecord {
                            component: "weights".to_owned(),
                            identifier: "Apache-2.0".to_owned(),
                            text_digest: Digest::sha256(b"license"),
                        }],
                        declared_capabilities: DeclaredCapabilities {
                            roles: vec![ArtifactRole::Generation],
                            languages: vec!["en".to_owned()],
                            context_tokens: Some(8_192),
                        },
                    },
                },
                ArtifactImportLimits {
                    maximum_artifact_bytes: 1024,
                    maximum_storage_entries: 8,
                },
                &CancellationToken::new(),
            )
            .expect("import");
        let installed = InstalledArtifact {
            artifact_id: artifact_id.clone(),
            artifact_digest: digest.clone(),
            byte_size: u64::try_from(ARTIFACT_BYTES.len()).expect("fixture size"),
            storage_key: format!("artifacts/{}", digest.as_str()),
        };
        let qualification = QualificationRecord {
            schema_version: QUALIFICATION_SCHEMA_VERSION,
            artifact_id: artifact_id.clone(),
            artifact_digest: digest.clone(),
            runtime: RuntimeIdentity {
                backend: "fake".to_owned(),
                version: "1.0.0".to_owned(),
                digest: Some(Digest::sha256(b"runtime")),
            },
            operating_system: "test".to_owned(),
            hardware_tier: HardwareTier {
                id: "test".to_owned(),
                memory_mib: 8_192,
                accelerator: "none".to_owned(),
            },
            supported_roles: vec![ArtifactRole::Generation],
            source_byte_limit: 4_096,
            context_token_limit: 8_192,
            prompt_template_digest: Digest::sha256(b"prompt"),
            request_policy_digest: Digest::sha256(b"request"),
            threshold_policy_digest: Digest::sha256(b"threshold"),
            license_decision: LicenseDecision::LocalUseOnly,
            status: QualificationStatus::Qualified,
        };
        let qualification_id = qualification
            .qualification_id()
            .expect("fixture qualification");
        let mut store =
            ArtifactStateStore::open_existing_writable_exact(&data.join("artifact-state.sqlite3"))
                .expect("open writable store");
        store
            .put_qualification(&qualification)
            .expect("store qualification");
        store
            .activate(
                ActivationId::from_digest(Digest::sha256(b"eval-conformance")),
                ArtifactRole::Generation,
                &installed,
                &qualification_id,
            )
            .expect("activate generation");
        (artifact_id, digest)
    }

    #[test]
    fn generative_kind_fails_closed_without_a_recovered_binding() {
        let digest = Digest::sha256(ARTIFACT_BYTES);
        let definition = definition(ArtifactId::from_digest(digest.clone()), digest);
        assert_eq!(
            run_attached_baseline(&definition, &suite(), None, None, &CancellationToken::new(),),
            Err(BaselineError::MissingBackend)
        );
    }

    #[test]
    fn recovered_fake_binding_runs_direct_prompt_through_conformance() {
        let directory = tempdir().expect("temporary directory");
        let data = directory.path().join("data");
        let (artifact_id, digest) = activate_fake_generation(&data);
        let report = run_attached_baseline(
            &definition(artifact_id.clone(), digest),
            &suite(),
            Some(&data),
            Some(&artifact_id),
            &CancellationToken::new(),
        )
        .expect("attached generative baseline");
        assert_eq!(report.kind, BaselineKind::DirectPrompt);
        assert_eq!(report.artifact_id.as_ref(), Some(&artifact_id));
        assert_eq!(
            report
                .runtime
                .as_ref()
                .map(|runtime| runtime.backend.as_str()),
            Some("fake")
        );
        assert_eq!(report.statuses.failed, 0);
        assert_eq!(
            report.cases[0].status,
            Some(RewriteStatus::UnchangedNoEligibleContent)
        );
        let serialized = serde_json::to_string(&report).expect("serialize report");
        assert!(!serialized.contains("Hello world"));
    }
}
