use crate::fingerprint::Fingerprint;

/// Describes how much re-analysis a changed file requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeClass {
    /// No change detected — reuse cached analysis result.
    Skip,
    /// Body content changed but public interface is the same.
    PartialUpdate,
    /// Exports or imports changed — re-analyze this node and all edges from it.
    ArchitectureUpdate,
    /// No prior fingerprint exists or file was deleted — full analysis required.
    FullUpdate,
}

/// Classify the change between an old and new fingerprint.
/// Pass `None` for `old` when no prior fingerprint exists.
pub fn classify_change(old: Option<&Fingerprint>, new: &Fingerprint) -> ChangeClass {
    let Some(old) = old else {
        return ChangeClass::FullUpdate;
    };
    if old.content_matches(new) {
        return ChangeClass::Skip;
    }
    if old.signature_matches(new) {
        return ChangeClass::PartialUpdate;
    }
    ChangeClass::ArchitectureUpdate
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn fp(sig: &str, content: &str) -> Fingerprint {
        Fingerprint {
            path: "test.rs".into(),
            signature_hash: sig.into(),
            content_hash: content.into(),
            exports: BTreeSet::new(),
            imports: BTreeSet::new(),
        }
    }

    #[test]
    fn no_prior_is_full_update() {
        assert_eq!(
            classify_change(None, &fp("s1", "c1")),
            ChangeClass::FullUpdate
        );
    }

    #[test]
    fn identical_is_skip() {
        let f = fp("s1", "c1");
        assert_eq!(
            classify_change(Some(&f), &fp("s1", "c1")),
            ChangeClass::Skip
        );
    }

    #[test]
    fn same_signature_different_content_is_partial() {
        assert_eq!(
            classify_change(Some(&fp("s1", "c1")), &fp("s1", "c2")),
            ChangeClass::PartialUpdate
        );
    }

    #[test]
    fn different_signature_is_arch_update() {
        assert_eq!(
            classify_change(Some(&fp("s1", "c1")), &fp("s2", "c2")),
            ChangeClass::ArchitectureUpdate
        );
    }
}
