use ring::signature::{RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY, RsaPublicKeyComponents};

use super::armor;

const PEM_BEGIN: &[u8] = b"-----BEGIN PUBLIC KEY-----\n";
const PEM_END: &[u8] = b"-----END PUBLIC KEY-----\n";
const MAXIMUM_PUBLIC_KEY_BYTES: usize = 8 * 1024;
const RSA_ENCRYPTION_IDENTIFIER: &[u8] = &[
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00,
];

pub(super) fn verify_rsa_sha1(pem: &[u8], message: &[u8], signature: &[u8]) -> Result<(), ()> {
    let der = armor::decode_pem(pem, PEM_BEGIN, PEM_END, MAXIMUM_PUBLIC_KEY_BYTES)?;
    let key = parse_spki(&der)?;
    if signature.len() != key.modulus.len() {
        return Err(());
    }
    RsaPublicKeyComponents {
        n: key.modulus,
        e: key.exponent,
    }
    .verify(
        &RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY,
        message,
        signature,
    )
    .map_err(|_| ())
}

struct RsaKey<'a> {
    modulus: &'a [u8],
    exponent: &'a [u8],
}

fn parse_spki(input: &[u8]) -> Result<RsaKey<'_>, ()> {
    let mut outer = DerReader::sequence(input)?;
    let algorithm = outer.element(0x30)?;
    if algorithm != RSA_ENCRYPTION_IDENTIFIER {
        return Err(());
    }
    let bit_string = outer.element(0x03)?;
    outer.finish()?;
    if bit_string.first() != Some(&0) {
        return Err(());
    }
    let mut rsa = DerReader::sequence(bit_string.get(1..).ok_or(())?)?;
    let modulus = positive_integer(rsa.element(0x02)?)?;
    let exponent = positive_integer(rsa.element(0x02)?)?;
    rsa.finish()?;
    if !(256..=1024).contains(&modulus.len()) || exponent != [0x01, 0x00, 0x01] {
        return Err(());
    }
    Ok(RsaKey { modulus, exponent })
}

fn positive_integer(bytes: &[u8]) -> Result<&[u8], ()> {
    if bytes.is_empty() || bytes[0] & 0x80 != 0 {
        return Err(());
    }
    if bytes[0] == 0 {
        if bytes.len() < 2 || bytes[1] & 0x80 == 0 {
            return Err(());
        }
        return Ok(&bytes[1..]);
    }
    Ok(bytes)
}

struct DerReader<'a> {
    remaining: &'a [u8],
}

impl<'a> DerReader<'a> {
    fn sequence(input: &'a [u8]) -> Result<Self, ()> {
        let mut reader = Self { remaining: input };
        let contents = reader.element(0x30)?;
        reader.finish()?;
        Ok(Self {
            remaining: contents,
        })
    }

    fn element(&mut self, expected_tag: u8) -> Result<&'a [u8], ()> {
        if self.remaining.first() != Some(&expected_tag) {
            return Err(());
        }
        let (length, length_bytes) = der_length(self.remaining.get(1..).ok_or(())?)?;
        let header = 1_usize.checked_add(length_bytes).ok_or(())?;
        let end = header.checked_add(length).ok_or(())?;
        let contents = self.remaining.get(header..end).ok_or(())?;
        self.remaining = self.remaining.get(end..).ok_or(())?;
        Ok(contents)
    }

    fn finish(self) -> Result<(), ()> {
        self.remaining.is_empty().then_some(()).ok_or(())
    }
}

fn der_length(input: &[u8]) -> Result<(usize, usize), ()> {
    let first = *input.first().ok_or(())?;
    if first < 0x80 {
        return Ok((usize::from(first), 1));
    }
    let width = usize::from(first & 0x7f);
    if width == 0 || width > 4 {
        return Err(());
    }
    let bytes = input.get(1..1 + width).ok_or(())?;
    if bytes.first() == Some(&0) || (width == 1 && bytes[0] < 0x80) {
        return Err(());
    }
    let length = bytes
        .iter()
        .fold(0_usize, |value, byte| (value << 8) | usize::from(*byte));
    Ok((length, width + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_lengths_and_integers_are_canonical() {
        assert_eq!(der_length(&[127]), Ok((127, 1)));
        assert_eq!(der_length(&[0x81, 0x80]), Ok((128, 2)));
        for invalid in [&[0x80][..], &[0x81, 0x7f], &[0x82, 0, 0x80]] {
            assert!(der_length(invalid).is_err());
        }
        assert_eq!(positive_integer(&[0, 0x80]), Ok(&[0x80][..]));
        for invalid in [&[][..], &[0], &[0, 1], &[0x80]] {
            assert!(positive_integer(invalid).is_err());
        }
    }
}
