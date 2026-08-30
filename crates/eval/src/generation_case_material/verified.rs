#[cfg(test)]
use std::cell::Cell;
use std::fmt;

use rewrite_app::{
    GenerationCaseSourceLease, GenerationCaseSourceLeaseError, MAX_GENERATION_CASE_SOURCE_BYTES,
};
use rewrite_model::{
    GenerationCaseManifestV1, GenerationSuiteManifestV1, MAX_GENERATION_SUITE_CASES,
};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use super::{GenerationDeterministicCaseContractError, GenerationDeterministicCaseContractV1};

mod identity;
use identity::derive_case_material_set_digest;
mod traversal;
pub use traversal::VerifiedGenerationCaseMaterialTraversalError;

/// Domain prefix for the complete generation case-material set digest.
pub const GENERATION_CASE_MATERIAL_SET_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-case-material-set:v1\0";
/// Maximum cases retained by one verified generation case-material authority.
pub const MAX_GENERATION_CASE_MATERIAL_CASES: usize = MAX_GENERATION_SUITE_CASES;
/// Maximum aggregate declared source bytes retained by one material authority.
pub const MAX_GENERATION_CASE_MATERIAL_TOTAL_SOURCE_BYTES: u64 =
    MAX_GENERATION_CASE_SOURCE_BYTES * MAX_GENERATION_CASE_MATERIAL_CASES as u64;
/// Maximum canonical bytes accepted for the material-set digest input.
pub const MAX_GENERATION_CASE_MATERIAL_IDENTITY_BYTES: usize = 128 * 1024;

/// Caller-selected ceilings below the fixed generation case-material bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationCaseMaterialLimits {
    /// Maximum semantic-order cases accepted by this operation.
    pub maximum_cases: usize,
    /// Maximum aggregate declared source bytes accepted by this operation.
    pub maximum_total_source_bytes: u64,
}

impl GenerationCaseMaterialLimits {
    fn validate(self) -> Result<Self, VerifiedGenerationCaseMaterialError> {
        if self.maximum_cases == 0
            || self.maximum_cases > MAX_GENERATION_CASE_MATERIAL_CASES
            || self.maximum_total_source_bytes == 0
            || self.maximum_total_source_bytes > MAX_GENERATION_CASE_MATERIAL_TOTAL_SOURCE_BYTES
        {
            return Err(VerifiedGenerationCaseMaterialError::InvalidLimits);
        }
        Ok(self)
    }
}

/// Exact relationship rejected while joining a suite, cases, contracts, and leases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifiedGenerationCaseMaterialRelationship {
    /// Collection lengths do not form one complete one-to-one closure.
    CollectionClosure,
    /// A case does not occupy its suite-declared semantic-order position.
    SuiteCase,
    /// A retained source lease does not bind its paired complete case manifest.
    SourceLeaseCase,
    /// Re-derived immutable material identity differs from the retained identity.
    MaterialSetDigest,
}

/// Failure to create, use, or revalidate complete generation case material.
#[derive(Debug, Error)]
pub enum VerifiedGenerationCaseMaterialError {
    /// A caller-selected ceiling is zero or exceeds its fixed hard bound.
    #[error("generation case-material limits are invalid")]
    InvalidLimits,
    /// The case count exceeds the caller-selected ceiling.
    #[error("generation case-material case limit exceeded")]
    CaseLimitExceeded,
    /// Aggregate source size overflowed or exceeded the caller-selected ceiling.
    #[error("generation case-material source byte limit exceeded")]
    SourceByteLimitExceeded,
    /// The requested semantic-order case index does not exist.
    #[error("generation case-material index is out of range")]
    CaseIndexOutOfRange,
    /// The operation was cancelled before authority could be returned or used.
    #[error("generation case-material operation cancelled")]
    Cancelled,
    /// Canonical material-set identity bytes exceeded their fixed ceiling.
    #[error("generation case-material identity exceeds its canonical byte limit")]
    CanonicalIdentityTooLarge,
    /// One exact suite, case, contract, lease, or retained-digest relationship differed.
    #[error("generation case-material relationship does not match at index {semantic_index}")]
    Relationship {
        /// Semantic suite position at which the mismatch was observed.
        semantic_index: usize,
        /// Closed relationship category without source bytes or paths.
        relationship: VerifiedGenerationCaseMaterialRelationship,
    },
    /// A deterministic case contract could not be revalidated exactly.
    #[error("generation case-material contract failed at index {semantic_index}")]
    Contract {
        /// Semantic suite position at which validation failed.
        semantic_index: usize,
        /// Exact typed contract failure.
        #[source]
        source: GenerationDeterministicCaseContractError,
    },
    /// A retained source capability could not be revalidated exactly.
    #[error("generation case-material source lease failed at index {semantic_index}")]
    SourceLease {
        /// Semantic suite position at which validation failed.
        semantic_index: usize,
        /// Exact app-owned source-boundary failure.
        #[source]
        source: GenerationCaseSourceLeaseError,
    },
}

