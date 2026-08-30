//! Resource-result mode and exact collection for one candidate batch set.

use rewrite_model::GenerationResourceAttemptResultRecordV1;

use super::{RetainedCandidateBatch, VerifiedCandidateBatchSetRelationship as Relationship};

/// Closed resource-result mode selected at set construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CandidateBatchSetResourceMode {
    /// No batch may carry a resource result.
    Ordinary,
    /// Every batch must carry exactly one resource result.
    ResourceObserved,
}

pub(super) fn validate_mode<B: RetainedCandidateBatch>(
    mode: CandidateBatchSetResourceMode,
    batches: &[B],
) -> Result<(), Relationship> {
    validate_presence(
        mode,
        batches
            .iter()
            .map(|batch| batch.resource_result().is_some()),
    )
}

fn validate_presence(
    mode: CandidateBatchSetResourceMode,
    present: impl IntoIterator<Item = bool>,
) -> Result<(), Relationship> {
    let mut any_present = false;
    let mut any_missing = false;
    for value in present {
        any_present |= value;
        any_missing |= !value;
    }
    match mode {
        CandidateBatchSetResourceMode::Ordinary if any_present => {
            Err(Relationship::UnexpectedResourceResult)
        }
        CandidateBatchSetResourceMode::ResourceObserved if any_missing => {
            Err(Relationship::MissingResourceResult)
        }
        CandidateBatchSetResourceMode::Ordinary
        | CandidateBatchSetResourceMode::ResourceObserved => Ok(()),
    }
}

pub(super) fn collect_results<B: RetainedCandidateBatch>(
    mode: CandidateBatchSetResourceMode,
    batches: &[B],
) -> Result<Option<Vec<&GenerationResourceAttemptResultRecordV1>>, Relationship> {
    validate_mode(mode, batches)?;
    match mode {
        CandidateBatchSetResourceMode::Ordinary => Ok(None),
        CandidateBatchSetResourceMode::ResourceObserved => batches
            .iter()
            .map(RetainedCandidateBatch::resource_result)
            .collect::<Option<Vec<_>>>()
            .map(Some)
            .ok_or(Relationship::ResourceResultClosure),
    }
}

#[cfg(test)]
mod tests {
    use super::{CandidateBatchSetResourceMode as Mode, validate_presence};
    use crate::VerifiedCandidateBatchSetRelationship as Relationship;

    #[test]
    fn ordinary_requires_every_result_to_be_absent() {
        assert_eq!(validate_presence(Mode::Ordinary, [false, false]), Ok(()));
        assert_eq!(
            validate_presence(Mode::Ordinary, [false, true]),
            Err(Relationship::UnexpectedResourceResult)
        );
        assert_eq!(
            validate_presence(Mode::Ordinary, [true, true]),
            Err(Relationship::UnexpectedResourceResult)
        );
    }

    #[test]
    fn strict_requires_every_result_to_be_present() {
        assert_eq!(
            validate_presence(Mode::ResourceObserved, [true, true]),
            Ok(())
        );
        assert_eq!(
            validate_presence(Mode::ResourceObserved, [true, false]),
            Err(Relationship::MissingResourceResult)
        );
        assert_eq!(
            validate_presence(Mode::ResourceObserved, [false, false]),
            Err(Relationship::MissingResourceResult)
        );
    }
}
