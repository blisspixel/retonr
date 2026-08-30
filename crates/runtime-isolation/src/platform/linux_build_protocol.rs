use crate::{
    ControlledBuildOutput, ControlledBuildOutputTree, ControlledBuildProcessStatus,
    MAXIMUM_CONTROLLED_BUILD_INPUT_FILES,
};
use rewrite_types::Digest;

use super::{
    linux_helper::{decode_namespace_evidence, encode_namespace_evidence},
    linux_helper_setup::{HelperFailure, NamespaceEvidence, RawNamespaceIdentity},
    linux_startup,
};

const ARM_BYTES: usize = 68;
const BOOTSTRAP_ARM_BYTES: usize = ARM_BYTES + 1;
const RESULT_HEADER_BYTES: usize = 7;
const TREE_BYTES: usize = 80;
const INPUT_COUNT_BYTES: usize = 5;
const INPUT_PATH_HEADER_BYTES: usize = 2;
const INPUT_DECLARATION_SUFFIX_BYTES: usize = 8 + 64;
const MAXIMUM_INPUT_PATH_BYTES: usize = 4_096;
const HELPER_FAILURE_VERSION: u8 = 1;
pub(super) const BUILD_ROOT_DESCRIPTOR_COUNT: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InputDeclaration {
    pub(super) relative_path: String,
    pub(super) expected_digest: Digest,
    pub(super) expected_bytes: u64,
}

pub(super) fn encode_input_count(count: usize) -> Option<[u8; INPUT_COUNT_BYTES]> {
    if count == 0 || count > MAXIMUM_CONTROLLED_BUILD_INPUT_FILES {
        return None;
    }
    let mut payload = [0_u8; INPUT_COUNT_BYTES];
    payload[0] = 1;
    payload[1..].copy_from_slice(&u32::try_from(count).ok()?.to_be_bytes());
    Some(payload)
}

pub(super) fn decode_input_count(payload: &[u8]) -> Option<usize> {
    if payload.len() != INPUT_COUNT_BYTES || payload[0] != 1 {
        return None;
    }
    let count = usize::try_from(u32::from_be_bytes(payload[1..].try_into().ok()?)).ok()?;
    (count > 0 && count <= MAXIMUM_CONTROLLED_BUILD_INPUT_FILES).then_some(count)
}

pub(super) fn encode_input_declarations(declarations: &[(&str, &Digest, u64)]) -> Option<Vec<u8>> {
    if declarations.is_empty()
        || declarations.len() > super::linux_control::MAX_RECEIVED_DESCRIPTORS
    {
        return None;
    }
    let mut payload = Vec::new();
    payload.push(2);
    payload.push(u8::try_from(declarations.len()).ok()?);
    for (path, digest, bytes) in declarations {
        if !valid_input_path(path) {
            return None;
        }
        let path = path.as_bytes();
        payload.extend_from_slice(&u16::try_from(path.len()).ok()?.to_be_bytes());
        payload.extend_from_slice(path);
        payload.extend_from_slice(&bytes.to_be_bytes());
        payload.extend_from_slice(digest.as_str().as_bytes());
    }
    Some(payload)
}

pub(super) fn decode_input_declarations(payload: &[u8]) -> Option<Vec<InputDeclaration>> {
    if payload.len() < INPUT_PATH_HEADER_BYTES || payload[0] != 2 {
        return None;
    }
    let count = usize::from(payload[1]);
    if count == 0 || count > super::linux_control::MAX_RECEIVED_DESCRIPTORS {
        return None;
    }
    let mut offset = INPUT_PATH_HEADER_BYTES;
    let mut declarations = Vec::with_capacity(count);
    for _ in 0..count {
        let length = usize::from(u16::from_be_bytes(
            payload.get(offset..offset + 2)?.try_into().ok()?,
        ));
        offset = offset.checked_add(2)?;
        let end = offset.checked_add(length)?;
        let path = std::str::from_utf8(payload.get(offset..end)?).ok()?;
        if !valid_input_path(path) {
            return None;
        }
        offset = end;
        let suffix_end = offset.checked_add(INPUT_DECLARATION_SUFFIX_BYTES)?;
        let bytes = u64::from_be_bytes(payload.get(offset..offset + 8)?.try_into().ok()?);
        let digest_text = std::str::from_utf8(payload.get(offset + 8..suffix_end)?).ok()?;
        let expected_digest = Digest::from_sha256_hex(digest_text).ok()?;
        declarations.push(InputDeclaration {
            relative_path: path.to_owned(),
            expected_digest,
            expected_bytes: bytes,
        });
        offset = suffix_end;
    }
    (offset == payload.len()).then_some(declarations)
}

