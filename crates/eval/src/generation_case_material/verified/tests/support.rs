use std::{fs, path::PathBuf};

use rewrite_app::{
    GenerationCaseSourceLease, GenerationCaseSourceLeaseLimits, MAX_GENERATION_CASE_SOURCE_BYTES,
};
use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, GenerationCaseManifestV1, GenerationCaseManifestV1Input,
    GenerationClusterRecordV1, GenerationSuiteManifestV1, InstalledArtifact, LicenseRecord,
};
use rewrite_model_store::{ArtifactStateStore, StoredArtifactInstallation};
use rewrite_types::{CancellationToken, Digest, ReasonCode, RewriteStatus};
use tempfile::TempDir;

use crate::{
    ExpectedOutput, GenerationDeterministicCaseContractV1,
    GenerationDeterministicCaseContractV1Input, ReferenceJudgment,
};

use super::{
    GenerationCaseMaterialLimits, MAX_GENERATION_CASE_MATERIAL_CASES,
    MAX_GENERATION_CASE_MATERIAL_TOTAL_SOURCE_BYTES, VerifiedGenerationCaseMaterial,
    VerifiedGenerationCaseMaterialError,
};

pub(super) const PRIMARY_SOURCE: &[u8] = b"Retain Acme 42 exactly.";
pub(super) const SECONDARY_SOURCE: &[u8] = b"Keep Zeta 7 unchanged.";
const NON_UTF8_SOURCE: &[u8] = b"\xffinvalid";

const SOURCE_LEASE_ENTRY_LIMIT: usize = 16;

#[derive(Clone, Copy)]
struct CaseSpec {
    key: &'static str,
    source: &'static [u8],
    category: &'static str,
    protected_terms: &'static [&'static str],
    reference_judgment: ReferenceJudgment,
    expected_status: RewriteStatus,
    expected_reason: Option<ReasonCode>,
    expected_output: ExpectedOutput,
    rubric_clause_ids: &'static [&'static str],
}

const PRIMARY: CaseSpec = CaseSpec {
    key: "protected-literal",
    source: PRIMARY_SOURCE,
    category: "protected_value_negative",
    protected_terms: &["42", "Acme"],
    reference_judgment: ReferenceJudgment::Unacceptable,
    expected_status: RewriteStatus::Abstained,
    expected_reason: Some(ReasonCode::ProtectedValueChanged),
    expected_output: ExpectedOutput::Source,
    rubric_clause_ids: &["fidelity", "protected-values"],
};
const SECONDARY: CaseSpec = CaseSpec {
    key: "zeta-literal",
    source: SECONDARY_SOURCE,
    category: "protected_value_negative",
    protected_terms: &["7", "Zeta"],
    reference_judgment: ReferenceJudgment::Unacceptable,
    expected_status: RewriteStatus::Abstained,
    expected_reason: Some(ReasonCode::ProtectedValueChanged),
    expected_output: ExpectedOutput::Source,
    rubric_clause_ids: &["fidelity", "protected-values"],
};
const NON_UTF8: CaseSpec = CaseSpec {
    key: "non-utf8-source",
    source: NON_UTF8_SOURCE,
    category: "protected_value_negative",
    protected_terms: &["invalid"],
    reference_judgment: ReferenceJudgment::Unacceptable,
    expected_status: RewriteStatus::Abstained,
    expected_reason: Some(ReasonCode::ProtectedValueChanged),
    expected_output: ExpectedOutput::Source,
    rubric_clause_ids: &["fidelity", "protected-values"],
};
const ELIGIBLE: CaseSpec = CaseSpec {
    key: "eligible-rewrite",
    source: b"Acme 42 needs polish.",
    category: "eligible_rewrite",
    protected_terms: &["42", "Acme"],
    reference_judgment: ReferenceJudgment::Acceptable,
    expected_status: RewriteStatus::Rewritten,
    expected_reason: None,
    expected_output: ExpectedOutput::Candidate,
    rubric_clause_ids: &["fidelity", "protected-values"],
};
const ELIGIBLE_ZETA: CaseSpec = CaseSpec {
    key: "eligible-zeta",
    source: b"Zeta 7 needs polish.",
    category: "eligible_rewrite",
    protected_terms: &["7", "Zeta"],
    reference_judgment: ReferenceJudgment::Acceptable,
    expected_status: RewriteStatus::Rewritten,
    expected_reason: None,
    expected_output: ExpectedOutput::Candidate,
    rubric_clause_ids: &["fidelity", "protected-values"],
};
const ELIGIBLE_NON_UTF8: CaseSpec = CaseSpec {
    key: "eligible-non-utf8",
    source: NON_UTF8_SOURCE,
    category: "eligible_rewrite",
    protected_terms: &["invalid"],
    reference_judgment: ReferenceJudgment::Acceptable,
    expected_status: RewriteStatus::Rewritten,
    expected_reason: None,
    expected_output: ExpectedOutput::Candidate,
    rubric_clause_ids: &["fidelity", "protected-values"],
};

