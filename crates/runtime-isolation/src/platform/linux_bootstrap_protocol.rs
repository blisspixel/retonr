use rewrite_types::Digest;

use crate::{
    MAXIMUM_CONTROLLED_BUILD_INPUT_FILES, RetainedProgramBootstrapAttempt,
    RetainedProgramBootstrapInputKind, RetainedProgramBootstrapLaunchSpec,
};

use super::linux_build_protocol::InputDeclaration;

const VERSION: u8 = 1;
const DIGEST_BYTES: usize = 64;
const FIXED_HEADER_BYTES: usize =
    1 + 1 + 4 + DIGEST_BYTES + 8 + DIGEST_BYTES + 8 + DIGEST_BYTES + 1;
const SIGNED_INPUT_BYTES: usize = 1 + 8 + DIGEST_BYTES;
pub(super) const BOOTSTRAP_ROOT_DESCRIPTOR_COUNT: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BootstrapSignedInput {
    pub(super) kind: RetainedProgramBootstrapInputKind,
    pub(super) digest: Digest,
    pub(super) byte_size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BootstrapExecutable {
    pub(super) digest: Digest,
    pub(super) byte_size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BootstrapRequest {
    pub(super) attempt: RetainedProgramBootstrapAttempt,
    pub(super) input_count: usize,
    pub(super) launch_digest: Digest,
    pub(super) recipe_digest: Digest,
    pub(super) recipe_bytes: u64,
    pub(super) busybox_executable: BootstrapExecutable,
    pub(super) signed_inputs: Vec<BootstrapSignedInput>,
}

impl BootstrapRequest {
    pub(super) fn joins(&self, declarations: &[InputDeclaration]) -> bool {
        declarations.len() == self.input_count
            && declarations.iter().any(|declaration| {
                declaration.relative_path == "lineage/build-recipe-v2.json"
                    && declaration.expected_digest == self.recipe_digest
                    && declaration.expected_bytes == self.recipe_bytes
            })
            && self.signed_inputs.iter().all(|expected| {
                declarations.iter().any(|declaration| {
                    declaration.relative_path == expected.kind.relative_path()
                        && declaration.expected_digest == expected.digest
                        && declaration.expected_bytes == expected.byte_size
                })
            })
            && declarations.iter().any(|declaration| {
                declaration.relative_path == "toolchains/busybox"
                    && declaration.expected_digest == self.busybox_executable.digest
                    && declaration.expected_bytes == self.busybox_executable.byte_size
            })
    }

    pub(super) fn busybox_package(&self) -> Option<&BootstrapSignedInput> {
        self.signed_inputs.iter().find(|input| {
            input.kind == RetainedProgramBootstrapInputKind::AlpineBusyboxStaticPackage
        })
    }

    pub(super) const fn busybox_executable(&self) -> &BootstrapExecutable {
        &self.busybox_executable
    }
}

pub(super) fn encode(
    specification: &RetainedProgramBootstrapLaunchSpec,
    input_count: usize,
) -> Option<Vec<u8>> {
    if input_count == 0 || input_count > MAXIMUM_CONTROLLED_BUILD_INPUT_FILES {
        return None;
    }
    let inputs = specification.signed_inputs().measurements();
    if inputs.len() != RetainedProgramBootstrapInputKind::ALL.len() {
        return None;
    }
    let mut payload = Vec::with_capacity(
        FIXED_HEADER_BYTES.saturating_add(inputs.len().saturating_mul(SIGNED_INPUT_BYTES)),
    );
    payload.push(VERSION);
    payload.push(specification.attempt().code());
    payload.extend_from_slice(&u32::try_from(input_count).ok()?.to_be_bytes());
    payload.extend_from_slice(specification.redacted_digest().as_str().as_bytes());
    payload.extend_from_slice(&specification.recipe_bytes().to_be_bytes());
    payload.extend_from_slice(specification.recipe_digest().as_str().as_bytes());
    payload.extend_from_slice(&specification.busybox_executable().byte_size().to_be_bytes());
    payload.extend_from_slice(
        specification
            .busybox_executable()
            .digest()
            .as_str()
            .as_bytes(),
    );
    payload.push(u8::try_from(inputs.len()).ok()?);
    for input in inputs {
        payload.push(input.kind().code());
        payload.extend_from_slice(&input.byte_size().to_be_bytes());
        payload.extend_from_slice(input.digest().as_str().as_bytes());
    }
    Some(payload)
}

pub(super) fn decode(payload: &[u8]) -> Option<BootstrapRequest> {
    let expected = FIXED_HEADER_BYTES.checked_add(
        RetainedProgramBootstrapInputKind::ALL
            .len()
            .checked_mul(SIGNED_INPUT_BYTES)?,
    )?;
    if payload.len() != expected || payload.first().copied()? != VERSION {
        return None;
    }
    let attempt = RetainedProgramBootstrapAttempt::from_code(payload[1])?;
    let input_count = usize::try_from(u32::from_be_bytes(payload[2..6].try_into().ok()?)).ok()?;
    if input_count == 0 || input_count > MAXIMUM_CONTROLLED_BUILD_INPUT_FILES {
        return None;
    }
    let launch_digest = parse_digest(&payload[6..70])?;
    let recipe_bytes = u64::from_be_bytes(payload[70..78].try_into().ok()?);
    if recipe_bytes == 0 {
        return None;
    }
    let recipe_digest = parse_digest(&payload[78..142])?;
    let busybox_bytes = u64::from_be_bytes(payload[142..150].try_into().ok()?);
    if busybox_bytes == 0 {
        return None;
    }
    let busybox_digest = parse_digest(&payload[150..214])?;
    if usize::from(payload[214]) != RetainedProgramBootstrapInputKind::ALL.len() {
        return None;
    }
    let mut signed_inputs = Vec::with_capacity(RetainedProgramBootstrapInputKind::ALL.len());
    let mut offset = FIXED_HEADER_BYTES;
    for expected_kind in RetainedProgramBootstrapInputKind::ALL {
        let kind = RetainedProgramBootstrapInputKind::from_code(payload[offset])?;
        let byte_size = u64::from_be_bytes(payload[offset + 1..offset + 9].try_into().ok()?);
        let digest = parse_digest(&payload[offset + 9..offset + SIGNED_INPUT_BYTES])?;
        if kind != expected_kind || byte_size == 0 {
            return None;
        }
        signed_inputs.push(BootstrapSignedInput {
            kind,
            digest,
            byte_size,
        });
        offset += SIGNED_INPUT_BYTES;
    }
    Some(BootstrapRequest {
        attempt,
        input_count,
        launch_digest,
        recipe_digest,
        recipe_bytes,
        busybox_executable: BootstrapExecutable {
            digest: busybox_digest,
            byte_size: busybox_bytes,
        },
        signed_inputs,
    })
}

fn parse_digest(bytes: &[u8]) -> Option<Digest> {
    Digest::from_sha256_hex(String::from_utf8(bytes.to_vec()).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        AuthenticatedBusyboxExecutable, ControlledBuildLaunchSpec,
        RetainedProgramBootstrapInputMeasurement, RetainedProgramBootstrapSignedInputs,
    };

    use super::*;

    fn specification() -> RetainedProgramBootstrapLaunchSpec {
        let inputs = RetainedProgramBootstrapInputKind::ALL
            .into_iter()
            .map(|kind| {
                RetainedProgramBootstrapInputMeasurement::new(
                    kind,
                    Digest::sha256(kind.relative_path().as_bytes()),
                    1,
                )
                .expect("measurement")
            })
            .collect();
        RetainedProgramBootstrapLaunchSpec::new(
            RetainedProgramBootstrapAttempt::Primary,
            Digest::sha256(b"recipe"),
            6,
            RetainedProgramBootstrapSignedInputs::new(inputs).expect("inputs"),
            AuthenticatedBusyboxExecutable::new(Digest::sha256(b"busybox"), 7).expect("busybox"),
            ControlledBuildLaunchSpec::new(
                "bin/builder".to_owned(),
                Digest::sha256(b"builder"),
                7,
                Duration::from_secs(1),
            )
            .expect("build"),
        )
        .expect("bootstrap")
    }

    #[test]
    fn request_is_exact_versioned_and_binds_authenticated_busybox() {
        let specification = specification();
        let encoded = encode(&specification, 15).expect("encoded");
        let decoded = decode(&encoded).expect("decoded");
        assert_eq!(decoded.attempt, RetainedProgramBootstrapAttempt::Primary);
        assert_eq!(decoded.input_count, 15);
        assert_eq!(decoded.launch_digest, specification.redacted_digest());
        assert_eq!(
            decoded
                .busybox_package()
                .expect("busybox package")
                .kind
                .relative_path(),
            "lineage/host/alpine/busybox-static-1.37.0-r30.apk"
        );
        assert_eq!(
            decoded.busybox_executable(),
            &BootstrapExecutable {
                digest: Digest::sha256(b"busybox"),
                byte_size: 7,
            }
        );
        assert!(decode(&encoded[..encoded.len() - 1]).is_none());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode(&trailing).is_none());
    }

    #[test]
    fn request_rejects_role_reordering_and_invalid_counts() {
        let specification = specification();
        assert!(encode(&specification, 0).is_none());
        assert!(encode(&specification, MAXIMUM_CONTROLLED_BUILD_INPUT_FILES + 1).is_none());
        let mut encoded = encode(&specification, 15).expect("encoded");
        encoded[FIXED_HEADER_BYTES] = 2;
        assert!(decode(&encoded).is_none());
    }

    #[test]
    fn declaration_join_requires_every_exact_measurement() {
        let specification = specification();
        let request = decode(&encode(&specification, 15).expect("encoded")).expect("decoded");
        let mut declarations = vec![InputDeclaration {
            relative_path: "lineage/build-recipe-v2.json".to_owned(),
            expected_digest: specification.recipe_digest().clone(),
            expected_bytes: specification.recipe_bytes(),
        }];
        declarations.extend(request.signed_inputs.iter().map(|input| InputDeclaration {
            relative_path: input.kind.relative_path().to_owned(),
            expected_digest: input.digest.clone(),
            expected_bytes: input.byte_size,
        }));
        declarations.push(InputDeclaration {
            relative_path: "toolchains/busybox".to_owned(),
            expected_digest: request.busybox_executable.digest.clone(),
            expected_bytes: request.busybox_executable.byte_size,
        });
        assert!(request.joins(&declarations));
        declarations[5].expected_bytes = 2;
        assert!(!request.joins(&declarations));
    }
}