/// Opaque authority for one complete semantic-order generation case-material set.
///
/// It owns all source leases and inert portable records. Copied records and the
/// material-set digest remain inert without this live capability.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedGenerationCaseMaterial;
///
/// fn clone_capability(value: &VerifiedGenerationCaseMaterial<'_>) {
///     let _forged: VerifiedGenerationCaseMaterial<'_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedGenerationCaseMaterial;
///
/// fn serialize_capability(value: &VerifiedGenerationCaseMaterial<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct VerifiedGenerationCaseMaterial<'store> {
    suite: GenerationSuiteManifestV1,
    cases: Vec<GenerationCaseManifestV1>,
    contracts: Vec<GenerationDeterministicCaseContractV1>,
    source_leases: Vec<GenerationCaseSourceLease<'store>>,
    material_set_digest: Digest,
    total_source_bytes: u64,
    limits: GenerationCaseMaterialLimits,
    #[cfg(test)]
    revalidation_calls: Cell<usize>,
}

impl<'store> VerifiedGenerationCaseMaterial<'store> {
    /// Consumes and joins one complete suite, its cases, contracts, and source leases.
    ///
    /// Inputs must be in the suite's semantic order. Every source lease is
    /// revalidated before and after relationship and identity validation.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedGenerationCaseMaterialError`] for invalid or exceeded
    /// limits, cancellation, incomplete or reordered closure, relationship drift,
    /// source-boundary drift, or excessive canonical identity bytes.
    pub fn verify(
        suite: GenerationSuiteManifestV1,
        cases: Vec<GenerationCaseManifestV1>,
        contracts: Vec<GenerationDeterministicCaseContractV1>,
        source_leases: Vec<GenerationCaseSourceLease<'store>>,
        limits: GenerationCaseMaterialLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, VerifiedGenerationCaseMaterialError> {
        let limits = limits.validate()?;
        ensure_active(cancellation)?;
        validate_collection_shape(&suite, &cases, &contracts, &source_leases, limits)?;
        let total_source_bytes = validate_total_source_bytes(&cases, limits)?;
        revalidate_leases(&source_leases, cancellation)?;
        validate_relationships(&suite, &cases, &contracts, &source_leases)?;
        let material_set_digest = derive_case_material_set_digest(&suite, &cases)?;
        revalidate_leases(&source_leases, cancellation)?;
        ensure_active(cancellation)?;
        Ok(Self {
            suite,
            cases,
            contracts,
            source_leases,
            material_set_digest,
            total_source_bytes,
            limits,
            #[cfg(test)]
            revalidation_calls: Cell::new(0),
        })
    }

    /// Revalidates every retained lease, relationship, bound, and identity.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedGenerationCaseMaterialError`] for cancellation or any
    /// source, suite, case, contract, collection, limit, or identity drift.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedGenerationCaseMaterialError> {
        #[cfg(test)]
        self.revalidation_calls
            .set(self.revalidation_calls.get().saturating_add(1));
        ensure_active(cancellation)?;
        validate_collection_shape(
            &self.suite,
            &self.cases,
            &self.contracts,
            &self.source_leases,
            self.limits,
        )?;
        let total_source_bytes = validate_total_source_bytes(&self.cases, self.limits)?;
        if total_source_bytes != self.total_source_bytes {
            return Err(relationship_error(
                0,
                VerifiedGenerationCaseMaterialRelationship::MaterialSetDigest,
            ));
        }
        revalidate_leases(&self.source_leases, cancellation)?;
        validate_relationships(
            &self.suite,
            &self.cases,
            &self.contracts,
            &self.source_leases,
        )?;
        if derive_case_material_set_digest(&self.suite, &self.cases)? != self.material_set_digest {
            return Err(relationship_error(
                0,
                VerifiedGenerationCaseMaterialRelationship::MaterialSetDigest,
            ));
        }
        revalidate_leases(&self.source_leases, cancellation)?;
        ensure_active(cancellation)
    }

