//! CLI adapter for shared coherent regular-file input.

pub(crate) use rewrite_app::document_input::{
    is_changed, open_regular_file, read_direct_unaliased_bounded, read_directory_bounded,
    read_directory_unaliased_bounded, read_regular_bounded,
};

#[cfg(test)]
mod tests;
