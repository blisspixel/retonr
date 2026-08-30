use std::{convert::Infallible, error::Error, fmt};

use crate::generation_case_material::{
    VerifiedGenerationCaseMaterialError, VerifiedGenerationCaseMaterialTraversalError,
};

use super::AttemptVisitStop;
use crate::candidate_judge_preparation::{
    CandidateJudgePreparationError, CandidateJudgePreparationRelationship,
    CandidateJudgePreparationRequestFailure,
};

pub(crate) enum CandidateJudgeRunnerRequestError<E> {
    Preparation {
        source: CandidateJudgePreparationError,
    },
    CursorOutOfRange {
        schedule_cursor: usize,
    },
    AttemptClosure {
        schedule_cursor: usize,
    },
    RequestBindingMismatch {
        schedule_cursor: usize,
    },
    Callback {
        schedule_cursor: usize,
        source: E,
    },
    PrimaryAndFinalValidation {
        primary: Box<CandidateJudgeRunnerRequestError<E>>,
        final_validation: Box<CandidateJudgePreparationError>,
    },
}

impl<E> fmt::Debug for CandidateJudgeRunnerRequestError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CandidateJudgeRunnerRequestError");
        match self {
            Self::Preparation { .. } => debug.field("kind", &"preparation"),
            Self::CursorOutOfRange { schedule_cursor } => debug
                .field("kind", &"cursor_out_of_range")
                .field("schedule_cursor", schedule_cursor),
            Self::AttemptClosure { schedule_cursor } => debug
                .field("kind", &"attempt_closure")
                .field("schedule_cursor", schedule_cursor),
            Self::RequestBindingMismatch { schedule_cursor } => debug
                .field("kind", &"request_binding_mismatch")
                .field("schedule_cursor", schedule_cursor),
            Self::Callback {
                schedule_cursor, ..
            } => debug
                .field("kind", &"callback")
                .field("schedule_cursor", schedule_cursor),
            Self::PrimaryAndFinalValidation { .. } => {
                debug.field("kind", &"primary_and_final_validation")
            }
        };
        debug.finish_non_exhaustive()
    }
}

impl<E> fmt::Display for CandidateJudgeRunnerRequestError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preparation { .. } => {
                formatter.write_str("judge runner handoff validation failed")
            }
            Self::CursorOutOfRange { schedule_cursor } => {
                write!(
                    formatter,
                    "judge schedule cursor {schedule_cursor} is out of range"
                )
            }
            Self::AttemptClosure { schedule_cursor } => write!(
                formatter,
                "judge attempt relationship failed at cursor {schedule_cursor}"
            ),
            Self::RequestBindingMismatch { schedule_cursor } => write!(
                formatter,
                "judge request identity failed at cursor {schedule_cursor}"
            ),
            Self::Callback {
                schedule_cursor, ..
            } => write!(
                formatter,
                "judge request callback failed at cursor {schedule_cursor}"
            ),
            Self::PrimaryAndFinalValidation { .. } => formatter
                .write_str("judge request callback and final handoff validation both failed"),
        }
    }
}

impl<E: Error + 'static> Error for CandidateJudgeRunnerRequestError<E> {}

pub(super) fn resolve_traversal<T, E>(
    schedule_cursor: usize,
    result: Result<Vec<()>, VerifiedGenerationCaseMaterialTraversalError<AttemptVisitStop<T, E>>>,
) -> Result<T, CandidateJudgeRunnerRequestError<E>> {
    match result {
        Ok(_) => Err(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor }),
        Err(VerifiedGenerationCaseMaterialTraversalError::Material { source }) => {
            Err(preparation(material_error(source)))
        }
        Err(VerifiedGenerationCaseMaterialTraversalError::MaterialAndFinalValidation {
            primary,
            final_validation,
        }) => Err(preparation(combine_preparation(
            material_error(primary),
            material_error(*final_validation),
        ))),
        Err(VerifiedGenerationCaseMaterialTraversalError::Callback { source, .. }) => {
            resolve_stop(schedule_cursor, source, None)
        }
        Err(VerifiedGenerationCaseMaterialTraversalError::CallbackAndFinalValidation {
            source,
            final_validation,
            ..
        }) => resolve_stop(
            schedule_cursor,
            source,
            Some(material_error(*final_validation)),
        ),
        Err(VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
            source,
            source_validation,
            final_validation,
            ..
        }) => {
            let mut validation = material_error(*source_validation);
            if let Some(final_validation) = final_validation {
                validation = combine_preparation(validation, material_error(*final_validation));
            }
            resolve_stop(schedule_cursor, source, Some(validation))
        }
    }
}

