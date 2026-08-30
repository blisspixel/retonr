use std::{error::Error, fmt};

use rewrite_types::CancellationToken;

use super::{VerifiedGenerationCaseMaterial, VerifiedGenerationCaseMaterialError, ensure_active};

/// Lossless failure from a callback-scoped traversal of every verified source.
pub enum VerifiedGenerationCaseMaterialTraversalError<E> {
    /// A source or complete material validation failed.
    Material {
        /// Exact typed material failure.
        source: VerifiedGenerationCaseMaterialError,
    },
    /// Traversal validation and the final material bracket both failed.
    MaterialAndFinalValidation {
        /// First exact traversal validation failure.
        primary: VerifiedGenerationCaseMaterialError,
        /// Exact final material validation failure.
        final_validation: Box<VerifiedGenerationCaseMaterialError>,
    },
    /// The callback rejected one exact source and final validation succeeded.
    Callback {
        /// Semantic suite index supplied to the callback.
        semantic_index: usize,
        /// Caller-defined typed callback failure.
        source: E,
    },
    /// The callback failed and final complete material validation also failed.
    CallbackAndFinalValidation {
        /// Semantic suite index supplied to the callback.
        semantic_index: usize,
        /// Caller-defined typed callback failure.
        source: E,
        /// Exact final material validation failure.
        final_validation: Box<VerifiedGenerationCaseMaterialError>,
    },
    /// The callback and its source lease validation both failed.
    CallbackAndSourceValidation {
        /// Semantic suite index supplied to the callback.
        semantic_index: usize,
        /// Caller-defined typed callback failure.
        source: E,
        /// Exact source-lease validation failure.
        source_validation: Box<VerifiedGenerationCaseMaterialError>,
        /// Optional exact final complete material validation failure.
        final_validation: Option<Box<VerifiedGenerationCaseMaterialError>>,
    },
}

impl<E> fmt::Debug for VerifiedGenerationCaseMaterialTraversalError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("VerifiedGenerationCaseMaterialTraversalError");
        match self {
            Self::Material { .. } => debug.field("kind", &"material"),
            Self::MaterialAndFinalValidation { .. } => {
                debug.field("kind", &"material_and_final_validation")
            }
            Self::Callback { semantic_index, .. } => debug
                .field("kind", &"callback")
                .field("semantic_index", semantic_index),
            Self::CallbackAndFinalValidation { semantic_index, .. } => debug
                .field("kind", &"callback_and_final_validation")
                .field("semantic_index", semantic_index),
            Self::CallbackAndSourceValidation { semantic_index, .. } => debug
                .field("kind", &"callback_and_source_validation")
                .field("semantic_index", semantic_index),
        };
        debug.finish_non_exhaustive()
    }
}

impl<E> fmt::Display for VerifiedGenerationCaseMaterialTraversalError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Material { .. } => {
                formatter.write_str("generation case-material traversal failed")
            }
            Self::MaterialAndFinalValidation { .. } => formatter
                .write_str("generation case-material traversal and final validation failed"),
            Self::Callback { semantic_index, .. } => write!(
                formatter,
                "generation case-material callback failed at index {semantic_index}"
            ),
            Self::CallbackAndFinalValidation { semantic_index, .. } => write!(
                formatter,
                "generation case-material callback and final validation failed at index {semantic_index}"
            ),
            Self::CallbackAndSourceValidation { semantic_index, .. } => write!(
                formatter,
                "generation case-material callback and source validation failed at index {semantic_index}"
            ),
        }
    }
}

impl<E: Error + 'static> Error for VerifiedGenerationCaseMaterialTraversalError<E> {}

