#[path = "support/model_runtime.rs"]
mod model_runtime;
#[path = "support/qualification.rs"]
mod qualification;

#[cfg(test)]
pub(crate) use model_runtime::{Fixture, RuntimeFixture, frozen_with_label, launch};
#[cfg(all(feature = "test-support", not(test)))]
pub(crate) use model_runtime::{Fixture, RuntimeFixture, launch};
#[cfg(test)]
pub(super) use qualification::portable_and_policy_substitution_cases;
#[cfg(test)]
pub(crate) use qualification::{
    QualificationFixture, bindings_with_platform, bindings_with_strategy, changed_state,
    characterized, compilation_input, system, verified_policy,
};
#[cfg(all(feature = "test-support", not(test)))]
pub(crate) use qualification::{characterized, system, verified_policy};
