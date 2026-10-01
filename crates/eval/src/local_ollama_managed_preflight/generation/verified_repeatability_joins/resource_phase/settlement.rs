//! Private projection retaining the resource and complete join authorities.

use super::VerifiedGenerationQualificationResourcePhase;
use crate::{CompletePassedRepeatabilityRelations, VerifiedCandidateJudgeJoin};
use rewrite_model::{
    GenerationRepeatabilityEvidenceManifestV1, GenerationResourceAttemptResultRecordV1,
    GenerationResourceEvidenceManifestV1,
};
use rewrite_types::CancellationToken;

impl<'store, 'records, 'model, 'runtime>
    VerifiedGenerationQualificationResourcePhase<'store, 'records, 'model, 'runtime>
{
    pub(crate) fn with_settlement_joins<T, E>(
        &mut self,
        cancellation: &CancellationToken,
        callback: impl FnOnce(
            CompletePassedRepeatabilityRelations<'_>,
            &GenerationRepeatabilityEvidenceManifestV1,
            &[GenerationResourceAttemptResultRecordV1],
            &GenerationResourceEvidenceManifestV1,
            &mut [VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>],
        ) -> Result<T, E>,
    ) -> Result<Result<T, E>, ()> {
        self.revalidate(cancellation).map_err(|_| ())?;
        let evidence = &self.evidence;
        let result =
            self.repeatability
                .with_settlement_joins(cancellation, |closure, manifest, joins| {
                    callback(
                        closure,
                        manifest,
                        evidence.resource_results(),
                        evidence.resource_manifest(),
                        joins,
                    )
                });
        let final_validation = self.revalidate(cancellation);
        if final_validation.is_err() {
            return Err(());
        }
        result
    }

    pub(crate) fn revalidate_for_mandatory_settlement_finalization(&mut self) -> Result<(), ()> {
        // Independently visit every retained join even if resource policy or
        // frozen resource evidence fails. Each pass uses an uncancelled token.
        let repeatability = self
            .repeatability
            .revalidate_for_mandatory_settlement_finalization();
        let resource = self.revalidate(&CancellationToken::new());
        if repeatability.is_ok() && resource.is_ok() {
            Ok(())
        } else {
            Err(())
        }
    }
}
