use rewrite_model::{
    GenerationResourceAttemptResultRecordV1, GenerationResourceEvidenceManifestV1,
};

use super::DerivedResourcePhase;

pub(super) struct FrozenResourcePhaseEvidence {
    results: Vec<GenerationResourceAttemptResultRecordV1>,
    manifest: GenerationResourceEvidenceManifestV1,
}

impl FrozenResourcePhaseEvidence {
    pub(super) fn from_derived(derived: DerivedResourcePhase) -> Self {
        Self {
            results: derived.results,
            manifest: derived.manifest,
        }
    }

    pub(super) fn resource_results(&self) -> &[GenerationResourceAttemptResultRecordV1] {
        &self.results
    }

    pub(super) const fn resource_manifest(&self) -> &GenerationResourceEvidenceManifestV1 {
        &self.manifest
    }

    pub(super) fn matches(&self, derived: &DerivedResourcePhase) -> bool {
        self.results == derived.results && self.manifest == derived.manifest
    }
}
