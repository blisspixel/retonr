use rewrite_inference::GenerationCandidate;
use rewrite_model::CandidateGenerationReceiptSetV1;
use rewrite_types::CancellationToken;

use crate::{
    EVALUATION_SCHEMA_VERSION, EvaluationCase, EvaluationSuite,
    GenerationDeterministicCaseContractError, GenerationDeterministicCaseContractV1,
    VerifiedGenerationCaseMaterial, VerifiedGenerationCaseMaterialTraversalError,
};

use super::identity::derive_semantic_to_lexicographic_permutation;
use super::{
    CandidateDeterministicCompilerError, CandidateDeterministicCompilerRelationship,
    MAX_CANDIDATE_DETERMINISTIC_COMPILER_CASES, MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES,
    ensure_active, relationship_error,
};

// Portable ownership allowance for fixed EvaluationCase fields, string/vector
// descriptors, and container growth. Exact JSON remains bounded independently.
const PROJECTED_SUITE_CONTAINER_BYTES: usize = 64;
const PROJECTED_CASE_FIXED_BYTES: usize = 512;
const PROJECTED_PROTECTED_TERM_FIXED_BYTES: usize = 32;

pub(super) fn validate_material_closure(
    receipt_a: &CandidateGenerationReceiptSetV1,
    receipt_b: &CandidateGenerationReceiptSetV1,
    material: &VerifiedGenerationCaseMaterial<'_>,
) -> Result<(), CandidateDeterministicCompilerError> {
    let suite = material.suite();
    if material.case_count() > MAX_CANDIDATE_DETERMINISTIC_COMPILER_CASES
        || receipt_a.suite_manifest_id() != suite.suite_manifest_id()
        || receipt_b.suite_manifest_id() != suite.suite_manifest_id()
        || receipt_a.entries().len() != material.case_count()
        || receipt_b.entries().len() != material.case_count()
    {
        return Err(relationship_error(
            0,
            CandidateDeterministicCompilerRelationship::SuiteClosure,
        ));
    }
    for (index, (((case, suite_id), entry_a), entry_b)) in material
        .cases()
        .iter()
        .zip(suite.case_ids())
        .zip(receipt_a.entries())
        .zip(receipt_b.entries())
        .enumerate()
    {
        if case.case_id() != suite_id
            || entry_a.case_id() != suite_id
            || entry_b.case_id() != suite_id
        {
            return Err(relationship_error(
                index,
                CandidateDeterministicCompilerRelationship::CaseClosure,
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_projection_bound(
    material: &VerifiedGenerationCaseMaterial<'_>,
    candidate_a: &[&GenerationCandidate],
    candidate_b: &[&GenerationCandidate],
) -> Result<(), CandidateDeterministicCompilerError> {
    validate_projection_parts(material.contracts(), candidate_a, candidate_b)
}

pub(crate) fn validate_projection_subset_bound(
    material: &VerifiedGenerationCaseMaterial<'_>,
    candidate_a: &[&GenerationCandidate],
    candidate_b: &[&GenerationCandidate],
    semantic_indices: &[usize],
) -> Result<(), CandidateDeterministicCompilerError> {
    for candidates in [candidate_a, candidate_b] {
        let candidate_bytes = semantic_indices.iter().try_fold(0_usize, |sum, index| {
            sum.checked_add(
                candidates
                    .get(*index)
                    .ok_or_else(|| {
                        relationship_error(
                            *index,
                            CandidateDeterministicCompilerRelationship::SelectionClosure,
                        )
                    })?
                    .text
                    .len(),
            )
            .ok_or(CandidateDeterministicCompilerError::ProjectedSuiteTooLarge)
        })?;
        let contracts = semantic_indices
            .iter()
            .map(|index| {
                material.contracts().get(*index).ok_or_else(|| {
                    relationship_error(
                        *index,
                        CandidateDeterministicCompilerRelationship::CaseClosure,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if projected_suite_size_iter(contracts.into_iter(), candidate_bytes)
            .is_none_or(|bytes| bytes > MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES)
        {
            return Err(CandidateDeterministicCompilerError::ProjectedSuiteTooLarge);
        }
    }
    Ok(())
}

pub(super) fn validate_projection_parts(
    contracts: &[GenerationDeterministicCaseContractV1],
    candidate_a: &[&GenerationCandidate],
    candidate_b: &[&GenerationCandidate],
) -> Result<(), CandidateDeterministicCompilerError> {
    for candidates in [candidate_a, candidate_b] {
        let candidate_bytes = candidates.iter().try_fold(0_usize, |sum, candidate| {
            sum.checked_add(candidate.text.len())
        });
        if candidate_bytes
            .and_then(|bytes| projected_suite_size(contracts, bytes))
            .is_none_or(|bytes| bytes > MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES)
        {
            return Err(CandidateDeterministicCompilerError::ProjectedSuiteTooLarge);
        }
    }
    Ok(())
}

pub(super) fn projected_suite_size(
    contracts: &[GenerationDeterministicCaseContractV1],
    candidate_bytes: usize,
) -> Option<usize> {
    projected_suite_size_iter(contracts.iter(), candidate_bytes)
}

fn projected_suite_size_iter<'a>(
    mut contracts: impl Iterator<Item = &'a GenerationDeterministicCaseContractV1>,
    candidate_bytes: usize,
) -> Option<usize> {
    contracts
        .try_fold(PROJECTED_SUITE_CONTAINER_BYTES, |total, contract| {
            let source = usize::try_from(contract.source_byte_count()).ok()?;
            let protected = contract
                .protected_terms()
                .iter()
                .try_fold(0_usize, |sum, term| {
                    sum.checked_add(PROJECTED_PROTECTED_TERM_FIXED_BYTES)?
                        .checked_add(term.len())
                })?;
            total
                .checked_add(PROJECTED_CASE_FIXED_BYTES)?
                .checked_add(contract.case_key().len())?
                .checked_add(contract.evaluation_category().len())?
                .checked_add(source)?
                .checked_add(protected)
        })
        .and_then(|base| base.checked_add(candidate_bytes))
}

pub(super) fn project_suites(
    material: &VerifiedGenerationCaseMaterial<'_>,
    candidate_a: &[&GenerationCandidate],
    candidate_b: &[&GenerationCandidate],
    cancellation: &CancellationToken,
) -> Result<(EvaluationSuite, EvaluationSuite, Vec<u32>), CandidateDeterministicCompilerError> {
    let projected = material
        .try_with_all_case_source_bytes(cancellation, |index, source| {
            let contract = material.contracts().get(index).ok_or_else(|| {
                relationship_error(
                    index,
                    CandidateDeterministicCompilerRelationship::CaseClosure,
                )
            })?;
            let text_a = candidate_a.get(index).ok_or_else(|| {
                relationship_error(
                    index,
                    CandidateDeterministicCompilerRelationship::SelectionClosure,
                )
            })?;
            let text_b = candidate_b.get(index).ok_or_else(|| {
                relationship_error(
                    index,
                    CandidateDeterministicCompilerRelationship::SelectionClosure,
                )
            })?;
            project_case_pair(contract, source, text_a.text.clone(), text_b.text.clone())
                .map(|(case_a, case_b)| (index, case_a, case_b))
                .map_err(
                    |source| CandidateDeterministicCompilerError::CaseProjection {
                        semantic_index: index,
                        source,
                    },
                )
        })
        .map_err(map_traversal_error)?;
    ensure_active(cancellation)?;
    let mut projected = projected;
    let permutation = derive_semantic_to_lexicographic_permutation(&projected)?;
    projected.sort_by(|left, right| left.1.id.cmp(&right.1.id));
    let mut cases_a = Vec::with_capacity(projected.len());
    let mut cases_b = Vec::with_capacity(projected.len());
    for (_, case_a, case_b) in projected {
        cases_a.push(case_a);
        cases_b.push(case_b);
    }
    Ok((
        EvaluationSuite {
            schema_version: EVALUATION_SCHEMA_VERSION,
            cases: cases_a,
        },
        EvaluationSuite {
            schema_version: EVALUATION_SCHEMA_VERSION,
            cases: cases_b,
        },
        permutation,
    ))
}

fn map_traversal_error(
    error: VerifiedGenerationCaseMaterialTraversalError<CandidateDeterministicCompilerError>,
) -> CandidateDeterministicCompilerError {
    match error {
        VerifiedGenerationCaseMaterialTraversalError::Material { source } => {
            CandidateDeterministicCompilerError::CaseMaterial { source }
        }
        VerifiedGenerationCaseMaterialTraversalError::MaterialAndFinalValidation {
            primary,
            final_validation,
        } => combine_failures(
            CandidateDeterministicCompilerError::CaseMaterial { source: primary },
            CandidateDeterministicCompilerError::CaseMaterial {
                source: *final_validation,
            },
        ),
        VerifiedGenerationCaseMaterialTraversalError::Callback { source, .. } => source,
        VerifiedGenerationCaseMaterialTraversalError::CallbackAndFinalValidation {
            source,
            final_validation,
            ..
        } => combine_failures(
            source,
            CandidateDeterministicCompilerError::CaseMaterial {
                source: *final_validation,
            },
        ),
        VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
            source,
            source_validation,
            final_validation,
            ..
        } => {
            let combined = combine_failures(
                source,
                CandidateDeterministicCompilerError::CaseMaterial {
                    source: *source_validation,
                },
            );
            match final_validation {
                Some(final_validation) => combine_failures(
                    combined,
                    CandidateDeterministicCompilerError::CaseMaterial {
                        source: *final_validation,
                    },
                ),
                None => combined,
            }
        }
    }
}

fn combine_failures(
    primary: CandidateDeterministicCompilerError,
    final_validation: CandidateDeterministicCompilerError,
) -> CandidateDeterministicCompilerError {
    CandidateDeterministicCompilerError::PrimaryAndFinalValidation {
        primary: Box::new(primary),
        final_validation: Box::new(final_validation),
    }
}

pub(crate) fn project_case_pair(
    contract: &GenerationDeterministicCaseContractV1,
    source: &[u8],
    candidate_a: String,
    candidate_b: String,
) -> Result<(EvaluationCase, EvaluationCase), GenerationDeterministicCaseContractError> {
    let case_a = contract.project_evaluation_case(source, candidate_a)?;
    let case_b = EvaluationCase {
        id: case_a.id.clone(),
        category: case_a.category.clone(),
        source: case_a.source.clone(),
        candidate: candidate_b,
        protected_terms: case_a.protected_terms.clone(),
        reference_judgment: case_a.reference_judgment,
        expected_status: case_a.expected_status,
        expected_reason: case_a.expected_reason,
        expected_output: case_a.expected_output,
    };
    Ok((case_a, case_b))
}

#[cfg(test)]
mod tests {
    use rewrite_model::ArtifactId;
    use rewrite_types::{Digest, ReasonCode, RewriteStatus};

    use crate::generation_case_material::verified_material_test_support::Fixture;
    use crate::{
        ExpectedOutput, GenerationDeterministicCaseContractV1Input, ReferenceJudgment,
        VerifiedGenerationCaseMaterialError,
    };

    use super::*;

    #[test]
    fn candidate_deterministic_compiler_projection_preflight_is_portably_bounded() {
        let candidate = GenerationCandidate {
            ordinal: 0,
            text: "candidate".to_owned(),
        };
        let ordinary = contract(1);
        assert!(
            validate_projection_parts(
                std::slice::from_ref(&ordinary),
                &[&candidate],
                &[&candidate]
            )
            .is_ok()
        );

        let excessive = contract(MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES as u64);
        assert!(matches!(
            validate_projection_parts(
                std::slice::from_ref(&excessive),
                &[&candidate],
                &[&candidate]
            ),
            Err(CandidateDeterministicCompilerError::ProjectedSuiteTooLarge)
        ));
    }

    #[test]
    fn candidate_deterministic_compiler_projection_requires_both_selected_cases() {
        let fixture = Fixture::pair();
        let material = fixture.verify().expect("verified material");
        let candidate = GenerationCandidate {
            ordinal: 0,
            text: "candidate".to_owned(),
        };
        let one = [&candidate];
        let two = [&candidate, &candidate];
        for result in [
            project_suites(&material, &one, &two, &CancellationToken::new()),
            project_suites(&material, &two, &one, &CancellationToken::new()),
        ] {
            assert!(matches!(
                result,
                Err(CandidateDeterministicCompilerError::Relationship {
                    semantic_index: 1,
                    relationship: CandidateDeterministicCompilerRelationship::SelectionClosure,
                })
            ));
        }
    }

    #[test]
    fn candidate_deterministic_compiler_projection_preserves_traversal_failures() {
        let material = || VerifiedGenerationCaseMaterialError::Cancelled;
        let callback = || CandidateDeterministicCompilerError::Cancelled;

        assert!(matches!(
            map_traversal_error(VerifiedGenerationCaseMaterialTraversalError::Material {
                source: material(),
            }),
            CandidateDeterministicCompilerError::CaseMaterial { .. }
        ));
        assert!(matches!(
            map_traversal_error(
                VerifiedGenerationCaseMaterialTraversalError::MaterialAndFinalValidation {
                    primary: material(),
                    final_validation: Box::new(material()),
                }
            ),
            CandidateDeterministicCompilerError::PrimaryAndFinalValidation { .. }
        ));
        assert!(matches!(
            map_traversal_error(VerifiedGenerationCaseMaterialTraversalError::Callback {
                semantic_index: 0,
                source: callback(),
            }),
            CandidateDeterministicCompilerError::Cancelled
        ));
        assert!(matches!(
            map_traversal_error(
                VerifiedGenerationCaseMaterialTraversalError::CallbackAndFinalValidation {
                    semantic_index: 0,
                    source: callback(),
                    final_validation: Box::new(material()),
                }
            ),
            CandidateDeterministicCompilerError::PrimaryAndFinalValidation { .. }
        ));
        for final_validation in [None, Some(Box::new(material()))] {
            assert!(matches!(
                map_traversal_error(
                    VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
                        semantic_index: 0,
                        source: callback(),
                        source_validation: Box::new(material()),
                        final_validation,
                    }
                ),
                CandidateDeterministicCompilerError::PrimaryAndFinalValidation { .. }
            ));
        }
    }

    fn contract(source_byte_count: u64) -> GenerationDeterministicCaseContractV1 {
        let source_digest = Digest::sha256(b"source");
        GenerationDeterministicCaseContractV1::new(GenerationDeterministicCaseContractV1Input {
            case_key: "projection-bound".to_owned(),
            source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
            source_digest,
            source_byte_count,
            language_digest: Digest::sha256(b"language"),
            mode_digest: Digest::sha256(b"mode"),
            format_digest: Digest::sha256(b"format"),
            evaluation_category: "fixture".to_owned(),
            protected_terms: vec!["term".to_owned()],
            reference_judgment: ReferenceJudgment::Unacceptable,
            expected_status: RewriteStatus::Abstained,
            expected_reason: Some(ReasonCode::ProtectedValueChanged),
            expected_output: ExpectedOutput::Source,
            rubric_clause_ids: vec!["fidelity".to_owned()],
        })
        .expect("case contract")
    }
}
