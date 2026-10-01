//! Private test-only authority adapter. The actual policy remains source Denied.
//! No shipping feature exposes this constructor or promotes approval metadata.

use super::*;
use rewrite_app::{
    GenerationQualificationPhasePolicyVerifier,
    ProductionGenerationQualificationResourcePolicySource,
};

impl<'store, 'records, 'model, 'runtime>
    VerifiedGenerationQualificationResourcePhase<'store, 'records, 'model, 'runtime>
{
    pub(crate) fn refuse_synthetic_settlement_fixture(&mut self) {
        self.synthetic_resource_settlement = false;
    }

    pub(crate) fn synthetic_settlement_fixture(
        mut repeatability: VerifiedCompletePassedRepeatabilityJoins<
            'store,
            'records,
            'model,
            'runtime,
        >,
        cancellation: &CancellationToken,
    ) -> Self {
        let material =
            crate::generation_case_material::verified_material_test_support::Fixture::judge_pair();
        let policy_scope = super::super::tests::complete_qualification_authority(&material);
        let resource_policy = GenerationQualificationPhasePolicyVerifier::verify_resource(
            super::tests::canonical_resource_policy(),
            policy_scope.operation_policy(),
            &ProductionGenerationQualificationResourcePolicySource::new(),
        )
        .expect("structurally verified source-denied policy");
        let baseline = repeatability
            .operation_policy()
            .baseline_generation_system_id()
            .clone();
        let results = repeatability
            .collect_resource_results(&baseline, cancellation)
            .expect("private retained synthetic resource observations");
        let derived = derive_phase_with_policy(
            &repeatability,
            repeatability.operation_policy().resource_policy_digest(),
            resource_policy.limits(),
            &results,
            cancellation,
        )
        .expect("real resource policy derivation kernel");
        Self {
            repeatability,
            resource_policy,
            evidence: FrozenResourcePhaseEvidence::from_derived(derived),
            synthetic_resource_settlement: true,
        }
    }

    pub(super) fn revalidate_synthetic_resource_settlement(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationResourcePhaseCompilationError> {
        let operation_policy = self.repeatability.operation_policy().clone();
        let baseline = operation_policy.baseline_generation_system_id().clone();
        let results = self
            .repeatability
            .collect_resource_results(&baseline, cancellation)
            .map_err(|error| {
                GenerationQualificationResourcePhaseCompilationError::InitialAuthority(
                    GenerationQualificationResourcePhaseAuthorityError::Repeatability(error),
                )
            })?;
        let derived = derive_phase_with_policy(
            &self.repeatability,
            operation_policy.resource_policy_digest(),
            self.resource_policy.limits(),
            &results,
            cancellation,
        )
        .map_err(GenerationQualificationResourcePhaseCompilationError::Compilation)?;
        if self.evidence.matches(&derived) {
            Ok(())
        } else {
            Err(
                GenerationQualificationResourcePhaseCompilationError::Compilation(
                    GenerationQualificationResourcePhaseDerivationError::Relationship,
                ),
            )
        }
    }
}
