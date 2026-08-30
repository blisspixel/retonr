use super::*;

const KEY: &[u8] = include_bytes!("fixtures/key.asc");
const SIGNATURE: &[u8] = include_bytes!("fixtures/signature.asc");
const PAYLOAD: &[u8] = b"deterministic retained-program upstream fixture";
const EXPECTED: SignatureExpectation<'static> = SignatureExpectation {
    fingerprint: "F58ED2E654CE90943155556386C0A905F115EC05",
    key_id: "86C0A905F115EC05",
    created: 1_700_000_000,
    issuer_fingerprint_count: 1,
    key_armor_header: None,
};

#[test]
fn deterministic_signature_verifies_and_binds_every_input() {
    assert!(verify_detached_signature(KEY, SIGNATURE, PAYLOAD, EXPECTED).is_ok());
    let mut changed_payload = PAYLOAD.to_vec();
    changed_payload[0] ^= 1;
    assert_eq!(
        verify_detached_signature(KEY, SIGNATURE, &changed_payload, EXPECTED),
        Err(RetainedProgramUpstreamClosureError::InvalidSignature)
    );
    let changed_identity = SignatureExpectation {
        key_id: "86C0A905F115EC04",
        ..EXPECTED
    };
    assert_eq!(
        verify_detached_signature(KEY, SIGNATURE, PAYLOAD, changed_identity),
        Err(RetainedProgramUpstreamClosureError::InvalidTrustRoot)
    );
    let changed_time = SignatureExpectation {
        created: EXPECTED.created + 1,
        ..EXPECTED
    };
    assert_eq!(
        verify_detached_signature(KEY, SIGNATURE, PAYLOAD, changed_time),
        Err(RetainedProgramUpstreamClosureError::InvalidSignature)
    );
}

#[test]
fn production_expectations_pin_exact_headers_and_identities() {
    let rust = SignatureExpectation::rust();
    assert_eq!(rust.fingerprint, RUST_FINGERPRINT);
    assert_eq!(rust.key_id, RUST_KEY_ID);
    assert_eq!(rust.created, RUST_SIGNATURE_CREATION);
    assert_eq!(rust.issuer_fingerprint_count, 0);
    assert_eq!(rust.key_armor_header, Some(b"Version: GnuPG v1".as_slice()));

    let alpine = SignatureExpectation::alpine();
    assert_eq!(alpine.fingerprint, ALPINE_FINGERPRINT);
    assert_eq!(alpine.key_id, ALPINE_KEY_ID);
    assert_eq!(alpine.created, ALPINE_SIGNATURE_CREATION);
    assert_eq!(alpine.issuer_fingerprint_count, 1);
    assert_eq!(
        alpine.key_armor_header,
        Some(b"Version: GnuPG v2".as_slice())
    );
}

#[test]
fn armor_rejects_crc_composition_and_trailing_content() {
    let crc_index = SIGNATURE
        .windows(2)
        .position(|window| window == b"\n=")
        .expect("CRC")
        + 2;
    let mut bad_crc = SIGNATURE.to_vec();
    bad_crc[crc_index] = if bad_crc[crc_index] == b'A' {
        b'B'
    } else {
        b'A'
    };
    for changed in [
        bad_crc,
        [SIGNATURE, SIGNATURE].concat(),
        [SIGNATURE, b"trailing"].concat(),
        SIGNATURE[..SIGNATURE.len() - 1].to_vec(),
        SIGNATURE.replace_lf_with_crlf(),
    ] {
        assert_eq!(
            verify_detached_signature(KEY, &changed, PAYLOAD, EXPECTED),
            Err(RetainedProgramUpstreamClosureError::InvalidSignature)
        );
    }
}

trait ReplaceLf {
    fn replace_lf_with_crlf(&self) -> Vec<u8>;
}

impl ReplaceLf for [u8] {
    fn replace_lf_with_crlf(&self) -> Vec<u8> {
        self.iter()
            .flat_map(|byte| {
                if *byte == b'\n' {
                    [Some(b'\r'), Some(b'\n')]
                } else {
                    [Some(*byte), None]
                }
            })
            .flatten()
            .collect()
    }
}
