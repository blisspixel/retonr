use rewrite_inference::{InferenceError, OperationContext};

use super::OllamaBackend;
use crate::{
    contract::{OllamaPreflight, OllamaPreflightBinding},
    response::{
        compatibility_error, confirm_inventory_digest, parse_ollama_inventory, policy_error,
    },
};

impl OllamaBackend {
    /// Captures coherent read-only runtime, inventory, model-description, and residency evidence.
    ///
    /// The preflight performs no generation, acquisition, activation, or model load. Runtime,
    /// inventory, and residency state are sampled before and after model inspection so drift fails
    /// closed.
    ///
    /// # Errors
    ///
    /// Returns an error when any configured target is absent or changed, a response is invalid,
    /// or runtime, inventory, or residency identity changes during inspection.
    pub async fn preflight(
        &self,
        context: OperationContext<'_>,
    ) -> Result<OllamaPreflight, InferenceError> {
        let _permit = self.operation_permit(context).await?;
        super::check_context(context)?;
        if self.preflight_targets.is_empty() {
            return Err(policy_error("preflight_not_configured"));
        }
        let runtime_before = self.runtime_identity(context).await?;
        let tags_before = self.tags(context).await?;
        let mut inventory = parse_ollama_inventory(&tags_before)?;
        inventory.sort_unstable_by(|left, right| left.reference.cmp(&right.reference));
        let running_before = self.running_models(context).await?;
        let mut bindings = Vec::with_capacity(self.preflight_targets.len());
        for target in &self.preflight_targets {
            confirm_inventory_digest(target.reference(), target.inventory_digest(), &tags_before)?;
            bindings.push(OllamaPreflightBinding {
                reference: target.reference.clone(),
                inventory_digest: target.inventory_digest.clone(),
                details: self.show_details(target.reference(), context).await?,
            });
        }
        bindings.sort_unstable_by(|left, right| left.reference.cmp(&right.reference));
        let tags_after = self.tags(context).await?;
        let runtime_after = self.runtime_identity(context).await?;
        let running_after = self.running_models(context).await?;
        if runtime_before != runtime_after
            || tags_before != tags_after
            || running_before != running_after
        {
            return Err(compatibility_error("runtime_changed_during_preflight"));
        }
        for target in &self.preflight_targets {
            confirm_inventory_digest(target.reference(), target.inventory_digest(), &tags_after)?;
        }
        Ok(OllamaPreflight {
            runtime: runtime_after,
            inventory,
            bindings,
            running: running_after,
        })
    }
}
