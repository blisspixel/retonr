use super::{
    DocumentIntakeError, SidecarKind, SidecarObservation, SidecarScanCompleteness, ensure_active,
};
use rewrite_types::CancellationToken;
use std::{fs, io, path::Path};

pub(super) fn scan(
    source: Option<&Path>,
    cancellation: &CancellationToken,
) -> Result<SidecarObservation, DocumentIntakeError> {
    scan_with(source, cancellation, |path| fs::symlink_metadata(path))
}

fn scan_with(
    source: Option<&Path>,
    cancellation: &CancellationToken,
    mut metadata: impl FnMut(&Path) -> io::Result<fs::Metadata>,
) -> Result<SidecarObservation, DocumentIntakeError> {
    ensure_active(cancellation)?;
    let Some(source) = source else {
        return Ok(SidecarObservation {
            present: Vec::new(),
            completeness: SidecarScanCompleteness::NotApplicable,
        });
    };
    let mut observation = SidecarObservation {
        present: Vec::new(),
        completeness: SidecarScanCompleteness::Complete,
    };
    let (Some(parent), Some(name)) = (source.parent(), source.file_name()) else {
        observation.completeness = SidecarScanCompleteness::Incomplete;
        return Ok(observation);
    };
    for kind in [SidecarKind::C2pa, SidecarKind::Xmp] {
        ensure_active(cancellation)?;
        let mut adjacent = name.to_os_string();
        adjacent.push(kind.suffix());
        match metadata(&parent.join(adjacent)) {
            Ok(value) if value.is_file() && !value.file_type().is_symlink() => {
                observation.present.push(kind);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => observation.completeness = SidecarScanCompleteness::Incomplete,
        }
        ensure_active(cancellation)?;
    }
    Ok(observation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_and_io_failures_cannot_be_classified_as_complete_absence() {
        for kind in [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Interrupted,
            io::ErrorKind::Other,
        ] {
            let mut calls = 0;
            let result = scan_with(
                Some(Path::new("private-source.txt")),
                &CancellationToken::new(),
                |_| {
                    calls += 1;
                    Err(io::Error::from(kind))
                },
            )
            .expect("typed incomplete observation");
            assert_eq!(calls, 2);
            assert_eq!(result.completeness, SidecarScanCompleteness::Incomplete);
            assert!(result.present.is_empty());
        }
    }

    #[test]
    fn incomplete_scan_preserves_an_independently_detected_sidecar() {
        let root = tempfile::tempdir().expect("root");
        let regular = root.path().join("metadata");
        fs::write(&regular, b"metadata").expect("regular sidecar");
        let observed = fs::symlink_metadata(regular).expect("metadata observation");
        let result = scan_with(
            Some(Path::new("draft.txt")),
            &CancellationToken::new(),
            |path| {
                if path.extension().is_some_and(|suffix| suffix == "c2pa") {
                    Ok(observed.clone())
                } else {
                    Err(io::Error::from(io::ErrorKind::PermissionDenied))
                }
            },
        )
        .expect("partial observation");
        assert_eq!(result.present, vec![SidecarKind::C2pa]);
        assert_eq!(result.completeness, SidecarScanCompleteness::Incomplete);
    }

    #[test]
    fn absence_is_complete_but_original_cancellation_after_lookup_discards_it() {
        let token = CancellationToken::new();
        let absent = scan_with(Some(Path::new("draft.txt")), &token, |_| {
            Err(io::Error::from(io::ErrorKind::NotFound))
        })
        .expect("complete absence");
        assert_eq!(absent.completeness, SidecarScanCompleteness::Complete);
        let mut calls = 0;
        assert!(matches!(
            scan_with(Some(Path::new("draft.txt")), &token, |_| {
                calls += 1;
                token.cancel();
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }),
            Err(DocumentIntakeError::Cancelled)
        ));
        assert_eq!(calls, 1);
    }
}
