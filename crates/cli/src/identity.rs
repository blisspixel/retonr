//! Shared product-identity facts for recovery commands.

use serde::Serialize;

use rewrite_app::ArtifactRepository;

use crate::contract::CLI_SCHEMA_VERSION;

/// Content-free product and machine-contract identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ProductIdentity {
    pub(crate) product: &'static str,
    pub(crate) product_version: &'static str,
    pub(crate) rust_version: &'static str,
    pub(crate) cli_schema_version: u32,
    pub(crate) store_schema_version: u32,
    pub(crate) local_only: bool,
}

impl ProductIdentity {
    pub(crate) fn current() -> Self {
        Self {
            product: "retonr",
            product_version: env!("CARGO_PKG_VERSION"),
            rust_version: env!("CARGO_PKG_RUST_VERSION"),
            cli_schema_version: CLI_SCHEMA_VERSION,
            store_schema_version: ArtifactRepository::required_schema_version(),
            local_only: true,
        }
    }

    pub(crate) fn text(&self) -> String {
        format!(
            "product: {}\nproduct_version: {}\nrust_version: {}\ncli_schema_version: {}\nstore_schema_version: {}\nlocal_only: {}\n",
            self.product,
            self.product_version,
            self.rust_version,
            self.cli_schema_version,
            self.store_schema_version,
            self.local_only
        )
    }
}

#[cfg(test)]
mod tests {
    use rewrite_app::ArtifactRepository;

    use super::ProductIdentity;

    #[test]
    fn identity_is_local_and_versioned() {
        let identity = ProductIdentity::current();
        assert_eq!(identity.product, "retonr");
        assert_eq!(identity.product_version, "0.1.0");
        assert_eq!(identity.rust_version, "1.97.1");
        assert_eq!(identity.cli_schema_version, 1);
        assert_eq!(
            identity.store_schema_version,
            ArtifactRepository::required_schema_version()
        );
        assert!(identity.local_only);
        assert!(identity.text().contains("local_only: true"));
    }
}
