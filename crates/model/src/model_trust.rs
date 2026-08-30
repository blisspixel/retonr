//! Inert identities shared by model-package trust boundaries.

use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! owner_derived_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(Digest);

        impl $name {
            /// Wraps a digest derived by the owning verification boundary.
            ///
            /// The returned value is inert. It grants no verification, license,
            /// launch, qualification, or live-use authority.
            #[must_use]
            pub const fn from_derived_digest(digest: Digest) -> Self {
                Self(digest)
            }

            /// Returns the digest defining this inert identity.
            #[must_use]
            pub const fn digest(&self) -> &Digest {
                &self.0
            }
        }
    };
}

owner_derived_id!(
    ModelPackageFoundationId,
    "Stable identity derived by the model-package foundation verifier."
);
owner_derived_id!(
    ModelLicenseControlId,
    "Identity derived from one canonical portable model-license control."
);