pub(crate) struct Fixture {
    _directory: TempDir,
    pub(super) root: PathBuf,
    #[cfg(unix)]
    pub(super) canonical_sources: Vec<PathBuf>,
    pub(super) store: ArtifactStateStore,
    pub(super) selections: Vec<StoredArtifactInstallation>,
    pub(crate) cases: Vec<GenerationCaseManifestV1>,
    pub(crate) contracts: Vec<GenerationDeterministicCaseContractV1>,
    pub(crate) suite: GenerationSuiteManifestV1,
}

impl Fixture {
    pub(super) fn primary() -> Self {
        Self::new(&[PRIMARY], "suite protocol")
    }

    pub(crate) fn pair() -> Self {
        Self::new(&[PRIMARY, SECONDARY], "suite protocol")
    }

    pub(crate) fn non_utf8() -> Self {
        Self::new(&[NON_UTF8], "suite protocol")
    }

    pub(crate) fn judge_pair() -> Self {
        Self::new(&[ELIGIBLE, PRIMARY], "judge suite protocol")
    }

    pub(crate) fn judge_triple() -> Self {
        Self::new(
            &[ELIGIBLE, PRIMARY, ELIGIBLE_ZETA],
            "judge triple suite protocol",
        )
    }

    pub(crate) fn judge_non_utf8() -> Self {
        Self::new(&[ELIGIBLE_NON_UTF8], "judge non-UTF8 suite protocol")
    }

    pub(crate) fn judge_triple_reversed_eligible_order() -> Self {
        Self::new(
            &[ELIGIBLE_ZETA, PRIMARY, ELIGIBLE],
            "judge reversed triple suite protocol",
        )
    }

    pub(super) fn reversed_pair() -> Self {
        Self::new(&[SECONDARY, PRIMARY], "suite protocol")
    }

    pub(crate) fn pair_with_protocol(protocol: &'static str) -> Self {
        Self::new(&[PRIMARY, SECONDARY], protocol)
    }

