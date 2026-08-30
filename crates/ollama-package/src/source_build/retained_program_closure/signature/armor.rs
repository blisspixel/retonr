const ARMOR_LINE_BYTES: usize = 64;

pub(super) fn decode_openpgp(
    bytes: &[u8],
    begin: &[u8],
    end: &[u8],
    expected_header: Option<&[u8]>,
    maximum_decoded: usize,
) -> Result<Vec<u8>, ()> {
    let inner = strict_inner(bytes, begin, end)?;
    let mut lines = exact_lines(inner)?;
    if let Some(header) = expected_header {
        if lines.first().copied() != Some(header) || lines.get(1).copied() != Some(b"") {
            return Err(());
        }
        lines.drain(..2);
    } else {
        if lines.first().copied() != Some(b"") {
            return Err(());
        }
        lines.remove(0);
    }
    let crc_line = lines.pop().ok_or(())?;
    let crc_text = crc_line.strip_prefix(b"=").ok_or(())?;
    if crc_text.len() != 4 || lines.is_empty() {
        return Err(());
    }
    let encoded = canonical_body(&lines, maximum_decoded)?;
    let decoded = decode_base64(&encoded, maximum_decoded)?;
    let crc = decode_base64(crc_text, 3)?;
    if crc.len() != 3 || crc.as_slice() != crc24(&decoded).as_slice() {
        return Err(());
    }
    Ok(decoded)
}

pub(super) fn decode_pem(
    bytes: &[u8],
    begin: &[u8],
    end: &[u8],
    maximum_decoded: usize,
) -> Result<Vec<u8>, ()> {
    let inner = strict_inner(bytes, begin, end)?;
    let lines = exact_lines(inner)?;
    if lines.is_empty() || lines.iter().any(|line| line.is_empty()) {
        return Err(());
    }
    let encoded = canonical_body(&lines, maximum_decoded)?;
    decode_base64(&encoded, maximum_decoded)
}

fn strict_inner<'a>(bytes: &'a [u8], begin: &[u8], end: &[u8]) -> Result<&'a [u8], ()> {
    if !bytes.starts_with(begin)
        || !bytes.ends_with(end)
        || occurrences(bytes, begin) != 1
        || occurrences(bytes, end) != 1
        || bytes.contains(&b'\r')
        || bytes
            .iter()
            .any(|byte| *byte != b'\n' && !(0x20..=0x7e).contains(byte))
    {
        return Err(());
    }
    bytes
        .get(begin.len()..bytes.len().checked_sub(end.len()).ok_or(())?)
        .ok_or(())
}

fn exact_lines(inner: &[u8]) -> Result<Vec<&[u8]>, ()> {
    let mut lines = inner.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    if lines.pop() != Some(b"") {
        return Err(());
    }
    Ok(lines)
}

fn canonical_body(lines: &[&[u8]], maximum_decoded: usize) -> Result<Vec<u8>, ()> {
    let maximum_encoded = maximum_decoded
        .checked_add(2)
        .and_then(|value| value.checked_div(3))
        .and_then(|value| value.checked_mul(4))
        .ok_or(())?;
    let mut encoded = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.is_empty()
            || line.len() > ARMOR_LINE_BYTES
            || (index + 1 != lines.len() && line.len() != ARMOR_LINE_BYTES)
            || encoded.len().checked_add(line.len()).is_none()
        {
            return Err(());
        }
        encoded.extend_from_slice(line);
    }
    if encoded.len() > maximum_encoded {
        return Err(());
    }
    Ok(encoded)
}

fn decode_base64(encoded: &[u8], maximum_decoded: usize) -> Result<Vec<u8>, ()> {
    if encoded.is_empty() || !encoded.len().is_multiple_of(4) {
        return Err(());
    }
    let mut decoded = Vec::with_capacity(encoded.len() / 4 * 3);
    for (index, chunk) in encoded.chunks_exact(4).enumerate() {
        let final_chunk = index + 1 == encoded.len() / 4;
        let a = base64_value(chunk[0]).ok_or(())?;
        let b = base64_value(chunk[1]).ok_or(())?;
        let c = (chunk[2] != b'=').then(|| base64_value(chunk[2])).flatten();
        let d = (chunk[3] != b'=').then(|| base64_value(chunk[3])).flatten();
        if c.is_none() && (!final_chunk || chunk[3] != b'=' || b & 0x0f != 0)
            || c.is_some() && d.is_none() && (!final_chunk || c.ok_or(())? & 0x03 != 0)
            || c.is_none() && d.is_some()
        {
            return Err(());
        }
        decoded.push((a << 2) | (b >> 4));
        if let Some(c) = c {
            decoded.push((b << 4) | (c >> 2));
            if let Some(d) = d {
                decoded.push((c << 6) | d);
            }
        }
        if decoded.len() > maximum_decoded {
            return Err(());
        }
    }
    Ok(decoded)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn crc24(bytes: &[u8]) -> [u8; 3] {
    let mut crc = 0x00b7_04ce_u32;
    for byte in bytes {
        crc ^= u32::from(*byte) << 16;
        for _ in 0..8 {
            crc <<= 1;
            if crc & 0x0100_0000 != 0 {
                crc ^= 0x0186_4cfb;
            }
        }
    }
    let bytes = crc.to_be_bytes();
    [bytes[1], bytes[2], bytes[3]]
}

fn occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_BEGIN: &[u8] = b"-----BEGIN PGP PUBLIC KEY BLOCK-----\n";
    const KEY_END: &[u8] = b"-----END PGP PUBLIC KEY BLOCK-----\n";

    #[test]
    fn base64_and_crc_are_canonical_and_bounded() {
        assert_eq!(decode_base64(b"YWJj", 3), Ok(b"abc".to_vec()));
        assert_eq!(decode_base64(b"YQ==", 1), Ok(b"a".to_vec()));
        assert_eq!(decode_base64(b"YWI=", 2), Ok(b"ab".to_vec()));
        for changed in [
            b"YQ=A".as_slice(),
            b"YR==".as_slice(),
            b"YWJ=".as_slice(),
            b"YWJj====".as_slice(),
            b"YWJ".as_slice(),
        ] {
            assert!(decode_base64(changed, 16).is_err());
        }
        assert_eq!(crc24(b"123456789"), [0x21, 0xcf, 0x02]);
    }

    #[test]
    fn exact_optional_header_is_required_when_configured() {
        let key = include_bytes!("fixtures/key.asc");
        let with_header = [
            KEY_BEGIN,
            b"Version: fixture\n",
            key.get(KEY_BEGIN.len()..).expect("fixture body"),
        ]
        .concat();
        assert!(
            decode_openpgp(
                &with_header,
                KEY_BEGIN,
                KEY_END,
                Some(b"Version: fixture"),
                4096
            )
            .is_ok()
        );
        assert!(decode_openpgp(key, KEY_BEGIN, KEY_END, Some(b"Version: fixture"), 4096).is_err());
        assert!(decode_openpgp(&with_header, KEY_BEGIN, KEY_END, None, 4096).is_err());
        assert!(decode_openpgp(key, KEY_BEGIN, KEY_END, None, 1).is_err());
    }
}
