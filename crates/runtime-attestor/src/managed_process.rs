use std::{fs::File, time::Instant};

use rewrite_types::CancellationToken;

use crate::deadline::{ObservationBudget, earliest_deadline, operation_expired};
use crate::{
    AttachedProcessEvidence, AttachedProcessLease, AttachedProcessWitnessError,
    AttachedProcessWitnessLimits, ListenerEndpoint, ManagedLinuxProcessExpectation,
    NativeLoadDiscovery, NativeLoadDiscoveryRequest, NativeLoadObservationRequest,
    NativeLoadObserverError, NativeManagedLinuxProcessLease, NativeManagedLinuxProcessObserver,
    RetainedTcpConnection, RetainedTcpConnectionEvidence, compare_evidence, ensure_active,
    ensure_native_active, map_native_process_error, observe_initial_connection, platform,
    reobserve_connection_once,
};

impl NativeManagedLinuxProcessObserver {
    /// Attaches to one exact managed Linux target using its namespace-local
    /// socket-diagnostics descriptor.
    ///
    /// The descriptor is consumed and retained. This operation never creates a
    /// host-namespace socket-diagnostics fallback.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError`] unless every descriptor, process,
    /// executable, namespace, listener, UID, holder, and resource invariant is met.
    pub fn attach(
        &self,
        endpoint: ListenerEndpoint,
        diagnostics: File,
        expected: ManagedLinuxProcessExpectation,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
    ) -> Result<NativeManagedLinuxProcessLease, AttachedProcessWitnessError> {
        Self::attach_with_deadline(endpoint, diagnostics, expected, limits, cancellation, None)
    }

    /// Attaches to one exact managed Linux target under an already-captured
    /// absolute operation deadline.
    ///
    /// The earlier of the operation deadline and the observer's local ceiling is
    /// retained by the returned lease. The operation deadline is never recaptured
    /// or extended by later observations.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError::DeadlineExceeded`] at or after the
    /// supplied deadline, or the same bounded errors as [`Self::attach`].
    pub fn attach_until(
        &self,
        endpoint: ListenerEndpoint,
        diagnostics: File,
        expected: ManagedLinuxProcessExpectation,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<NativeManagedLinuxProcessLease, AttachedProcessWitnessError> {
        Self::attach_with_deadline(
            endpoint,
            diagnostics,
            expected,
            limits,
            cancellation,
            Some(operation_deadline),
        )
    }

    fn attach_with_deadline(
        endpoint: ListenerEndpoint,
        diagnostics: File,
        expected: ManagedLinuxProcessExpectation,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<NativeManagedLinuxProcessLease, AttachedProcessWitnessError> {
        let result = (|| {
            let mut limits = limits.validate()?;
            let budget = ObservationBudget::fresh(limits.maximum_elapsed, operation_deadline)
                .map_err(|()| AttachedProcessWitnessError::DeadlineExceeded)?;
            limits.maximum_elapsed = budget.maximum_elapsed();
            let started = budget.started();
            ensure_active(cancellation, started, limits)?;
            let platform = platform::ManagedLease::attach(
                endpoint,
                diagnostics,
                expected,
                limits,
                cancellation,
                started,
            )?;
            let initial = platform.initial_evidence().clone();
            Ok(NativeManagedLinuxProcessLease {
                initial,
                endpoint,
                platform,
                limits,
                started,
                operation_deadline,
            })
        })();
        attached_deadline_precedence(result, cancellation, operation_deadline)
    }
}

impl AttachedProcessLease for NativeManagedLinuxProcessLease {
    fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.initial
    }

    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        self.reobserve_with_deadline(cancellation, None)
    }

    fn observe_connection(
        &mut self,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.observe_connection_with_deadline(connection, cancellation, None)
    }

    fn reobserve_connection(
        &mut self,
        connection: RetainedTcpConnection,
        initial: &RetainedTcpConnectionEvidence,
        cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.reobserve_connection_with_deadline(connection, initial, cancellation, None)
    }