    fn new(specs: &[CaseSpec], protocol: &str) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("managed");
        fs::create_dir(&root).expect("create managed root");
        fs::create_dir(root.join("artifacts")).expect("create artifact directory");
        fs::write(root.join(".artifact-import.lock"), []).expect("create lifecycle lock");
        let database = directory.path().join("state.sqlite3");
        let mut store = ArtifactStateStore::open(&database).expect("open artifact state");
        let cluster = GenerationClusterRecordV1::new("literal", digest("cluster policy"))
            .expect("cluster is valid");
        let mut selections = Vec::with_capacity(specs.len());
        #[cfg(unix)]
        let mut canonical_sources = Vec::with_capacity(specs.len());
        let mut contracts = Vec::with_capacity(specs.len());
        let mut cases = Vec::with_capacity(specs.len());
        for spec in specs {
            let source_digest = Digest::sha256(spec.source);
            let artifact_id = ArtifactId::from_digest(source_digest.clone());
            let contract = contract(spec, artifact_id.clone(), source_digest.clone());
            let case = GenerationCaseManifestV1::new(
                &cluster,
                GenerationCaseManifestV1Input {
                    case_key: spec.key.to_owned(),
                    source_artifact_id: artifact_id.clone(),
                    source_digest: source_digest.clone(),
                    source_byte_count: spec.source.len() as u64,
                    case_contract_digest: contract.contract_digest().clone(),
                    language_digest: contract.language_digest().clone(),
                    mode_digest: contract.mode_digest().clone(),
                    format_digest: contract.format_digest().clone(),
                },
            )
            .expect("case is valid");
            let manifest = artifact_manifest(artifact_id.clone(), source_digest.clone(), spec);
            let installed = InstalledArtifact {
                artifact_id,
                artifact_digest: source_digest.clone(),
                byte_size: spec.source.len() as u64,
                storage_key: format!("artifacts/{}", source_digest.as_str()),
            };
            let canonical = root.join("artifacts").join(source_digest.as_str());
            fs::write(&canonical, spec.source).expect("write canonical source");
            let selection = store
                .put_installation(&manifest, &installed)
                .expect("store source installation")
                .installation;
            selections.push(selection);
            #[cfg(unix)]
            canonical_sources.push(canonical);
            contracts.push(contract);
            cases.push(case);
        }
        let suite =
            GenerationSuiteManifestV1::new(digest(protocol), &cases).expect("suite is valid");
        Self {
            _directory: directory,
            root,
            #[cfg(unix)]
            canonical_sources,
            store,
            selections,
            cases,
            contracts,
            suite,
        }
    }

    pub(super) fn leases(&self) -> Vec<GenerationCaseSourceLease<'_>> {
        self.leases_in_order(&(0..self.cases.len()).collect::<Vec<_>>())
    }

    pub(super) fn leases_in_order(&self, order: &[usize]) -> Vec<GenerationCaseSourceLease<'_>> {
        order
            .iter()
            .map(|index| {
                GenerationCaseSourceLease::acquire(
                    &self.root,
                    &self.store,
                    self.selections[*index].clone(),
                    &self.cases[*index],
                    GenerationCaseSourceLeaseLimits {
                        maximum_source_bytes: MAX_GENERATION_CASE_SOURCE_BYTES,
                        maximum_storage_entries: SOURCE_LEASE_ENTRY_LIMIT,
                    },
                    &CancellationToken::new(),
                )
                .expect("source lease acquires")
            })
            .collect()
    }

    pub(crate) fn verify(
        &self,
    ) -> Result<VerifiedGenerationCaseMaterial<'_>, VerifiedGenerationCaseMaterialError> {
        VerifiedGenerationCaseMaterial::verify(
            self.suite.clone(),
            self.cases.clone(),
            self.contracts.clone(),
            self.leases(),
            material_limits(),
            &CancellationToken::new(),
        )
    }
}

pub(super) const fn material_limits() -> GenerationCaseMaterialLimits {
    GenerationCaseMaterialLimits {
        maximum_cases: MAX_GENERATION_CASE_MATERIAL_CASES,
        maximum_total_source_bytes: MAX_GENERATION_CASE_MATERIAL_TOTAL_SOURCE_BYTES,
    }
}

pub(super) fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn contract(
    spec: &CaseSpec,
    source_artifact_id: ArtifactId,
    source_digest: Digest,
) -> GenerationDeterministicCaseContractV1 {
    GenerationDeterministicCaseContractV1::new(GenerationDeterministicCaseContractV1Input {
        case_key: spec.key.to_owned(),
        source_artifact_id,
        source_digest,
        source_byte_count: spec.source.len() as u64,
        language_digest: digest("language"),
        mode_digest: digest("mode"),
        format_digest: digest("format"),
        evaluation_category: spec.category.to_owned(),
        protected_terms: spec
            .protected_terms
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        reference_judgment: spec.reference_judgment,
        expected_status: spec.expected_status,
        expected_reason: spec.expected_reason,
        expected_output: spec.expected_output,
        rubric_clause_ids: spec
            .rubric_clause_ids
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    })
    .expect("contract is valid")
}

fn artifact_manifest(
    artifact_id: ArtifactId,
    source_digest: Digest,
    spec: &CaseSpec,
) -> ArtifactManifest {
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id,
        source: ArtifactSource {
            origin: format!("fixture/{}", spec.key),
            revision: "fixture-revision".to_owned(),
        },
        artifact_digest: source_digest,
        byte_size: spec.source.len() as u64,
        format: "utf8".to_owned(),
        family: "generation-case-source".to_owned(),
        architecture: None,
        quantization: None,
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "source".to_owned(),
            identifier: "CC0-1.0".to_owned(),
            text_digest: digest("license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: None,
        },
    }
}
