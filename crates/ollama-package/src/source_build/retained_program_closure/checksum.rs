use rewrite_types::Digest;

use super::RetainedProgramUpstreamClosureError;

const RUST_CHANNEL_NAME: &str = "channel-rust-1.97.1.toml";
const RUST_CHANNEL_DIGEST: &str =
    "03569b1886ceb5c05276b50c8431ab111de944cd6140fe1fa7d821dd8e0f29cf";
const ALPINE_MINIROOTFS_NAME: &str = "alpine-minirootfs-3.23.3-x86_64.tar.gz";
const ALPINE_MINIROOTFS_DIGEST: &str =
    "42d0e6d8de5521e7bf92e075e032b5690c1d948fa9775efa32a51a38b25460fb";

pub(super) fn verify_rust_checksum(
    bytes: &[u8],
) -> Result<(), RetainedProgramUpstreamClosureError> {
    verify_checksum(bytes, RUST_CHANNEL_DIGEST, RUST_CHANNEL_NAME)
}

pub(super) fn verify_alpine_checksum(
    bytes: &[u8],
) -> Result<(), RetainedProgramUpstreamClosureError> {
    verify_checksum(bytes, ALPINE_MINIROOTFS_DIGEST, ALPINE_MINIROOTFS_NAME)
}

fn verify_checksum(
    bytes: &[u8],
    expected_digest: &str,
    expected_name: &str,
) -> Result<(), RetainedProgramUpstreamClosureError> {
    let expected_length = 64_usize
        .checked_add(2)
        .and_then(|length| length.checked_add(expected_name.len()))
        .and_then(|length| length.checked_add(1))
        .ok_or(RetainedProgramUpstreamClosureError::InvalidChecksum)?;
    if bytes.len() != expected_length || bytes.last() != Some(&b'\n') {
        return Err(RetainedProgramUpstreamClosureError::InvalidChecksum);
    }
    let line = &bytes[..bytes.len() - 1];
    let (digest, name) = line
        .split_at_checked(64)
        .ok_or(RetainedProgramUpstreamClosureError::InvalidChecksum)?;
    if !digest
        .iter()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || name.get(..2) != Some(b"  ")
        || name.get(2..) != Some(expected_name.as_bytes())
        || digest != expected_digest.as_bytes()
        || Digest::from_sha256_hex(expected_digest).is_err()
    {
        return Err(RetainedProgramUpstreamClosureError::InvalidChecksum);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ALPINE_MINIROOTFS_DIGEST, ALPINE_MINIROOTFS_NAME, verify_checksum};

    #[test]
    fn accepts_only_exact_checksum_grammar() {
        let canonical = format!("{ALPINE_MINIROOTFS_DIGEST}  {ALPINE_MINIROOTFS_NAME}\n");
        assert!(
            verify_checksum(
                canonical.as_bytes(),
                ALPINE_MINIROOTFS_DIGEST,
                ALPINE_MINIROOTFS_NAME,
            )
            .is_ok()
        );
        for changed in [
            canonical.replace("  ", " "),
            canonical.replace('a', "A"),
            canonical.replace('\n', "\r\n"),
            canonical.trim_end().to_owned(),
            format!("{canonical}ignored"),
            canonical.replace("alpine-", "other-"),
        ] {
            assert!(
                verify_checksum(
                    changed.as_bytes(),
                    ALPINE_MINIROOTFS_DIGEST,
                    ALPINE_MINIROOTFS_NAME,
                )
                .is_err(),
                "accepted changed checksum: {changed:?}"
            );
        }
    }
}
