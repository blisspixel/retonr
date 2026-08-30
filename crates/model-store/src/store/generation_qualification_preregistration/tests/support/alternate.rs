use rewrite_model::{
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations, StructuredCompletionRequestBindingId,
};

use super::{Fixture, digest};

impl Fixture {
    pub(in crate::store::generation_qualification_preregistration::tests) fn alternate_projection(
        &self,
    ) -> (
        Vec<GenerationQualificationRequestProjectionEntryV1Input>,
        GenerationQualificationRequestProjectionV1,
    ) {
        let mut inputs = self.entry_inputs.clone();
        inputs[0].structured_completion_request_binding_id =
            StructuredCompletionRequestBindingId::from_derived_digest(digest(
                "alternate structured request",
            ));
        let projection = GenerationQualificationRequestProjectionV1::new(
            GenerationQualificationRequestProjectionV1Relations {
                operation_policy: &self.policy,
                qualification_plan: &self.plan,
                suite: &self.suite,
                planned_attempts: &self.attempts,
            },
            &inputs,
        )
        .expect("alternate projection");
        (inputs, projection)
    }
}
