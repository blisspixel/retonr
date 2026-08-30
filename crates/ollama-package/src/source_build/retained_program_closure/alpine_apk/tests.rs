use rewrite_types::Digest;

use super::{
    ApkExpectation, ApkPayloadExpectation, MAXIMUM_CONTROL_SEGMENT_BYTES,
    MAXIMUM_DATA_SEGMENT_BYTES, MAXIMUM_SIGNATURE_SEGMENT_BYTES, decode_gzip_member,
    extract_metadata, extract_root_key, extract_signature, validate_data_tar,
    verify_alpine_package, verify_metadata,
};

struct Fixture {
    minirootfs: Vec<u8>,
    package: Vec<u8>,
    signature_size: u64,
    data_hash: String,
    payload: Vec<u8>,
    payload_digest: String,
}

#[test]
fn synthetic_apk_signature_and_data_hash_verify_in_ci() {
    let fixture = fixture();
    let expected = ApkExpectation {
        signature_size: fixture.signature_size,
        package_name: "libgcc",
        package_version: "15.2.0-r2",
        origin: "gcc",
        build_date: "1761849853",
        commit: "7fa6e7fd3465c5ff89cc33a56d20bf4d59105aaf",
        data_hash: &fixture.data_hash,
        payload: Some(ApkPayloadExpectation {
            path: b"usr/lib/libgcc_s.so.1",
            size: u64::try_from(fixture.payload.len()).expect("payload size"),
            mode: 0o755,
            digest: &fixture.payload_digest,
        }),
    };
    extract_root_key(&fixture.minirootfs).expect("root key");
    let (signature_end, signature_tar) =
        decode_gzip_member(&fixture.package, MAXIMUM_SIGNATURE_SEGMENT_BYTES)
            .expect("signature gzip");
    extract_signature(&signature_tar, fixture.signature_size).expect("signature tar");
    let (control_length, control_tar) = decode_gzip_member(
        &fixture.package[signature_end..],
        MAXIMUM_CONTROL_SEGMENT_BYTES,
    )
    .expect("control gzip");
    let metadata = extract_metadata(&control_tar).expect("control tar");
    let data = &fixture.package[signature_end + control_length..];
    let (_, data_tar) = decode_gzip_member(data, MAXIMUM_DATA_SEGMENT_BYTES).expect("data gzip");
    validate_data_tar(&data_tar, expected.payload).expect("data tar");
    verify_metadata(&metadata, data, expected).expect("metadata");
    assert!(decode_gzip_member(b"not gzip", 32).is_err());
    assert!(decode_gzip_member(&fixture.package, 1).is_err());
    assert!(extract_signature(&signature_tar, fixture.signature_size + 1).is_err());
    for payload in [
        ApkPayloadExpectation {
            path: b"usr/lib/missing.so",
            ..expected.payload.expect("payload expectation")
        },
        ApkPayloadExpectation {
            size: 1,
            ..expected.payload.expect("payload expectation")
        },
        ApkPayloadExpectation {
            mode: 0o644,
            ..expected.payload.expect("payload expectation")
        },
        ApkPayloadExpectation {
            digest: "0000000000000000000000000000000000000000000000000000000000000000",
            ..expected.payload.expect("payload expectation")
        },
    ] {
        assert!(validate_data_tar(&data_tar, Some(payload)).is_err());
    }
    let verified = verify_alpine_package(&fixture.minirootfs, &fixture.package, expected)
        .expect("synthetic APK should verify");
    assert_eq!(
        verified.payload.as_deref(),
        Some(fixture.payload.as_slice())
    );

    let mut changed_package = fixture.package.clone();
    let last = changed_package.len() - 1;
    changed_package[last] ^= 1;
    assert!(verify_alpine_package(&fixture.minirootfs, &changed_package, expected).is_err());
    assert!(verify_alpine_package(b"not a rootfs", &fixture.package, expected).is_err());
}

fn fixture() -> Fixture {
    let minirootfs = decode_fixture_base64(include_str!("fixtures/minirootfs.b64"));
    let package = decode_fixture_base64(include_str!("fixtures/package.b64"));
    let payload = b"synthetic libgcc".to_vec();
    let payload_digest = Digest::sha256(&payload).as_str().to_owned();
    let (signature_end, _) = decode_gzip_member(&package, MAXIMUM_SIGNATURE_SEGMENT_BYTES)
        .expect("fixture signature segment");
    let (control_length, _) =
        decode_gzip_member(&package[signature_end..], MAXIMUM_CONTROL_SEGMENT_BYTES)
            .expect("fixture control segment");
    let data = &package[signature_end + control_length..];
    let data_hash = Digest::sha256(data).as_str().to_owned();
    Fixture {
        minirootfs,
        package,
        signature_size: 256,
        data_hash,
        payload,
        payload_digest,
    }
}

fn decode_fixture_base64(text: &str) -> Vec<u8> {
    let encoded = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    let mut result = Vec::with_capacity(encoded.len() / 4 * 3);
    for chunk in encoded.chunks_exact(4) {
        let value = |byte| match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 0,
            _ => panic!("fixture base64 is checked in and canonical"),
        };
        let values = [
            value(chunk[0]),
            value(chunk[1]),
            value(chunk[2]),
            value(chunk[3]),
        ];
        result.push((values[0] << 2) | (values[1] >> 4));
        if chunk[2] != b'=' {
            result.push((values[1] << 4) | (values[2] >> 2));
        }
        if chunk[3] != b'=' {
            result.push((values[2] << 6) | values[3]);
        }
    }
    result
}
