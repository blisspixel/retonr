//! Process-local provenance for one Active qualification operation.

use std::sync::Arc;

/// Noncloneable root identity owned by exactly one Active operation.
pub(crate) struct ActiveGenerationQualificationSubject {
    marker: Arc<()>,
}

impl ActiveGenerationQualificationSubject {
    pub(crate) fn new() -> Self {
        Self {
            marker: Arc::new(()),
        }
    }

    pub(crate) fn binding(&self) -> ActiveGenerationQualificationBinding {
        ActiveGenerationQualificationBinding {
            marker: Arc::clone(&self.marker),
        }
    }

    pub(crate) fn accepts(&self, binding: &ActiveGenerationQualificationBinding) -> bool {
        Arc::ptr_eq(&self.marker, &binding.marker)
    }
}

/// Internally copyable binding carried by authorities derived from one Active root.
#[derive(Clone)]
pub(crate) struct ActiveGenerationQualificationBinding {
    marker: Arc<()>,
}

impl ActiveGenerationQualificationBinding {
    pub(crate) fn same_subject(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.marker, &other.marker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_match_only_their_exact_process_local_subject() {
        let first = ActiveGenerationQualificationSubject::new();
        let second = ActiveGenerationQualificationSubject::new();
        let binding = first.binding();
        let copied = binding.clone();

        assert!(first.accepts(&binding));
        assert!(binding.same_subject(&copied));
        assert!(!second.accepts(&binding));
        assert!(!second.binding().same_subject(&binding));
    }
}
