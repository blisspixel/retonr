//! Supported command-line editorial freedom levels.

/// Supported CLI edit levels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum EditLevelArg {
    /// Surface cleanup, grammar, punctuation, and mechanical correction.
    TouchUp,
    /// Style, tone, and register tuning while strictly preserving layout and length bounds.
    VoicePass,
    /// Sentence-level and paragraph-level rewrites preserving all factual claims.
    Rewrite,
    /// Structural reformulations retaining core invariants and claims.
    Reconstruct,
}

impl From<EditLevelArg> for rewrite_types::EditLevel {
    fn from(arg: EditLevelArg) -> Self {
        match arg {
            EditLevelArg::TouchUp => Self::TouchUp,
            EditLevelArg::VoicePass => Self::VoicePass,
            EditLevelArg::Rewrite => Self::Rewrite,
            EditLevelArg::Reconstruct => Self::Reconstruct,
        }
    }
}