    /// Supplies one exact semantic-order source only for one callback's duration.
    ///
    /// The complete material authority is revalidated before and after the
    /// callback. The app-owned lease additionally validates around its exact read.
    /// A failed final validation discards the callback result.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedGenerationCaseMaterialError`] for an invalid index,
    /// cancellation, or any source, relationship, limit, or identity drift.
    pub fn with_case_source_bytes<T, F>(
        &self,
        semantic_index: usize,
        cancellation: &CancellationToken,
        use_bytes: F,
    ) -> Result<T, VerifiedGenerationCaseMaterialError>
    where
        F: for<'bytes> FnOnce(&'bytes [u8]) -> T,
    {
        self.revalidate(cancellation)?;
        let source_lease = self
            .source_leases
            .get(semantic_index)
            .ok_or(VerifiedGenerationCaseMaterialError::CaseIndexOutOfRange)?;
        let result = source_lease
            .with_source_bytes(cancellation, use_bytes)
            .map_err(|source| VerifiedGenerationCaseMaterialError::SourceLease {
                semantic_index,
                source,
            })?;
        self.revalidate(cancellation)?;
        Ok(result)
    }

    /// Supplies every exact source in semantic order within one callback bracket.
    ///
    /// The complete material authority is revalidated once before traversal and
    /// once after it. Every app-owned source lease still validates before and after
    /// its own exact read. Callback results are discarded if final validation fails.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedGenerationCaseMaterialError`] for cancellation or any
    /// source, relationship, limit, or identity drift.
    pub fn with_all_case_source_bytes<T, F>(
        &self,
        cancellation: &CancellationToken,
        mut use_bytes: F,
    ) -> Result<Vec<T>, VerifiedGenerationCaseMaterialError>
    where
        F: for<'bytes> FnMut(usize, &'bytes [u8]) -> T,
    {
        self.revalidate(cancellation)?;
        let mut results = Vec::with_capacity(self.source_leases.len());
        for (semantic_index, source_lease) in self.source_leases.iter().enumerate() {
            ensure_active(cancellation)?;
            let result = source_lease
                .with_source_bytes(cancellation, |bytes| use_bytes(semantic_index, bytes))
                .map_err(|source| VerifiedGenerationCaseMaterialError::SourceLease {
                    semantic_index,
                    source,
                })?;
            results.push(result);
        }
        self.revalidate(cancellation)?;
        Ok(results)
    }

    /// Returns the complete inert suite retained by this authority.
    #[must_use]
    pub const fn suite(&self) -> &GenerationSuiteManifestV1 {
        &self.suite
    }

    /// Returns all complete inert case manifests in semantic suite order.
    #[must_use]
    pub fn cases(&self) -> &[GenerationCaseManifestV1] {
        &self.cases
    }

    /// Returns all strict deterministic contracts in semantic suite order.
    #[must_use]
    pub fn contracts(&self) -> &[GenerationDeterministicCaseContractV1] {
        &self.contracts
    }

    /// Returns the number of exact semantic-order cases.
    #[must_use]
    pub fn case_count(&self) -> usize {
        self.cases.len()
    }

    /// Returns the aggregate nonzero source byte count.
    #[must_use]
    pub const fn total_source_bytes(&self) -> u64 {
        self.total_source_bytes
    }

    /// Returns the inert canonical digest of this complete material set.
    #[must_use]
    pub const fn case_material_set_digest(&self) -> &Digest {
        &self.material_set_digest
    }

    #[cfg(test)]
    pub(super) fn revalidation_call_count(&self) -> usize {
        self.revalidation_calls.get()
    }
}

