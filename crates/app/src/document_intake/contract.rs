use crate::PlainTextInventory;

/// Adjacent metadata formats recognized without decoding their contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidecarKind {
    /// An adjacent .c2pa regular file.
    C2pa,
    /// An adjacent .xmp regular file.
    Xmp,
}

impl SidecarKind {
    /// Returns the exact suffix appended to native source path bytes.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::C2pa => ".c2pa",
            Self::Xmp => ".xmp",
        }
    }
}

/// Whether all supported adjacent metadata lookups completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidecarScanCompleteness {
    /// Every supported adjacent path was inspected or was absent.
    Complete,
    /// The input is a supplied byte stream without a filesystem selection.
    NotApplicable,
    /// At least one lookup failed for a reason other than an absent path.
    Incomplete,
}

/// Bounded adjacent metadata observations, without display labels or contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidecarObservation {
    /// Direct regular sidecars found in supported suffix order.
    pub present: Vec<SidecarKind>,
    /// Completion state of the two supported lookups.
    pub completeness: SidecarScanCompleteness,
}

/// Decision boundary for a derivative, independent of encoding support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivativeDisposition {
    /// No supported carrier or adjacent sidecar requires a decision.
    NotRequired,
    /// Possible carrier, detected sidecar or incomplete scan requires a decision.
    ExplicitDecisionRequired,
    /// Unsupported encoding prevents a source-form decision.
    NotChecked,
}

/// Complete-input inventory and metadata facts, without serialized reports.
pub struct DocumentIntakeObservation {
    /// Inventory over all input bytes, including the typed carrier observation.
    pub inventory: PlainTextInventory,
    /// Adjacent sidecars were observed without opening or decoding them.
    pub sidecars: SidecarObservation,
    /// Required derivative decision under the supported plain-text policy.
    pub derivative: DerivativeDisposition,
}

impl std::fmt::Debug for DocumentIntakeObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentIntakeObservation")
            .field("byte_size", &self.inventory.byte_size)
            .field("sidecars", &self.sidecars)
            .field("derivative", &self.derivative)
            .finish_non_exhaustive()
    }
}

/// Complete read bytes with their shared inventory and metadata observations.
pub struct SelectedDocument {
    /// Exact input bytes before any preview clipping or editorial changes.
    pub bytes: Vec<u8>,
    /// Typed inventory and derivative policy observations.
    pub observation: DocumentIntakeObservation,
}

impl std::fmt::Debug for SelectedDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectedDocument")
            .field("byte_size", &self.bytes.len())
            .field("observation", &self.observation)
            .finish_non_exhaustive()
    }
}
