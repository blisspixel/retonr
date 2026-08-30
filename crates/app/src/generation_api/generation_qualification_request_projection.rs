//! App-owned streaming compiler for complete qualification request projections.

use std::{
    fmt,
    time::{Duration, Instant},
};

use rewrite_model::{
    GenerationClusterRecordV1, GenerationQualificationOperationLimitsV1,
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    GenerationCaseSourceLease, GenerationCaseSourceLeaseError,
    GenerationQualificationRequestBuildError, GenerationQualificationRequestBuilderV1,
};
use crate::ArtifactInventoryError;

mod mandatory_finalization;
mod stream;
mod validation;

use validation::AttemptRoute;

/// Noncloneable operation clock started before preregistration work begins.
///
/// The token binds one validated common-limit value to a start and absolute
/// monotonic deadline. Projection compilation consumes it and requires its limits
/// to equal the exact operation policy, so no caller can extend the deadline by
/// supplying an alternate duration.
///
/// ```compile_fail
/// use rewrite_app::GenerationQualificationOperationDeadlineV1;
/// fn cannot_clone(value: &GenerationQualificationOperationDeadlineV1) {
///     let _forged: GenerationQualificationOperationDeadlineV1 = value.clone();
/// }
/// ```
pub struct GenerationQualificationOperationDeadlineV1 {
    limits: GenerationQualificationOperationLimitsV1,
    started: Instant,
    deadline: Instant,
}

impl GenerationQualificationOperationDeadlineV1 {
    /// Starts one operation clock from the exact typed common limits.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for cancellation or an unrepresentable deadline.
    pub fn start(
        limits: GenerationQualificationOperationLimitsV1,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationQualificationRequestProjectionError> {
        check_cancelled(cancellation)?;
        let started = Instant::now();
        let deadline = started
            .checked_add(Duration::from_millis(u64::from(
                limits.maximum_elapsed_milliseconds(),
            )))
            .ok_or(GenerationQualificationRequestProjectionError::DeadlineUnavailable)?;
        check_gate(cancellation, deadline)?;
        Ok(Self {
            limits,
            started,
            deadline,
        })
    }

    /// Returns the captured monotonic start instant.
    #[must_use]
    pub const fn started(&self) -> Instant {
        self.started
    }

    /// Returns the fixed absolute monotonic deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }
}

/// One suite-order case's retained source and exact target and baseline builders.
pub struct GenerationQualificationRequestProjectionCaseAuthoritiesV1<'store> {
    /// Retained exact source capability for this case.
    pub source: GenerationCaseSourceLease<'store>,
    /// Exact cluster named by this case and its planned attempts.
    pub cluster: GenerationClusterRecordV1,
    /// Exact target-system builder for this case.
    pub target_builder: GenerationQualificationRequestBuilderV1,
    /// Exact baseline-system builder for this case.
    pub baseline_builder: GenerationQualificationRequestBuilderV1,
}

/// Complete caller-independent inputs for streaming request-projection compilation.
pub struct GenerationQualificationRequestProjectionCompilerV1Input<'records, 'store> {
    /// Operation clock captured before policy construction or projection work.
    pub operation_deadline: GenerationQualificationOperationDeadlineV1,
    /// Exact already constructed operation-policy record, consumed once.
    pub operation_policy: GenerationQualificationOperationPolicyV1,
    /// Complete exact records used to validate the operation policy.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'records>,
    /// Exact non-derived operation-policy input, retained with the authority.
    pub operation_policy_input: GenerationQualificationOperationPolicyV1Input,
    /// Owned case authorities in exact suite semantic order.
    pub case_authorities: Vec<GenerationQualificationRequestProjectionCaseAuthoritiesV1<'store>>,
}

/// Content-free failure while compiling or revalidating a request projection.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationRequestProjectionError {
    /// Cancellation was observed before any deadline or lower-priority failure.
    #[error("generation qualification request projection was cancelled")]
    Cancelled,
    /// The one retained absolute operation deadline was reached.
    #[error("generation qualification request projection deadline was reached")]
    DeadlineExceeded,
    /// The absolute operation deadline could not be represented.
    #[error("generation qualification request projection deadline is unavailable")]
    DeadlineUnavailable,
    /// The operation policy and its complete portable closure did not match.
    #[error("generation qualification request projection operation does not match")]
    OperationMismatch,
    /// A suite-order case authority, cluster, builder, or system did not match.
    #[error("generation qualification request projection case authority does not match")]
    CaseAuthorityMismatch,
    /// A planned attempt could not be assigned to one exact semantic case and repetition.
    #[error("generation qualification request projection route does not match")]
    RouteMismatch,
    /// A retained source capability failed revalidation or changed.
    #[error("generation qualification request projection source does not match")]
    SourceMismatch,
    /// One exact request could not be reproduced at its derived plan ordinal.
    #[error("generation qualification request projection request {attempt_ordinal} failed")]
    RequestBuild {
        /// Derived zero-based attempt ordinal.
        attempt_ordinal: u32,
        /// Content-free lower-layer request-construction failure.
        #[source]
        source: GenerationQualificationRequestBuildError,
    },
    /// The derived entries did not form the exact portable projection.
    #[error("generation qualification request projection derivation does not match")]
    ProjectionMismatch,
    /// Fresh reconstruction differed from the retained projection authority.
    #[error("generation qualification request projection changed")]
    ProjectionDrift,
}