    fn observe_native_load(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<rewrite_model::NativeLoadObservation, NativeLoadObserverError> {
        self.observe_native_load_with_deadline(request, cancellation, None)
    }

    fn discover_external_native_components(
        &mut self,
        request: &NativeLoadDiscoveryRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<NativeLoadDiscovery, NativeLoadObserverError> {
        self.discover_external_native_components_with_deadline(request, cancellation, None)
    }
}

impl NativeManagedLinuxProcessLease {
    /// Reobserves the retained managed process under an already-captured absolute deadline.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError::DeadlineExceeded`] at or after the
    /// deadline, or any ordinary process reobservation error.
    pub fn reobserve_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        self.reobserve_with_deadline(cancellation, Some(operation_deadline))
    }

    fn reobserve_with_deadline(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let (started, limits) = self.bounded_limits(operation_deadline)?;
            ensure_active(cancellation, started, limits)?;
            let observed = self.platform.reobserve(limits, cancellation, started)?;
            compare_evidence(&self.initial, &observed)?;
            Ok(observed)
        })();
        attached_deadline_precedence(result, cancellation, operation_deadline)
    }

    /// Attributes an exact retained connection under an absolute operation deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, relationship, cancellation, or native observation error.
    pub fn observe_connection_until(
        &mut self,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.observe_connection_with_deadline(connection, cancellation, Some(operation_deadline))
    }

    fn observe_connection_with_deadline(
        &mut self,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let (started, limits) = self.bounded_limits(operation_deadline)?;
            ensure_active(cancellation, started, limits)?;
            if connection.server() != self.endpoint.socket() {
                return Err(AttachedProcessWitnessError::ConnectionProcessMismatch);
            }
            observe_initial_connection(cancellation, started, limits, || {
                self.platform
                    .observe_connection(connection, limits, cancellation, started)
            })
        })();
        attached_deadline_precedence(result, cancellation, operation_deadline)
    }

    /// Reobserves an exact retained connection under an absolute operation deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, relationship, cancellation, or native observation error.
    pub fn reobserve_connection_until(
        &mut self,
        connection: RetainedTcpConnection,
        initial: &RetainedTcpConnectionEvidence,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.reobserve_connection_with_deadline(
            connection,
            initial,
            cancellation,
            Some(operation_deadline),
        )
    }

    fn reobserve_connection_with_deadline(
        &mut self,
        connection: RetainedTcpConnection,
        initial: &RetainedTcpConnectionEvidence,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let (started, limits) = self.bounded_limits(operation_deadline)?;
            ensure_active(cancellation, started, limits)?;
            if connection.server() != self.endpoint.socket() {
                return Err(AttachedProcessWitnessError::ConnectionProcessMismatch);
            }
            reobserve_connection_once(initial, || {
                self.platform
                    .observe_connection(connection, limits, cancellation, started)
            })
        })();
        attached_deadline_precedence(result, cancellation, operation_deadline)
    }

    /// Observes the server native-load closure under an absolute operation deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, resource, process, or native-load error.
    pub fn observe_native_load_until(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<rewrite_model::NativeLoadObservation, NativeLoadObserverError> {
        self.observe_native_load_with_deadline(request, cancellation, Some(operation_deadline))
    }

    fn observe_native_load_with_deadline(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<rewrite_model::NativeLoadObservation, NativeLoadObserverError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let mut limits = request.validate()?;
            let native_budget =
                ObservationBudget::fresh(limits.maximum_elapsed, operation_deadline)
                    .map_err(|()| NativeLoadObserverError::DeadlineExceeded)?;
            limits.maximum_elapsed = native_budget.maximum_elapsed();
            let native_started = native_budget.started();
            let (process_started, process_limits) = self
                .bounded_limits(operation_deadline)
                .map_err(map_native_process_error)?;
            ensure_native_active(cancellation, native_started, limits)?;
            let before = self
                .platform
                .reobserve(process_limits, cancellation, process_started)
                .map_err(map_native_process_error)?;
            compare_evidence(&self.initial, &before).map_err(map_native_process_error)?;
            let observation = self.platform.observe_native_load(
                request,
                limits,
                cancellation,
                native_started,
                self.initial.evidence_digest(),
            )?;
            let after = self
                .platform
                .reobserve(process_limits, cancellation, process_started)
                .map_err(map_native_process_error)?;
            compare_evidence(&self.initial, &after).map_err(map_native_process_error)?;
            if observation.process_evidence_digest() != self.initial.evidence_digest()
                || observation.runtime_package_manifest_id() != request.expected_package_id
            {
                return Err(NativeLoadObserverError::InvalidObservation);
            }
            Ok(observation)
        })();
        native_deadline_precedence(result, cancellation, operation_deadline)
    }

    fn discover_external_native_components_with_deadline(
        &mut self,
        request: &NativeLoadDiscoveryRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<NativeLoadDiscovery, NativeLoadObserverError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let mut limits = request.validate()?;
            let native_budget =
                ObservationBudget::fresh(limits.maximum_elapsed, operation_deadline)
                    .map_err(|()| NativeLoadObserverError::DeadlineExceeded)?;
            limits.maximum_elapsed = native_budget.maximum_elapsed();
            let native_started = native_budget.started();
            let (process_started, process_limits) = self
                .bounded_limits(operation_deadline)
                .map_err(map_native_process_error)?;
            ensure_native_active(cancellation, native_started, limits)?;
            let before = self
                .platform
                .reobserve(process_limits, cancellation, process_started)
                .map_err(map_native_process_error)?;
            compare_evidence(&self.initial, &before).map_err(map_native_process_error)?;
            let discovery = self.platform.discover_external_native_components(
                request,
                limits,
                cancellation,
                native_started,
                self.initial.evidence_digest(),
            )?;
            let after = self
                .platform
                .reobserve(process_limits, cancellation, process_started)
                .map_err(map_native_process_error)?;
            compare_evidence(&self.initial, &after).map_err(map_native_process_error)?;
            if discovery.process_evidence_digest() != self.initial.evidence_digest()
                || discovery.runtime_package_manifest_id() != request.expected_package_id
            {
                return Err(NativeLoadObserverError::InvalidObservation);
            }
            Ok(discovery)
        })();
        native_deadline_precedence(result, cancellation, operation_deadline)
    }

    pub(super) fn bounded_limits(
        &self,
        operation_deadline: Option<Instant>,
    ) -> Result<(Instant, AttachedProcessWitnessLimits), AttachedProcessWitnessError> {
        let budget = ObservationBudget::retained(
            self.started,
            self.limits.maximum_elapsed,
            operation_deadline,
        )
        .map_err(|()| AttachedProcessWitnessError::DeadlineExceeded)?;
        let mut limits = self.limits;
        limits.maximum_elapsed = budget.maximum_elapsed();
        Ok((budget.started(), limits))
    }

    pub(super) fn deadline(&self, supplied: Option<Instant>) -> Option<Instant> {
        earliest_deadline(self.operation_deadline, supplied)
    }
}

pub(super) fn attached_deadline_precedence<T>(
    result: Result<T, AttachedProcessWitnessError>,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<T, AttachedProcessWitnessError> {
    if operation_expired(operation_deadline) {
        Err(AttachedProcessWitnessError::DeadlineExceeded)
    } else if operation_deadline.is_some() && cancellation.is_cancelled() {
        Err(AttachedProcessWitnessError::Cancelled)
    } else {
        result
    }
}

pub(super) fn native_deadline_precedence<T>(
    result: Result<T, NativeLoadObserverError>,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<T, NativeLoadObserverError> {
    if operation_expired(operation_deadline) {
        Err(NativeLoadObserverError::DeadlineExceeded)
    } else if operation_deadline.is_some() && cancellation.is_cancelled() {
        Err(NativeLoadObserverError::Cancelled)
    } else {
        result
    }
}
