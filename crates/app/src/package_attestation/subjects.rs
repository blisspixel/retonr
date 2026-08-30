use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct RuntimePackageIdentityToken(Arc<()>);

impl RuntimePackageIdentityToken {
    pub(super) fn new() -> Self {
        Self(Arc::new(()))
    }

    pub(super) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone)]
pub(crate) struct ModelPackageIdentityToken(Arc<()>);

impl ModelPackageIdentityToken {
    pub(super) fn new() -> Self {
        Self(Arc::new(()))
    }

    pub(super) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone)]
pub(crate) struct ManagedOllamaLiveSubjectToken(Arc<()>);

impl ManagedOllamaLiveSubjectToken {
    fn new() -> Self {
        Self(Arc::new(()))
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

pub(crate) struct ManagedOllamaSubjectBinding {
    live: ManagedOllamaLiveSubjectToken,
    runtime: RuntimePackageIdentityToken,
    model: ModelPackageIdentityToken,
}

impl ManagedOllamaSubjectBinding {
    pub(crate) fn new(
        runtime: RuntimePackageIdentityToken,
        model: ModelPackageIdentityToken,
    ) -> Self {
        Self {
            live: ManagedOllamaLiveSubjectToken::new(),
            runtime,
            model,
        }
    }

    pub(crate) fn live_token(&self) -> ManagedOllamaLiveSubjectToken {
        self.live.clone()
    }

    pub(crate) fn runtime_token(&self) -> RuntimePackageIdentityToken {
        self.runtime.clone()
    }

    pub(crate) fn binds_live(&self, token: &ManagedOllamaLiveSubjectToken) -> bool {
        self.live.ptr_eq(token)
    }

    pub(crate) const fn runtime_token_ref(&self) -> &RuntimePackageIdentityToken {
        &self.runtime
    }

    pub(crate) fn model_token(&self) -> ModelPackageIdentityToken {
        self.model.clone()
    }

    pub(crate) const fn model_token_ref(&self) -> &ModelPackageIdentityToken {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ManagedOllamaSubjectBinding, ModelPackageIdentityToken, RuntimePackageIdentityToken,
    };

    #[test]
    fn tokens_preserve_capability_identity_without_portable_material() {
        let runtime = RuntimePackageIdentityToken::new();
        let same_runtime = runtime.clone();
        let other_runtime = RuntimePackageIdentityToken::new();
        let model = ModelPackageIdentityToken::new();
        let same_model = model.clone();
        let other_model = ModelPackageIdentityToken::new();
        assert!(runtime.ptr_eq(&same_runtime));
        assert!(!runtime.ptr_eq(&other_runtime));

        let binding = ManagedOllamaSubjectBinding::new(runtime, model);
        let live = binding.live_token();
        assert!(binding.binds_live(&live));
        assert!(binding.runtime_token().ptr_eq(&same_runtime));
        assert!(binding.runtime_token_ref().ptr_eq(&same_runtime));
        assert!(binding.model_token().ptr_eq(&same_model));

        let other = ManagedOllamaSubjectBinding::new(other_runtime, other_model);
        assert!(!other.binds_live(&live));
        assert!(!other.runtime_token().ptr_eq(&same_runtime));
        assert!(!other.model_token().ptr_eq(&same_model));
    }
}
