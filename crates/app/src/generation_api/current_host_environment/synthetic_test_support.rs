use std::{collections::VecDeque, sync::Mutex};

use rewrite_model::{HostEnvironmentV1, HostEnvironmentV1Input};
use rewrite_types::CancellationToken;

use super::{
    CurrentHostEnvironmentError, CurrentHostEnvironmentSource, VerifiedCurrentHostEnvironment,
};

pub(super) struct SyntheticEnvironmentSource {
    state: Mutex<SyntheticEnvironmentState>,
}

struct SyntheticEnvironmentState {
    observations: VecDeque<HostEnvironmentV1>,
    final_observation: HostEnvironmentV1,
}

impl SyntheticEnvironmentSource {
    fn stable(input: HostEnvironmentV1Input) -> Result<Self, CurrentHostEnvironmentError> {
        let environment = HostEnvironmentV1::new(input)?;
        Ok(Self {
            state: Mutex::new(SyntheticEnvironmentState {
                observations: VecDeque::new(),
                final_observation: environment,
            }),
        })
    }

    fn sequence(inputs: Vec<HostEnvironmentV1Input>) -> Result<Self, CurrentHostEnvironmentError> {
        let observations = inputs
            .into_iter()
            .map(HostEnvironmentV1::new)
            .collect::<Result<VecDeque<_>, _>>()?;
        let final_observation = observations
            .back()
            .cloned()
            .ok_or(CurrentHostEnvironmentError::InvalidObservation)?;
        Ok(Self {
            state: Mutex::new(SyntheticEnvironmentState {
                observations,
                final_observation,
            }),
        })
    }
}

impl CurrentHostEnvironmentSource for SyntheticEnvironmentSource {
    fn observe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<HostEnvironmentV1, CurrentHostEnvironmentError> {
        if cancellation.is_cancelled() {
            return Err(CurrentHostEnvironmentError::Cancelled);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| CurrentHostEnvironmentError::ObservationUnavailable)?;
        Ok(state
            .observations
            .pop_front()
            .unwrap_or_else(|| state.final_observation.clone()))
    }
}

pub(super) fn exact(
    input: HostEnvironmentV1Input,
) -> Result<VerifiedCurrentHostEnvironment, CurrentHostEnvironmentError> {
    VerifiedCurrentHostEnvironment::observe_from_source(
        Box::new(SyntheticEnvironmentSource::stable(input)?),
        &CancellationToken::new(),
    )
}

pub(super) fn exact_sequence(
    inputs: Vec<HostEnvironmentV1Input>,
) -> Result<VerifiedCurrentHostEnvironment, CurrentHostEnvironmentError> {
    VerifiedCurrentHostEnvironment::observe_from_source(
        Box::new(SyntheticEnvironmentSource::sequence(inputs)?),
        &CancellationToken::new(),
    )
}