fn resolve_stop<T, E>(
    schedule_cursor: usize,
    stop: AttemptVisitStop<T, E>,
    final_validation: Option<CandidateJudgePreparationError>,
) -> Result<T, CandidateJudgeRunnerRequestError<E>> {
    let primary = match stop {
        AttemptVisitStop::Completed(value) => match final_validation {
            None => return Ok(value),
            Some(source) => {
                drop(value);
                return Err(preparation(source));
            }
        },
        AttemptVisitStop::Preparation(source) => preparation(source),
        AttemptVisitStop::RequestBindingMismatch => {
            CandidateJudgeRunnerRequestError::RequestBindingMismatch { schedule_cursor }
        }
        AttemptVisitStop::Callback(source) => CandidateJudgeRunnerRequestError::Callback {
            schedule_cursor,
            source,
        },
    };
    match final_validation {
        None => Err(primary),
        Some(final_validation) => Err(
            CandidateJudgeRunnerRequestError::PrimaryAndFinalValidation {
                primary: Box::new(primary),
                final_validation: Box::new(final_validation),
            },
        ),
    }
}

fn preparation<E>(source: CandidateJudgePreparationError) -> CandidateJudgeRunnerRequestError<E> {
    CandidateJudgeRunnerRequestError::Preparation { source }
}

fn material_error(source: VerifiedGenerationCaseMaterialError) -> CandidateJudgePreparationError {
    CandidateJudgePreparationError::CaseMaterial { source }
}

fn combine_preparation(
    primary: CandidateJudgePreparationError,
    final_validation: CandidateJudgePreparationError,
) -> CandidateJudgePreparationError {
    CandidateJudgePreparationError::PrimaryAndFinalValidation {
        primary: Box::new(primary),
        final_validation: Box::new(final_validation),
    }
}

