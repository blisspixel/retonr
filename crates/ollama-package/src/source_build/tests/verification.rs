use std::io::{Cursor, Read};

use super::*;

struct FailingRead;

impl Read for FailingRead {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("fixture read failure"))
    }
}

#[test]
fn every_frozen_component_byte_is_verified() {
    let bytes = canonical(&manifest_value());
    let verified = verify_runtime_source_build_inputs(
        &bytes,
        RuntimeSourceBuildInputLimits::default(),
        |path| Ok(Cursor::new(fixture_bytes(path.as_str()))),
        || false,
    )
    .expect("exact frozen closure verifies");
    assert_eq!(verified.manifest().components().len(), 16);
}

#[test]
fn missing_unreadable_changed_and_cancelled_components_fail_distinctly() {
    let bytes = canonical(&manifest_value());
    assert_eq!(
        verify_runtime_source_build_inputs(
            &bytes,
            RuntimeSourceBuildInputLimits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildInputOpenError> {
                Err(RuntimeSourceBuildInputOpenError)
            },
            || false
        ),
        Err(RuntimeSourceBuildInputError::ComponentUnavailable)
    );
    assert_eq!(
        verify_runtime_source_build_inputs(
            &bytes,
            RuntimeSourceBuildInputLimits::default(),
            |_path| Ok(FailingRead),
            || false
        ),
        Err(RuntimeSourceBuildInputError::ComponentRead)
    );
    for mode in 0..3 {
        let result = verify_runtime_source_build_inputs(
            &bytes,
            RuntimeSourceBuildInputLimits::default(),
            |path| {
                let mut component = fixture_bytes(path.as_str());
                if path.as_str() == "helper/isolation" {
                    match mode {
                        0 => {
                            component.pop();
                        }
                        1 => component.push(0),
                        2 => *component.last_mut().expect("nonempty component") ^= 1,
                        _ => unreachable!(),
                    }
                }
                Ok(Cursor::new(component))
            },
            || false,
        );
        assert_eq!(
            result,
            Err(match mode {
                0 | 1 => RuntimeSourceBuildInputError::ComponentSizeMismatch,
                2 => RuntimeSourceBuildInputError::ComponentDigestMismatch,
                _ => unreachable!(),
            })
        );
    }
    assert_eq!(
        verify_runtime_source_build_inputs(
            &bytes,
            RuntimeSourceBuildInputLimits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildInputOpenError> {
                panic!("cancellation must win before component open")
            },
            || true
        ),
        Err(RuntimeSourceBuildInputError::Cancelled)
    );
}
