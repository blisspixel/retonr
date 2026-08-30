use rewrite_ollama_package::{
    RuntimeSourceBuildPlan, VerifiedRuntimeSourceBuildInputs, verify_runtime_source_build_inputs,
};
use rewrite_types::CancellationToken;

mod contract;
mod source;

pub use contract::{
    MAX_RUNTIME_SOURCE_BUILD_BUNDLE_TREE_ENTRIES, RuntimeSourceBuildBundleError,
    RuntimeSourceBuildBundleLimits, RuntimeSourceBuildBundleSource,
    VerifiedRuntimeSourceBuildBundle,
};
use source::PinnedRuntimeSourceBuildBundle;

/// Retained filesystem capability for one byte-verified source-build bundle.
///
/// The lease keeps the manifest, component root, and every declared component
/// pinned so a later build stage does not reopen an untrusted pathname after
/// verification.
pub struct RuntimeSourceBuildBundleLease {
    pinned: PinnedRuntimeSourceBuildBundle,
    verified: VerifiedRuntimeSourceBuildInputs,
    plan: RuntimeSourceBuildPlan,
}

/// One declared component's path, verified identity, byte count, and retained file.
pub type RuntimeSourceBuildComponentCapability = (
    rewrite_model::ArtifactSetRelativePath,
    rewrite_types::Digest,
    u64,
    std::fs::File,
);

/// Retained files required by one controlled build.
///
/// The tuple contains the build program, isolation helper, component root, and
/// all declared component capabilities in canonical manifest order.
pub type RuntimeSourceBuildCapabilityFiles = (
    std::fs::File,
    std::fs::File,
    std::fs::File,
    Vec<RuntimeSourceBuildComponentCapability>,
);

/// Exact retained filesystem objects needed by the controlled build runner.
pub struct RuntimeSourceBuildCapabilities {
    build_program: std::fs::File,
    isolation_helper: std::fs::File,
    component_root: std::fs::File,
    component_files: Vec<RuntimeSourceBuildComponentCapability>,
}

impl RuntimeSourceBuildCapabilities {
    /// Consumes the capability set into build program, isolation helper, and
    /// complete read-only component-root object, and every exact component file
    /// in canonical manifest order.
    #[must_use]
    pub fn into_files(self) -> RuntimeSourceBuildCapabilityFiles {
        (
            self.build_program,
            self.isolation_helper,
            self.component_root,
            self.component_files,
        )
    }
}

impl RuntimeSourceBuildBundleLease {
    pub(crate) fn overlaps_path(&self, path: &std::path::Path) -> bool {
        self.pinned.overlaps_path(path)
    }

    pub(crate) fn clone_component_for_evidence(
        &self,
        path: &rewrite_model::ArtifactSetRelativePath,
    ) -> Result<std::fs::File, RuntimeSourceBuildBundleError> {
        self.pinned.clone_component_capability(path)
    }

    /// Returns the exact canonical manifest bytes held by this lease.
    #[must_use]
    pub fn manifest_bytes(&self) -> &[u8] {
        self.pinned.manifest_bytes()
    }

    /// Returns the typed manifest whose complete component set was hashed.
    #[must_use]
    pub const fn inputs(&self) -> &VerifiedRuntimeSourceBuildInputs {
        &self.verified
    }

    /// Returns the content-free deterministic build handoff.
    #[must_use]
    pub const fn plan(&self) -> &RuntimeSourceBuildPlan {
        &self.plan
    }

    /// Rechecks every held and named boundary without releasing the lease.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildBundleError`] for cancellation or any source
    /// identity, type, link, size, or tree change.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeSourceBuildBundleError> {
        self.pinned.recheck(cancellation)
    }

    /// Clones the exact retained objects required for one controlled build.
    ///
    /// The result contains the build program selected by the verified plan, the
    /// exact isolation helper selected by that plan, and the complete verified
    /// input-root capability. No caller-local path is returned.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildBundleError`] for cancellation, object drift,
    /// or a retained descriptor-clone failure.
    pub fn build_capabilities(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildCapabilities, RuntimeSourceBuildBundleError> {
        self.revalidate(cancellation)?;
        let build_program = self
            .pinned
            .clone_component_capability(self.plan.build_program_path())?;
        let isolation_helper = self
            .pinned
            .clone_component_capability(self.plan.isolation_helper_path())?;
        let component_root = self.pinned.clone_component_root_capability()?;
        let cloned_components = self.pinned.clone_all_component_capabilities()?;
        if cloned_components.len() != self.verified.manifest().components().len() {
            return Err(RuntimeSourceBuildBundleError::SourceChanged);
        }
        let component_files = cloned_components
            .into_iter()
            .zip(self.verified.manifest().components())
            .map(|((path, file), component)| {
                (
                    path,
                    component.digest().clone(),
                    component.byte_size(),
                    file,
                )
            })
            .collect::<Vec<_>>();
        if component_files
            .iter()
            .zip(self.verified.manifest().components())
            .any(|((path, digest, bytes, _file), component)| {
                path != component.relative_path()
                    || digest != component.digest()
                    || *bytes != component.byte_size()
            })
        {
            return Err(RuntimeSourceBuildBundleError::SourceChanged);
        }
        self.revalidate(cancellation)?;
        Ok(RuntimeSourceBuildCapabilities {
            build_program,
            isolation_helper,
            component_root,
            component_files,
        })
    }

    fn into_verified(self) -> VerifiedRuntimeSourceBuildBundle {
        VerifiedRuntimeSourceBuildBundle::new(self.pinned.manifest_bytes().to_vec(), self.verified)
    }
}

/// Read-only verifier for one caller-selected controlled-build input bundle.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeSourceBuildBundleVerifier;

impl RuntimeSourceBuildBundleVerifier {
    /// Acquires a retained byte-verified lease over one exact offline bundle.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildBundleError`] for invalid limits, unsafe or
    /// changing filesystem boundaries, a non-exact tree, cancellation, or any
    /// controlled-build input contract failure.
    pub fn acquire(
        selection: &RuntimeSourceBuildBundleSource,
        limits: RuntimeSourceBuildBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildBundleLease, RuntimeSourceBuildBundleError> {
        let limits = limits.validate()?;
        let pinned = PinnedRuntimeSourceBuildBundle::open(selection, limits, cancellation)?;
        let verified = verify_runtime_source_build_inputs(
            pinned.manifest_bytes(),
            limits.inputs,
            |path| pinned.clone_component(path),
            || cancellation.is_cancelled(),
        )?;
        pinned.recheck(cancellation)?;
        let plan = RuntimeSourceBuildPlan::for_legacy_read_only_verification(verified.manifest());
        Ok(RuntimeSourceBuildBundleLease {
            pinned,
            verified,
            plan,
        })
    }

    /// Pins, hashes, and rechecks one exact offline controlled-build input bundle.
    ///
    /// The verifier performs no discovery, mutation, execution, or network
    /// access. A successful result proves only that the selected local bytes
    /// match the canonical frozen input manifest.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildBundleError`] for invalid limits, unsafe or
    /// changing filesystem boundaries, a non-exact tree, cancellation, or any
    /// controlled-build input contract failure.
    pub fn verify(
        selection: &RuntimeSourceBuildBundleSource,
        limits: RuntimeSourceBuildBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedRuntimeSourceBuildBundle, RuntimeSourceBuildBundleError> {
        Self::acquire(selection, limits, cancellation)
            .map(RuntimeSourceBuildBundleLease::into_verified)
    }
}

#[cfg(test)]
mod tests;
