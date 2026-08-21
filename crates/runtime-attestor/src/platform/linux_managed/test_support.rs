use std::env;

use crate::AttachedProcessWitnessError;

const REQUIRE_NATIVE_ENVIRONMENT: &str = "REWRITE_ATTESTOR_REQUIRE_NATIVE";

pub(super) fn uncontrolled_access_denied<T>(
    result: &Result<T, AttachedProcessWitnessError>,
) -> bool {
    !native_required()
        && matches!(
            result,
            Err(AttachedProcessWitnessError::ProcessAccessDenied)
        )
}

pub(super) fn expect_native<T>(
    result: Result<T, AttachedProcessWitnessError>,
    context: &str,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(AttachedProcessWitnessError::ProcessAccessDenied) if !native_required() => None,
        Err(error) => panic!("{context}: {error}"),
    }
}

fn native_required() -> bool {
    match env::var(REQUIRE_NATIVE_ENVIRONMENT) {
        Ok(value) if value == "1" => true,
        Ok(value) => panic!("{REQUIRE_NATIVE_ENVIRONMENT} must be 1 when set, got {value:?}"),
        Err(env::VarError::NotPresent) => false,
        Err(env::VarError::NotUnicode(_)) => {
            panic!("{REQUIRE_NATIVE_ENVIRONMENT} must contain valid Unicode")
        }
    }
}
