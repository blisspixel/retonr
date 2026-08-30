use super::RetainedProgramUpstreamClosureError;

mod armor;
mod der;
mod openpgp;

pub(super) const RUST_FINGERPRINT: &str = "108F66205EAEB0AAA8DD5E1C85AB96E6FA1BE5FE";
pub(super) const RUST_KEY_ID: &str = "85AB96E6FA1BE5FE";
pub(super) const RUST_SIGNATURE_CREATION: u64 = 1_784_203_793;
pub(super) const ALPINE_FINGERPRINT: &str = "0482D84022F52DF1C4E7CD43293ACD0907D9495A";
pub(super) const ALPINE_KEY_ID: &str = "293ACD0907D9495A";
pub(super) const ALPINE_SIGNATURE_CREATION: u64 = 1_769_556_336;

const SIGNATURE_BEGIN: &[u8] = b"-----BEGIN PGP SIGNATURE-----\n";
const SIGNATURE_END: &[u8] = b"-----END PGP SIGNATURE-----\n";
const KEY_BEGIN: &[u8] = b"-----BEGIN PGP PUBLIC KEY BLOCK-----\n";
const KEY_END: &[u8] = b"-----END PGP PUBLIC KEY BLOCK-----\n";
const MAXIMUM_CERTIFICATE_BYTES: usize = 64 * 1024;
const MAXIMUM_SIGNATURE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy)]
pub(super) struct SignatureExpectation<'a> {
    fingerprint: &'a str,
    key_id: &'a str,
    created: u64,
    issuer_fingerprint_count: usize,
    key_armor_header: Option<&'a [u8]>,
}

impl SignatureExpectation<'static> {
    pub(super) const fn rust() -> Self {
        Self {
            fingerprint: RUST_FINGERPRINT,
            key_id: RUST_KEY_ID,
            created: RUST_SIGNATURE_CREATION,
            issuer_fingerprint_count: 0,
            key_armor_header: Some(b"Version: GnuPG v1"),
        }
    }

    pub(super) const fn alpine() -> Self {
        Self {
            fingerprint: ALPINE_FINGERPRINT,
            key_id: ALPINE_KEY_ID,
            created: ALPINE_SIGNATURE_CREATION,
            issuer_fingerprint_count: 1,
            key_armor_header: Some(b"Version: GnuPG v2"),
        }
    }
}

pub(super) fn verify_detached_signature(
    trust_root: &[u8],
    signature_bytes: &[u8],
    signed_bytes: &[u8],
    expected: SignatureExpectation<'_>,
) -> Result<(), RetainedProgramUpstreamClosureError> {
    let certificate_bytes = armor::decode_openpgp(
        trust_root,
        KEY_BEGIN,
        KEY_END,
        expected.key_armor_header,
        MAXIMUM_CERTIFICATE_BYTES,
    )
    .map_err(|()| RetainedProgramUpstreamClosureError::InvalidTrustRoot)?;
    let certificate = openpgp::parse_certificate(&certificate_bytes)
        .map_err(|()| RetainedProgramUpstreamClosureError::InvalidTrustRoot)?;
    if uppercase_hex(&certificate.fingerprint) != expected.fingerprint
        || uppercase_hex(&certificate.key_id) != expected.key_id
    {
        return Err(RetainedProgramUpstreamClosureError::InvalidTrustRoot);
    }

    let binary_signature = armor::decode_openpgp(
        signature_bytes,
        SIGNATURE_BEGIN,
        SIGNATURE_END,
        None,
        MAXIMUM_SIGNATURE_BYTES,
    )
    .map_err(|()| RetainedProgramUpstreamClosureError::InvalidSignature)?;
    let signature = openpgp::parse_binary_signature(&binary_signature)
        .map_err(|()| RetainedProgramUpstreamClosureError::InvalidSignature)?;
    if signature.created != expected.created
        || signature.issuer_key_ids.as_slice() != [certificate.key_id]
        || signature.issuer_fingerprints.len() != expected.issuer_fingerprint_count
        || signature
            .issuer_fingerprints
            .iter()
            .any(|fingerprint| fingerprint != &certificate.fingerprint)
    {
        return Err(RetainedProgramUpstreamClosureError::InvalidSignature);
    }
    openpgp::verify(&certificate, &signature, signed_bytes)
        .map_err(|()| RetainedProgramUpstreamClosureError::InvalidSignature)
}

pub(super) fn verify_rsa_pkcs1_sha1_public_key_pem(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), RetainedProgramUpstreamClosureError> {
    der::verify_rsa_sha1(public_key, message, signature)
        .map_err(|()| RetainedProgramUpstreamClosureError::InvalidAlpinePackage)
}

fn uppercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

#[cfg(test)]
mod tests;
