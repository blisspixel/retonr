//! Durable preregistration input retained by Prepared authority.

use rewrite_model_store::GenerationQualificationPreregistrationReadInput;

use super::PreparedGenerationQualificationOperation;

impl PreparedGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Returns the independently retained inert inputs required for a durable cold read.
    ///
    /// This view grants no traffic or live-runtime authority. The model store still
    /// recursively validates every relationship against its durable preregistration.
    pub(crate) fn preregistration_read_input(
        &self,
    ) -> GenerationQualificationPreregistrationReadInput<'_> {
        GenerationQualificationPreregistrationReadInput {
            operation_policy_id: self.readbacks.operation_policy.operation_policy_id(),
            request_projection_id: self.readbacks.request_projection.request_projection_id(),
            operation_policy_relations: self.stream.operation_policy_relations(),
            operation_policy_input: self.stream.operation_policy_input(),
            request_projection_entry_inputs: self.stream.entry_inputs(),
        }
    }
}