/// Noncloneable, nonserializable authority over one complete content-free projection.
///
/// The authority owns every builder and source lease, but it never retains or
/// exposes a raw request. Each raw request exists only while one builder is
/// exclusively borrowed during compilation or revalidation.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationRequestProjectionV1;
/// fn cannot_clone(value: &VerifiedGenerationQualificationRequestProjectionV1<'_, '_>) {
///     let _copy = (*value).clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationRequestProjectionV1;
/// fn require_serialize<T: serde::Serialize>(_: &T) {}
/// fn cannot_serialize(value: &VerifiedGenerationQualificationRequestProjectionV1<'_, '_>) {
///     require_serialize(value);
/// }
/// ```
pub struct VerifiedGenerationQualificationRequestProjectionV1<'records, 'store> {
    operation_policy: GenerationQualificationOperationPolicyV1,
    operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'records>,
    operation_policy_input: GenerationQualificationOperationPolicyV1Input,
    case_authorities: Vec<GenerationQualificationRequestProjectionCaseAuthoritiesV1<'store>>,
    routes: Vec<AttemptRoute>,
    entry_inputs: Vec<GenerationQualificationRequestProjectionEntryV1Input>,
    projection: GenerationQualificationRequestProjectionV1,
    operation_deadline: GenerationQualificationOperationDeadlineV1,
}

impl<'records, 'store> VerifiedGenerationQualificationRequestProjectionV1<'records, 'store> {
    /// Streams every exact planned request once and derives the complete projection.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for cancellation, deadline expiry, a changed
    /// owner, an incomplete operation closure, request drift, or projection drift.
    pub fn compile(
        input: GenerationQualificationRequestProjectionCompilerV1Input<'records, 'store>,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationQualificationRequestProjectionError> {
        check_gate(cancellation, input.operation_deadline.deadline)?;
        let operation_result = input.operation_policy.validate_against(
            input.operation_policy_relations,
            &input.operation_policy_input,
        );
        check_cancelled(cancellation)?;
        operation_result
            .map_err(|_| GenerationQualificationRequestProjectionError::OperationMismatch)?;
        if input.operation_deadline.limits != input.operation_policy.limits() {
            return Err(GenerationQualificationRequestProjectionError::OperationMismatch);
        }
        let deadline = input.operation_deadline.deadline;
        check_gate(cancellation, deadline)?;

        let mut case_authorities = input.case_authorities;
        let routes = checked_routes(
            &input.operation_policy,
            input.operation_policy_relations,
            &input.operation_policy_input,
            &case_authorities,
            cancellation,
            deadline,
        )?;
        revalidate_sources(&case_authorities, cancellation, deadline)?;
        let entry_inputs = stream::derive_entry_inputs(
            &input.operation_policy,
            input.operation_policy_relations,
            &mut case_authorities,
            &routes,
            cancellation,
            deadline,
        )?;
        let projection_result = GenerationQualificationRequestProjectionV1::new(
            projection_relations(&input.operation_policy, input.operation_policy_relations),
            &entry_inputs,
        );
        check_gate(cancellation, deadline)?;
        let projection = projection_result
            .map_err(|_| GenerationQualificationRequestProjectionError::ProjectionMismatch)?;

        let fresh_routes = checked_routes(
            &input.operation_policy,
            input.operation_policy_relations,
            &input.operation_policy_input,
            &case_authorities,
            cancellation,
            deadline,
        )?;
        if fresh_routes != routes {
            return Err(GenerationQualificationRequestProjectionError::RouteMismatch);
        }
        revalidate_sources(&case_authorities, cancellation, deadline)?;
        let projection_result = projection.validate_against(
            projection_relations(&input.operation_policy, input.operation_policy_relations),
            &entry_inputs,
        );
        check_gate(cancellation, deadline)?;
        projection_result
            .map_err(|_| GenerationQualificationRequestProjectionError::ProjectionMismatch)?;

        Ok(Self {
            operation_policy: input.operation_policy,
            operation_policy_relations: input.operation_policy_relations,
            operation_policy_input: input.operation_policy_input,
            case_authorities,
            routes,
            entry_inputs,
            projection,
            operation_deadline: input.operation_deadline,
        })
    }

    /// Freshly rebuilds every request one at a time and compares the projection.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for cancellation, expiry, owner drift,
    /// request drift, or any changed projection entry.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationRequestProjectionError> {
        let deadline = self.operation_deadline.deadline;
        check_gate(cancellation, deadline)?;
        let routes = checked_routes(
            &self.operation_policy,
            self.operation_policy_relations,
            &self.operation_policy_input,
            &self.case_authorities,
            cancellation,
            deadline,
        )?;
        if routes != self.routes {
            return Err(GenerationQualificationRequestProjectionError::ProjectionDrift);
        }
        revalidate_sources(&self.case_authorities, cancellation, deadline)?;
        let entry_inputs = stream::derive_entry_inputs(
            &self.operation_policy,
            self.operation_policy_relations,
            &mut self.case_authorities,
            &routes,
            cancellation,
            deadline,
        )?;
        if entry_inputs != self.entry_inputs {
            return Err(GenerationQualificationRequestProjectionError::ProjectionDrift);
        }
        let projection_result = self.projection.validate_against(
            projection_relations(&self.operation_policy, self.operation_policy_relations),
            &entry_inputs,
        );
        check_gate(cancellation, deadline)?;
        projection_result
            .map_err(|_| GenerationQualificationRequestProjectionError::ProjectionDrift)?;
        let fresh_routes = checked_routes(
            &self.operation_policy,
            self.operation_policy_relations,
            &self.operation_policy_input,
            &self.case_authorities,
            cancellation,
            deadline,
        )?;
        if fresh_routes != self.routes {
            return Err(GenerationQualificationRequestProjectionError::ProjectionDrift);
        }
        revalidate_sources(&self.case_authorities, cancellation, deadline)?;
        check_gate(cancellation, deadline)
    }

