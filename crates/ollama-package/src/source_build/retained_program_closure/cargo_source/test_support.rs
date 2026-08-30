use rewrite_types::Digest;

use super::{CargoSourceClosureReviewerFacts, VerifiedCargoSourceClosure};

pub(in crate::source_build::retained_program_closure) fn verified_cargo_source_closure_for_test(
    source_input_set_id: Digest,
    cargo_lock_digest: Digest,
    closure_id: Digest,
) -> VerifiedCargoSourceClosure {
    VerifiedCargoSourceClosure {
        cargo_lock_digest,
        closure_id,
        facts: CargoSourceClosureReviewerFacts {
            edges: 0,
            path_packages: 0,
            registry_packages: 0,
            source_files: 0,
        },
        raw_crate_archive_id: Digest::sha256(b"test raw crate archive"),
        raw_crate_tree_id: Digest::sha256(b"test raw crate tree"),
        repository_archive_id: Digest::sha256(b"test repository archive"),
        repository_tree_id: Digest::sha256(b"test repository tree"),
        source_input_set_id,
        vendor_archive_id: Digest::sha256(b"test vendor archive"),
        vendor_tree_id: Digest::sha256(b"test vendor tree"),
    }
}