pub(super) fn encode_helper_failure(failure: HelperFailure) -> Vec<u8> {
    let mut payload = Vec::with_capacity(1 + failure.code().len());
    payload.push(HELPER_FAILURE_VERSION);
    payload.extend_from_slice(failure.code().as_bytes());
    payload
}

pub(super) fn decode_helper_failure(payload: &[u8]) -> Option<crate::IsolationError> {
    let (version, code) = payload.split_first()?;
    if *version != HELPER_FAILURE_VERSION {
        return None;
    }
    HelperFailure::from_code(code).map(HelperFailure::into_isolation_error)
}

fn valid_input_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_INPUT_PATH_BYTES
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        })
}

pub(super) fn encode_armed(
    evidence: NamespaceEvidence,
    mount: RawNamespaceIdentity,
    landlock_abi: u32,
) -> [u8; ARM_BYTES] {
    let mut payload = [0_u8; ARM_BYTES];
    payload[..48].copy_from_slice(&encode_namespace_evidence(evidence));
    payload[48..56].copy_from_slice(&mount.device.to_be_bytes());
    payload[56..64].copy_from_slice(&mount.inode.to_be_bytes());
    payload[64..].copy_from_slice(&landlock_abi.to_be_bytes());
    payload
}

pub(super) fn decode_armed(
    payload: &[u8],
) -> Option<(NamespaceEvidence, RawNamespaceIdentity, u32)> {
    if payload.len() != ARM_BYTES {
        return None;
    }
    let evidence = decode_namespace_evidence(&payload[..48]).ok()?;
    let mount = RawNamespaceIdentity {
        device: u64::from_be_bytes(payload[48..56].try_into().ok()?),
        inode: u64::from_be_bytes(payload[56..64].try_into().ok()?),
    };
    let abi = u32::from_be_bytes(payload[64..].try_into().ok()?);
    (abi >= 3 && mount.device > 0 && mount.inode > 0).then_some((evidence, mount, abi))
}

pub(super) fn encode_bootstrap_armed(
    evidence: NamespaceEvidence,
    mount: RawNamespaceIdentity,
    landlock_abi: u32,
) -> [u8; BOOTSTRAP_ARM_BYTES] {
    let mut payload = [0_u8; BOOTSTRAP_ARM_BYTES];
    payload[..ARM_BYTES].copy_from_slice(&encode_armed(evidence, mount, landlock_abi));
    payload[ARM_BYTES] = 1;
    payload
}

pub(super) fn encode_finished(output: &ControlledBuildOutput) -> Option<Vec<u8>> {
    let (kind, code) = match output.status() {
        ControlledBuildProcessStatus::Success => (0_u8, 0_i32),
        ControlledBuildProcessStatus::ExitCode(code) if code > 0 => (1_u8, code),
        ControlledBuildProcessStatus::Signal(signal) if signal > 0 => (2_u8, signal),
        ControlledBuildProcessStatus::ExitCode(_) | ControlledBuildProcessStatus::Signal(_) => {
            return None;
        }
    };
    let tree = match (output.status(), output.tree()) {
        (ControlledBuildProcessStatus::Success, Some(tree)) => Some(tree),
        (
            ControlledBuildProcessStatus::ExitCode(_) | ControlledBuildProcessStatus::Signal(_),
            None,
        ) => None,
        _ => return None,
    };
    let streams = linux_startup::encode(output.streams())?;
    let mut payload = Vec::with_capacity(
        RESULT_HEADER_BYTES
            .saturating_add(tree.map_or(0, |_| TREE_BYTES))
            .saturating_add(streams.len()),
    );
    payload.push(2);
    payload.push(kind);
    payload.extend_from_slice(&code.to_be_bytes());
    payload.push(u8::from(tree.is_some()));
    if let Some(tree) = tree {
        payload.extend_from_slice(tree.digest().as_str().as_bytes());
        payload.extend_from_slice(&tree.entry_count().to_be_bytes());
        payload.extend_from_slice(&tree.regular_file_count().to_be_bytes());
        payload.extend_from_slice(&tree.total_file_bytes().to_be_bytes());
    }
    payload.extend_from_slice(&streams);
    Some(payload)
}

pub(super) fn encode_process_finished(output: &ControlledBuildOutput) -> Option<Vec<u8>> {
    if output.tree().is_some() {
        return None;
    }
    let (kind, code) = status_fields(output.status())?;
    let streams = linux_startup::encode(output.streams())?;
    let mut payload = Vec::with_capacity(6_usize.saturating_add(streams.len()));
    payload.push(1);
    payload.push(kind);
    payload.extend_from_slice(&code.to_be_bytes());
    payload.extend_from_slice(&streams);
    Some(payload)
}

