//! Supported command-line editorial freedom levels.

/// Supported CLI edit levels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum EditLevelArg {
    /// Punctuation and capitalization. This is the only level the check evaluator enforces.
    TouchUp,
    /// Not yet enforced. Check abstains on any wording change and reports touch-up as effective.
    VoicePass,
    /// Not yet enforced. Check abstains on any wording change and reports touch-up as effective.
    Rewrite,
    /// Not yet enforced. Check abstains on any wording change and reports touch-up as effective.
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
