use rewrite_model::HostEnvironmentV1;
use rewrite_types::CancellationToken;

use super::{CurrentHostEnvironmentError, CurrentHostEnvironmentSource, ensure_active};

pub(super) struct ProductionCurrentHostEnvironmentSource;

impl CurrentHostEnvironmentSource for ProductionCurrentHostEnvironmentSource {
    fn observe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<HostEnvironmentV1, CurrentHostEnvironmentError> {
        ensure_active(cancellation)?;
        Err(CurrentHostEnvironmentError::UnsupportedPlatform)
    }
}