pub(super) fn decode_process_finished(payload: &[u8]) -> Option<ControlledBuildOutput> {
    if payload.len() < 6 || payload[0] != 1 {
        return None;
    }
    let code = i32::from_be_bytes(payload[2..6].try_into().ok()?);
    let status = decode_status(payload[1], code)?;
    let streams = linux_startup::decode(&payload[6..])?;
    Some(ControlledBuildOutput::new(status, streams))
}

pub(super) fn decode_finished(payload: &[u8]) -> Option<ControlledBuildOutput> {
    if payload.len() < RESULT_HEADER_BYTES || payload[0] != 2 {
        return None;
    }
    let code = i32::from_be_bytes(payload[2..6].try_into().ok()?);
    let status = decode_status(payload[1], code)?;
    let has_tree = match payload[6] {
        0 => false,
        1 => true,
        _ => return None,
    };
    if has_tree != matches!(status, ControlledBuildProcessStatus::Success) {
        return None;
    }
    let (tree, stream_offset) = if has_tree {
        if payload.len() < RESULT_HEADER_BYTES + TREE_BYTES {
            return None;
        }
        let bytes = &payload[RESULT_HEADER_BYTES..RESULT_HEADER_BYTES + TREE_BYTES];
        let digest = Digest::from_sha256_hex(std::str::from_utf8(&bytes[..64]).ok()?).ok()?;
        let entry_count = u32::from_be_bytes(bytes[64..68].try_into().ok()?);
        let regular_file_count = u32::from_be_bytes(bytes[68..72].try_into().ok()?);
        let total_file_bytes = u64::from_be_bytes(bytes[72..80].try_into().ok()?);
        let tree = ControlledBuildOutputTree::from_parts(
            digest,
            entry_count,
            regular_file_count,
            total_file_bytes,
        )
        .ok()?;
        (Some(tree), RESULT_HEADER_BYTES + TREE_BYTES)
    } else {
        (None, RESULT_HEADER_BYTES)
    };
    let streams = linux_startup::decode(&payload[stream_offset..])?;
    let output = ControlledBuildOutput::new(status, streams);
    Some(match tree {
        Some(tree) => output.with_tree(tree),
        None => output,
    })
}

fn status_fields(status: ControlledBuildProcessStatus) -> Option<(u8, i32)> {
    match status {
        ControlledBuildProcessStatus::Success => Some((0, 0)),
        ControlledBuildProcessStatus::ExitCode(code) if code > 0 => Some((1, code)),
        ControlledBuildProcessStatus::Signal(signal) if signal > 0 => Some((2, signal)),
        ControlledBuildProcessStatus::ExitCode(_) | ControlledBuildProcessStatus::Signal(_) => None,
    }
}

fn decode_status(kind: u8, code: i32) -> Option<ControlledBuildProcessStatus> {
    match (kind, code) {
        (0, 0) => Some(ControlledBuildProcessStatus::Success),
        (1, code) if code > 0 => Some(ControlledBuildProcessStatus::ExitCode(code)),
        (2, signal) if signal > 0 => Some(ControlledBuildProcessStatus::Signal(signal)),
        _ => None,
    }
}

pub(super) fn require_armed(
    payload: &[u8],
) -> Result<(NamespaceEvidence, RawNamespaceIdentity, u32), HelperFailure> {
    decode_armed(payload).ok_or(HelperFailure::InvalidLaunch)
}

pub(super) fn require_bootstrap_armed(
    payload: &[u8],
) -> Result<(NamespaceEvidence, RawNamespaceIdentity, u32), HelperFailure> {
    if payload.len() != BOOTSTRAP_ARM_BYTES || payload[ARM_BYTES] != 1 {
        return Err(HelperFailure::InvalidLaunch);
    }
    require_armed(&payload[..ARM_BYTES])
}

#[cfg(test)]
mod tests {
    use crate::{
        ControlledBuildOutput, ControlledBuildOutputTree, ControlledBuildOutputTreeEntry,
        ControlledBuildProcessStatus, IsolationError, ManagedStartupOutput,
    };
    use rewrite_types::Digest;

    use super::*;
    use crate::NamespaceIdentity;