impl VerifiedGenerationCaseMaterial<'_> {
    /// Applies a fallible callback to every exact source in semantic order.
    ///
    /// Traversal stops after the first callback or source-lease failure, then runs
    /// the mandatory final complete-material validation. The error preserves both
    /// failures when the callback and a later validation fail together.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedGenerationCaseMaterialTraversalError`] for cancellation,
    /// material drift, source-lease drift, or a caller-defined callback failure.
    pub fn try_with_all_case_source_bytes<T, E, F>(
        &self,
        cancellation: &CancellationToken,
        mut use_bytes: F,
    ) -> Result<Vec<T>, VerifiedGenerationCaseMaterialTraversalError<E>>
    where
        F: for<'bytes> FnMut(usize, &'bytes [u8]) -> Result<T, E>,
    {
        self.revalidate(cancellation)
            .map_err(|source| VerifiedGenerationCaseMaterialTraversalError::Material { source })?;
        let mut output = Vec::with_capacity(self.source_leases.len());
        let mut callback_failure = None;
        let mut validation_failure = None;
        for (semantic_index, source_lease) in self.source_leases.iter().enumerate() {
            if let Err(source) = ensure_active(cancellation) {
                validation_failure = Some(source);
                break;
            }
            let mut callback_result = None;
            let lease_result = source_lease.with_source_bytes(cancellation, |bytes| {
                callback_result = Some(use_bytes(semantic_index, bytes));
            });
            match callback_result {
                Some(Ok(value)) => output.push(value),
                Some(Err(source)) => callback_failure = Some((semantic_index, source)),
                None => {}
            }
            if let Err(source) = lease_result {
                validation_failure = Some(VerifiedGenerationCaseMaterialError::SourceLease {
                    semantic_index,
                    source,
                });
            }
            if callback_failure.is_some() || validation_failure.is_some() {
                break;
            }
        }
        let final_validation = self.revalidate(cancellation).err();
        resolve_traversal(
            output,
            callback_failure,
            validation_failure,
            final_validation,
        )
    }
}

fn resolve_traversal<T, E>(
    output: Vec<T>,
    callback: Option<(usize, E)>,
    validation: Option<VerifiedGenerationCaseMaterialError>,
    final_validation: Option<VerifiedGenerationCaseMaterialError>,
) -> Result<Vec<T>, VerifiedGenerationCaseMaterialTraversalError<E>> {
    match (callback, validation, final_validation) {
        (None, None, None) => Ok(output),
        (None, Some(source), None) | (None, None, Some(source)) => {
            Err(VerifiedGenerationCaseMaterialTraversalError::Material { source })
        }
        (None, Some(primary), Some(final_validation)) => Err(
            VerifiedGenerationCaseMaterialTraversalError::MaterialAndFinalValidation {
                primary,
                final_validation: Box::new(final_validation),
            },
        ),
        (Some((semantic_index, source)), None, None) => {
            Err(VerifiedGenerationCaseMaterialTraversalError::Callback {
                semantic_index,
                source,
            })
        }
        (Some((semantic_index, source)), None, Some(final_validation)) => Err(
            VerifiedGenerationCaseMaterialTraversalError::CallbackAndFinalValidation {
                semantic_index,
                source,
                final_validation: Box::new(final_validation),
            },
        ),
        (Some((semantic_index, source)), Some(source_validation), final_validation) => Err(
            VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
                semantic_index,
                source,
                source_validation: Box::new(source_validation),
                final_validation: final_validation.map(Box::new),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn resolver_preserves_every_callback_and_validation_combination() {
        let material = || VerifiedGenerationCaseMaterialError::Cancelled;
        let callback = || io::Error::other("callback details");
        let cases = [
            resolve_traversal::<(), io::Error>(Vec::new(), None, Some(material()), None),
            resolve_traversal::<(), io::Error>(
                Vec::new(),
                None,
                Some(material()),
                Some(material()),
            ),
            resolve_traversal(Vec::<()>::new(), Some((2, callback())), None, None),
            resolve_traversal(
                Vec::<()>::new(),
                Some((2, callback())),
                None,
                Some(material()),
            ),
            resolve_traversal(
                Vec::<()>::new(),
                Some((2, callback())),
                Some(material()),
                None,
            ),
            resolve_traversal(
                Vec::<()>::new(),
                Some((2, callback())),
                Some(material()),
                Some(material()),
            ),
        ];
        for error in cases.into_iter().map(Result::unwrap_err) {
            let debug = format!("{error:?}");
            let display = error.to_string();
            assert!(debug.contains("kind"));
            assert!(display.contains("generation case-material"));
            assert!(!debug.contains("callback details"));
        }
        assert_eq!(
            resolve_traversal::<u8, io::Error>(vec![7], None, None, None).expect("success"),
            [7]
        );
    }
}