pub(super) fn request_error_into_preparation(
    error: CandidateJudgeRunnerRequestError<Infallible>,
) -> CandidateJudgePreparationError {
    match error {
        CandidateJudgeRunnerRequestError::Preparation { source } => source,
        CandidateJudgeRunnerRequestError::CursorOutOfRange { .. }
        | CandidateJudgeRunnerRequestError::AttemptClosure { .. } => {
            CandidateJudgePreparationError::Relationship(
                CandidateJudgePreparationRelationship::ScheduleClosure,
            )
        }
        CandidateJudgeRunnerRequestError::RequestBindingMismatch { schedule_cursor } => {
            CandidateJudgePreparationError::Request {
                schedule_index: schedule_cursor,
                failure: CandidateJudgePreparationRequestFailure::InvalidRequest,
            }
        }
        CandidateJudgeRunnerRequestError::Callback { source, .. } => match source {},
        CandidateJudgeRunnerRequestError::PrimaryAndFinalValidation {
            primary,
            final_validation,
        } => combine_preparation(request_error_into_preparation(*primary), *final_validation),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn traversal_resolution_preserves_and_redacts_every_failure_shape() {
        let material = || VerifiedGenerationCaseMaterialError::Cancelled;
        let callback = || io::Error::other("sensitive callback bytes");
        let cases = [
            resolve_traversal::<(), io::Error>(0, Ok(Vec::new())),
            resolve_traversal(
                0,
                Err(VerifiedGenerationCaseMaterialTraversalError::Material { source: material() }),
            ),
            resolve_traversal(
                0,
                Err(
                    VerifiedGenerationCaseMaterialTraversalError::MaterialAndFinalValidation {
                        primary: material(),
                        final_validation: Box::new(material()),
                    },
                ),
            ),
            resolve_traversal(
                0,
                Err(VerifiedGenerationCaseMaterialTraversalError::Callback {
                    semantic_index: 0,
                    source: AttemptVisitStop::Completed(()),
                }),
            ),
            resolve_traversal(
                0,
                Err(
                    VerifiedGenerationCaseMaterialTraversalError::CallbackAndFinalValidation {
                        semantic_index: 0,
                        source: AttemptVisitStop::Completed(()),
                        final_validation: Box::new(material()),
                    },
                ),
            ),
            resolve_traversal(
                0,
                Err(VerifiedGenerationCaseMaterialTraversalError::Callback {
                    semantic_index: 0,
                    source: AttemptVisitStop::Preparation(
                        CandidateJudgePreparationError::Relationship(
                            CandidateJudgePreparationRelationship::ScheduleClosure,
                        ),
                    ),
                }),
            ),
            resolve_traversal(
                0,
                Err(VerifiedGenerationCaseMaterialTraversalError::Callback {
                    semantic_index: 0,
                    source: AttemptVisitStop::RequestBindingMismatch,
                }),
            ),
            resolve_traversal(
                0,
                Err(VerifiedGenerationCaseMaterialTraversalError::Callback {
                    semantic_index: 0,
                    source: AttemptVisitStop::Callback(callback()),
                }),
            ),
            resolve_traversal(
                0,
                Err(
                    VerifiedGenerationCaseMaterialTraversalError::CallbackAndFinalValidation {
                        semantic_index: 0,
                        source: AttemptVisitStop::Callback(callback()),
                        final_validation: Box::new(material()),
                    },
                ),
            ),
            resolve_traversal(
                0,
                Err(
                    VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
                        semantic_index: 0,
                        source: AttemptVisitStop::Callback(callback()),
                        source_validation: Box::new(material()),
                        final_validation: Some(Box::new(material())),
                    },
                ),
            ),
        ];
        assert!(matches!(cases[3], Ok(())));
        for error in cases.into_iter().filter_map(Result::err) {
            let debug = format!("{error:?}");
            let display = error.to_string();
            assert!(debug.contains("kind"));
            assert!(display.contains("judge"));
            assert!(!debug.contains("sensitive callback bytes"));
        }
    }

    #[test]
    fn content_free_error_mapping_closes_cursor_binding_and_dual_failures() {
        let cursor =
            request_error_into_preparation(CandidateJudgeRunnerRequestError::CursorOutOfRange {
                schedule_cursor: 9,
            });
        assert!(matches!(
            cursor,
            CandidateJudgePreparationError::Relationship(
                CandidateJudgePreparationRelationship::ScheduleClosure
            )
        ));
        let binding = request_error_into_preparation(
            CandidateJudgeRunnerRequestError::RequestBindingMismatch { schedule_cursor: 3 },
        );
        assert!(matches!(
            binding,
            CandidateJudgePreparationError::Request {
                schedule_index: 3,
                failure: CandidateJudgePreparationRequestFailure::InvalidRequest
            }
        ));
        let preparation =
            request_error_into_preparation(CandidateJudgeRunnerRequestError::Preparation {
                source: CandidateJudgePreparationError::Relationship(
                    CandidateJudgePreparationRelationship::JudgePolicyClosure,
                ),
            });
        assert!(matches!(
            preparation,
            CandidateJudgePreparationError::Relationship(
                CandidateJudgePreparationRelationship::JudgePolicyClosure
            )
        ));
        let dual = request_error_into_preparation(
            CandidateJudgeRunnerRequestError::PrimaryAndFinalValidation {
                primary: Box::new(CandidateJudgeRunnerRequestError::AttemptClosure {
                    schedule_cursor: 0,
                }),
                final_validation: Box::new(CandidateJudgePreparationError::CaseMaterial {
                    source: VerifiedGenerationCaseMaterialError::Cancelled,
                }),
            },
        );
        assert!(matches!(
            dual,
            CandidateJudgePreparationError::PrimaryAndFinalValidation { .. }
        ));
    }
}