    /// Returns the complete inert portable projection.
    #[must_use]
    pub const fn projection(&self) -> &GenerationQualificationRequestProjectionV1 {
        &self.projection
    }

    /// Returns the internally derived entry inputs in exact plan order.
    #[must_use]
    pub fn entry_inputs(&self) -> &[GenerationQualificationRequestProjectionEntryV1Input] {
        &self.entry_inputs
    }

    /// Returns the exact retained operation-policy record.
    #[must_use]
    pub const fn operation_policy(&self) -> &GenerationQualificationOperationPolicyV1 {
        &self.operation_policy
    }

    /// Returns the exact retained non-derived operation-policy input.
    #[must_use]
    pub const fn operation_policy_input(&self) -> &GenerationQualificationOperationPolicyV1Input {
        &self.operation_policy_input
    }

    /// Returns the complete borrowed operation-policy relationship closure.
    #[must_use]
    pub const fn operation_policy_relations(
        &self,
    ) -> GenerationQualificationOperationPolicyV1Relations<'records> {
        self.operation_policy_relations
    }

    /// Returns the one absolute monotonic deadline established before compilation.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.operation_deadline.deadline
    }

    /// Returns the monotonic instant captured before preregistration work began.
    #[must_use]
    pub const fn started(&self) -> Instant {
        self.operation_deadline.started
    }
}

impl fmt::Debug for VerifiedGenerationQualificationRequestProjectionV1<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationRequestProjectionV1")
            .field(
                "operation_policy_id",
                self.operation_policy.operation_policy_id(),
            )
            .field(
                "request_projection_id",
                self.projection.request_projection_id(),
            )
            .field("case_count", &self.case_authorities.len())
            .field("entry_count", &self.projection.entry_count())
            .finish_non_exhaustive()
    }
}

fn checked_routes(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    operation_input: &GenerationQualificationOperationPolicyV1Input,
    cases: &[GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<Vec<AttemptRoute>, GenerationQualificationRequestProjectionError> {
    check_gate(cancellation, deadline)?;
    let result =
        validation::validate_and_derive_routes(operation_policy, relations, operation_input, cases);
    check_gate(cancellation, deadline)?;
    result
}

fn revalidate_sources(
    cases: &[GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<(), GenerationQualificationRequestProjectionError> {
    for case in cases {
        check_gate(cancellation, deadline)?;
        let result = case.source.revalidate(cancellation);
        check_gate(cancellation, deadline)?;
        result.map_err(|error| map_source_error(&error))?;
    }
    Ok(())
}

fn projection_relations<'a>(
    operation_policy: &'a GenerationQualificationOperationPolicyV1,
    relations: GenerationQualificationOperationPolicyV1Relations<'a>,
) -> GenerationQualificationRequestProjectionV1Relations<'a> {
    GenerationQualificationRequestProjectionV1Relations {
        operation_policy,
        qualification_plan: relations.plan,
        suite: relations.suite,
        planned_attempts: relations.planned_attempts,
    }
}

fn map_source_error(
    error: &GenerationCaseSourceLeaseError,
) -> GenerationQualificationRequestProjectionError {
    match error {
        GenerationCaseSourceLeaseError::Boundary(ArtifactInventoryError::Cancelled) => {
            GenerationQualificationRequestProjectionError::Cancelled
        }
        _ => GenerationQualificationRequestProjectionError::SourceMismatch,
    }
}

fn check_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationRequestProjectionError> {
    if cancellation.is_cancelled() {
        Err(GenerationQualificationRequestProjectionError::Cancelled)
    } else {
        Ok(())
    }
}

fn check_gate(
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<(), GenerationQualificationRequestProjectionError> {
    check_cancelled(cancellation)?;
    if Instant::now() >= deadline {
        Err(GenerationQualificationRequestProjectionError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "generation_qualification_request_projection_tests.rs"]
mod tests;