impl fmt::Debug for VerifiedGenerationCaseMaterial<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationCaseMaterial")
            .field("suite_manifest_id", self.suite.suite_manifest_id())
            .field("case_material_set_digest", &self.material_set_digest)
            .field("case_count", &self.cases.len())
            .field("total_source_bytes", &self.total_source_bytes)
            .finish_non_exhaustive()
    }
}

fn validate_collection_shape(
    suite: &GenerationSuiteManifestV1,
    cases: &[GenerationCaseManifestV1],
    contracts: &[GenerationDeterministicCaseContractV1],
    source_leases: &[GenerationCaseSourceLease<'_>],
    limits: GenerationCaseMaterialLimits,
) -> Result<(), VerifiedGenerationCaseMaterialError> {
    if cases.len() > limits.maximum_cases {
        return Err(VerifiedGenerationCaseMaterialError::CaseLimitExceeded);
    }
    if cases.is_empty()
        || cases.len() != suite.case_ids().len()
        || cases.len() != contracts.len()
        || cases.len() != source_leases.len()
    {
        return Err(relationship_error(
            0,
            VerifiedGenerationCaseMaterialRelationship::CollectionClosure,
        ));
    }
    Ok(())
}

fn validate_total_source_bytes(
    cases: &[GenerationCaseManifestV1],
    limits: GenerationCaseMaterialLimits,
) -> Result<u64, VerifiedGenerationCaseMaterialError> {
    let mut total = 0_u64;
    for case in cases {
        total = total
            .checked_add(case.source_byte_count())
            .ok_or(VerifiedGenerationCaseMaterialError::SourceByteLimitExceeded)?;
        if total > limits.maximum_total_source_bytes {
            return Err(VerifiedGenerationCaseMaterialError::SourceByteLimitExceeded);
        }
    }
    Ok(total)
}

fn validate_relationships(
    suite: &GenerationSuiteManifestV1,
    cases: &[GenerationCaseManifestV1],
    contracts: &[GenerationDeterministicCaseContractV1],
    source_leases: &[GenerationCaseSourceLease<'_>],
) -> Result<(), VerifiedGenerationCaseMaterialError> {
    for (semantic_index, (((suite_case_id, case), contract), source_lease)) in suite
        .case_ids()
        .iter()
        .zip(cases)
        .zip(contracts)
        .zip(source_leases)
        .enumerate()
    {
        if suite_case_id != case.case_id() {
            return Err(relationship_error(
                semantic_index,
                VerifiedGenerationCaseMaterialRelationship::SuiteCase,
            ));
        }
        contract.validate_case_manifest(case).map_err(|source| {
            VerifiedGenerationCaseMaterialError::Contract {
                semantic_index,
                source,
            }
        })?;
        if !source_lease.matches_case_manifest(case) {
            return Err(relationship_error(
                semantic_index,
                VerifiedGenerationCaseMaterialRelationship::SourceLeaseCase,
            ));
        }
    }
    Ok(())
}

fn revalidate_leases(
    leases: &[GenerationCaseSourceLease<'_>],
    cancellation: &CancellationToken,
) -> Result<(), VerifiedGenerationCaseMaterialError> {
    for (semantic_index, lease) in leases.iter().enumerate() {
        ensure_active(cancellation)?;
        lease.revalidate(cancellation).map_err(|source| {
            VerifiedGenerationCaseMaterialError::SourceLease {
                semantic_index,
                source,
            }
        })?;
    }
    Ok(())
}

const fn relationship_error(
    semantic_index: usize,
    relationship: VerifiedGenerationCaseMaterialRelationship,
) -> VerifiedGenerationCaseMaterialError {
    VerifiedGenerationCaseMaterialError::Relationship {
        semantic_index,
        relationship,
    }
}

fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), VerifiedGenerationCaseMaterialError> {
    if cancellation.is_cancelled() {
        Err(VerifiedGenerationCaseMaterialError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