    #[test]
    fn armed_and_finished_protocols_are_exact() {
        let evidence = NamespaceEvidence {
            network: super::super::linux_helper_setup::RawNamespaceIdentity {
                device: 1,
                inode: 2,
            },
            user: super::super::linux_helper_setup::RawNamespaceIdentity {
                device: 3,
                inode: 4,
            },
            process: super::super::linux_helper_setup::RawNamespaceIdentity {
                device: 5,
                inode: 6,
            },
        };
        assert_eq!(
            decode_armed(&encode_armed(
                evidence,
                RawNamespaceIdentity {
                    device: 7,
                    inode: 8,
                },
                3
            )),
            Some((
                evidence,
                RawNamespaceIdentity {
                    device: 7,
                    inode: 8
                },
                3
            ))
        );
        assert!(
            decode_armed(&encode_armed(
                evidence,
                RawNamespaceIdentity {
                    device: 7,
                    inode: 8,
                },
                2
            ))
            .is_none()
        );
        let mut bootstrap = encode_bootstrap_armed(
            evidence,
            RawNamespaceIdentity {
                device: 7,
                inode: 8,
            },
            3,
        );
        assert_eq!(
            require_bootstrap_armed(&bootstrap),
            Ok((
                evidence,
                RawNamespaceIdentity {
                    device: 7,
                    inode: 8,
                },
                3,
            ))
        );
        bootstrap[ARM_BYTES] = 0;
        assert_eq!(
            require_bootstrap_armed(&bootstrap),
            Err(HelperFailure::InvalidLaunch)
        );
        let output = ControlledBuildOutput::new(
            ControlledBuildProcessStatus::ExitCode(7),
            ManagedStartupOutput::new(b"out".to_vec(), b"err".to_vec(), false, true),
        );
        let encoded = encode_finished(&output).expect("encode");
        assert_eq!(decode_finished(&encoded), Some(output.clone()));
        let encoded = encode_process_finished(&output).expect("encode process result");
        assert_eq!(decode_process_finished(&encoded), Some(output));
        let tree = ControlledBuildOutputTree::compile(vec![
            ControlledBuildOutputTreeEntry::directory("bin", 0o755).expect("directory"),
            ControlledBuildOutputTreeEntry::regular_file(
                "bin/tool",
                4,
                Digest::sha256(b"tool"),
                0o755,
            )
            .expect("file"),
        ])
        .expect("tree");
        let success = ControlledBuildOutput::new(
            ControlledBuildProcessStatus::Success,
            ManagedStartupOutput::new(Vec::new(), Vec::new(), false, false),
        )
        .with_tree(tree);
        let encoded = encode_finished(&success).expect("encode success");
        assert_eq!(decode_finished(&encoded), Some(success));
        assert!(matches!(
            ControlledBuildOutputTree::from_parts(Digest::sha256(b"tree"), 0, 0, 0),
            Err(IsolationError::ControlledBuildObjectMismatch)
        ));
        let _ = NamespaceIdentity::new(1, 2);
    }

    #[test]
    fn retained_input_protocol_is_bounded_and_exact() {
        assert_eq!(BUILD_ROOT_DESCRIPTOR_COUNT, 2);
        let count = encode_input_count(4).expect("count");
        assert_eq!(decode_input_count(&count), Some(4));
        assert!(encode_input_count(0).is_none());
        assert!(encode_input_count(MAXIMUM_CONTROLLED_BUILD_INPUT_FILES + 1).is_none());

        let first = Digest::sha256(b"build");
        let second = Digest::sha256(b"source");
        let encoded = encode_input_declarations(&[
            ("scripts/build", &first, 5),
            ("sources/source.tar", &second, 6),
        ])
        .expect("input declarations");
        assert_eq!(
            decode_input_declarations(&encoded),
            Some(vec![
                InputDeclaration {
                    relative_path: "scripts/build".to_owned(),
                    expected_digest: first,
                    expected_bytes: 5,
                },
                InputDeclaration {
                    relative_path: "sources/source.tar".to_owned(),
                    expected_digest: second,
                    expected_bytes: 6,
                },
            ])
        );
        assert!(encode_input_declarations(&[("../escape", &Digest::sha256(b""), 0)]).is_none());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_input_declarations(&trailing).is_none());
    }

    #[test]
    fn late_helper_failure_protocol_is_versioned_and_exact() {
        for failure in HelperFailure::ALL {
            let encoded = encode_helper_failure(failure);
            assert_eq!(
                decode_helper_failure(&encoded),
                Some(failure.into_isolation_error())
            );
        }
        assert!(decode_helper_failure(&[]).is_none());
        assert!(decode_helper_failure(&[2, b'i', b'n', b'v']).is_none());
        assert!(decode_helper_failure(&[1, b'u', b'n', b'k']).is_none());
        let mut trailing = encode_helper_failure(HelperFailure::ControlledBuildObjectMismatch);
        trailing.push(0);
        assert!(decode_helper_failure(&trailing).is_none());
    }
}
