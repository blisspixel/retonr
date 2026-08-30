use ring::{
    digest::{self, Context},
    signature::{RSA_PKCS1_2048_8192_SHA512, RsaPublicKeyComponents},
};

const MAXIMUM_CERTIFICATE_PACKETS: usize = 64;
const MAXIMUM_CERTIFICATE_PACKET_BYTES: usize = 64 * 1024;
const MAXIMUM_SIGNATURE_PACKET_BYTES: usize = 16 * 1024;
const MAXIMUM_SIGNED_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

pub(super) struct RsaPublicKey {
    pub(super) modulus: Vec<u8>,
    pub(super) exponent: Vec<u8>,
    pub(super) fingerprint: [u8; 20],
    pub(super) key_id: [u8; 8],
}

pub(super) struct BinarySignature {
    pub(super) created: u64,
    pub(super) issuer_key_ids: Vec<[u8; 8]>,
    pub(super) issuer_fingerprints: Vec<[u8; 20]>,
    signed_header: Vec<u8>,
    left_hash: [u8; 2],
    rsa_mpi: Vec<u8>,
}

pub(super) fn parse_certificate(bytes: &[u8]) -> Result<RsaPublicKey, ()> {
    let packets = packets(
        bytes,
        MAXIMUM_CERTIFICATE_PACKETS,
        MAXIMUM_CERTIFICATE_PACKET_BYTES,
    )?;
    let primary_packets = packets
        .iter()
        .filter(|packet| packet.tag == 6)
        .collect::<Vec<_>>();
    if primary_packets.len() != 1
        || packets
            .iter()
            .any(|packet| !matches!(packet.tag, 2 | 6 | 13 | 14 | 17))
    {
        return Err(());
    }
    let body = primary_packets[0].body;
    if body.len() < 8 || body[0] != 4 || body[5] != 1 || body.len() > usize::from(u16::MAX) {
        return Err(());
    }
    let (modulus, remainder) = mpi(&body[6..])?;
    let (exponent, remainder) = mpi(remainder)?;
    if !remainder.is_empty()
        || !(256..=1024).contains(&modulus.len())
        || exponent != [0x01, 0x00, 0x01]
    {
        return Err(());
    }
    let mut fingerprint_input = Vec::with_capacity(body.len() + 3);
    fingerprint_input.push(0x99);
    fingerprint_input.extend_from_slice(&u16::try_from(body.len()).map_err(|_| ())?.to_be_bytes());
    fingerprint_input.extend_from_slice(body);
    let digest = digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, &fingerprint_input);
    let fingerprint: [u8; 20] = digest.as_ref().try_into().map_err(|_| ())?;
    let key_id = fingerprint[12..].try_into().map_err(|_| ())?;
    Ok(RsaPublicKey {
        modulus: modulus.to_vec(),
        exponent: exponent.to_vec(),
        fingerprint,
        key_id,
    })
}

pub(super) fn parse_binary_signature(bytes: &[u8]) -> Result<BinarySignature, ()> {
    let packets = packets(bytes, 1, MAXIMUM_SIGNATURE_PACKET_BYTES)?;
    if packets.len() != 1 || packets[0].tag != 2 {
        return Err(());
    }
    let body = packets[0].body;
    if body.len() < 12 || body[..4] != [4, 0, 1, 10] {
        return Err(());
    }
    let hashed_length = usize::from(u16::from_be_bytes([body[4], body[5]]));
    let hashed_end = 6_usize.checked_add(hashed_length).ok_or(())?;
    let unhashed_length_bytes = body
        .get(hashed_end..hashed_end.checked_add(2).ok_or(())?)
        .ok_or(())?;
    let unhashed_length = usize::from(u16::from_be_bytes(
        unhashed_length_bytes.try_into().map_err(|_| ())?,
    ));
    let unhashed_start = hashed_end + 2;
    let unhashed_end = unhashed_start.checked_add(unhashed_length).ok_or(())?;
    let left_hash_end = unhashed_end.checked_add(2).ok_or(())?;
    let left_hash: [u8; 2] = body
        .get(unhashed_end..left_hash_end)
        .ok_or(())?
        .try_into()
        .map_err(|_| ())?;
    let hashed = subpackets(body.get(6..hashed_end).ok_or(())?)?;
    let unhashed = subpackets(body.get(unhashed_start..unhashed_end).ok_or(())?)?;
    let creations = hashed
        .iter()
        .filter_map(|subpacket| (subpacket.kind == 2).then_some(subpacket.body))
        .collect::<Vec<_>>();
    if creations.len() != 1
        || creations[0].len() != 4
        || unhashed.iter().any(|subpacket| subpacket.kind == 2)
    {
        return Err(());
    }
    let created = u64::from(u32::from_be_bytes(creations[0].try_into().map_err(|_| ())?));
    let issuer_key_ids = hashed
        .iter()
        .chain(&unhashed)
        .filter_map(|subpacket| (subpacket.kind == 16).then_some(subpacket.body))
        .map(|body| body.try_into().map_err(|_| ()))
        .collect::<Result<Vec<[u8; 8]>, ()>>()?;
    let issuer_fingerprints = hashed
        .iter()
        .chain(&unhashed)
        .filter_map(|subpacket| (subpacket.kind == 33).then_some(subpacket.body))
        .map(|body| {
            if body.first() != Some(&4) || body.len() != 21 {
                return Err(());
            }
            body[1..].try_into().map_err(|_| ())
        })
        .collect::<Result<Vec<[u8; 20]>, ()>>()?;
    let (rsa_mpi, remainder) = mpi(body.get(left_hash_end..).ok_or(())?)?;
    if !remainder.is_empty() {
        return Err(());
    }
    Ok(BinarySignature {
        created,
        issuer_key_ids,
        issuer_fingerprints,
        signed_header: body[..hashed_end].to_vec(),
        left_hash,
        rsa_mpi: rsa_mpi.to_vec(),
    })
}

pub(super) fn verify(
    key: &RsaPublicKey,
    signature: &BinarySignature,
    message: &[u8],
) -> Result<(), ()> {
    let trailer_length = u32::try_from(signature.signed_header.len()).map_err(|_| ())?;
    let extra_length = signature.signed_header.len().checked_add(6).ok_or(())?;
    let total_length = message.len().checked_add(extra_length).ok_or(())?;
    if total_length > MAXIMUM_SIGNED_MESSAGE_BYTES || signature.rsa_mpi.len() > key.modulus.len() {
        return Err(());
    }
    let mut signed = Vec::with_capacity(total_length);
    signed.extend_from_slice(message);
    signed.extend_from_slice(&signature.signed_header);
    signed.extend_from_slice(&[4, 0xff]);
    signed.extend_from_slice(&trailer_length.to_be_bytes());
    let mut hash = Context::new(&digest::SHA512);
    hash.update(&signed);
    if hash.finish().as_ref().get(..2) != Some(signature.left_hash.as_slice()) {
        return Err(());
    }
    let mut normalized = vec![0; key.modulus.len()];
    let offset = normalized
        .len()
        .checked_sub(signature.rsa_mpi.len())
        .ok_or(())?;
    normalized[offset..].copy_from_slice(&signature.rsa_mpi);
    RsaPublicKeyComponents {
        n: &key.modulus,
        e: &key.exponent,
    }
    .verify(&RSA_PKCS1_2048_8192_SHA512, &signed, &normalized)
    .map_err(|_| ())
}

struct Packet<'a> {
    tag: u8,
    body: &'a [u8],
}

fn packets(bytes: &[u8], maximum_count: usize, maximum_body: usize) -> Result<Vec<Packet<'_>>, ()> {
    let mut input = bytes;
    let mut result = Vec::new();
    while !input.is_empty() {
        if result.len() >= maximum_count {
            return Err(());
        }
        let first = input[0];
        if first & 0x80 == 0 {
            return Err(());
        }
        let (tag, length, header_length) = if first & 0x40 != 0 {
            let (length, used) = new_length(input.get(1..).ok_or(())?)?;
            (first & 0x3f, length, used + 1)
        } else {
            let length_type = first & 0x03;
            let width = match length_type {
                0 => 1,
                1 => 2,
                2 => 4,
                _ => return Err(()),
            };
            let length = be_usize(input.get(1..1 + width).ok_or(())?)?;
            ((first >> 2) & 0x0f, length, width + 1)
        };
        if length > maximum_body {
            return Err(());
        }
        let end = header_length.checked_add(length).ok_or(())?;
        let body = input.get(header_length..end).ok_or(())?;
        result.push(Packet { tag, body });
        input = input.get(end..).ok_or(())?;
    }
    Ok(result)
}

fn new_length(input: &[u8]) -> Result<(usize, usize), ()> {
    match *input.first().ok_or(())? {
        value @ 0..=191 => Ok((usize::from(value), 1)),
        value @ 192..=223 => Ok((
            usize::from(value - 192) * 256 + usize::from(*input.get(1).ok_or(())?) + 192,
            2,
        )),
        255 => Ok((be_usize(input.get(1..5).ok_or(())?)?, 5)),
        _ => Err(()),
    }
}

fn be_usize(bytes: &[u8]) -> Result<usize, ()> {
    if bytes.is_empty() || bytes.len() > 4 {
        return Err(());
    }
    Ok(bytes
        .iter()
        .fold(0_usize, |value, byte| (value << 8) | usize::from(*byte)))
}

struct Subpacket<'a> {
    kind: u8,
    body: &'a [u8],
}

fn subpackets(mut input: &[u8]) -> Result<Vec<Subpacket<'_>>, ()> {
    let mut result = Vec::new();
    while !input.is_empty() {
        if result.len() >= 64 {
            return Err(());
        }
        let (length, length_bytes) = new_length(input)?;
        if length == 0 || length > MAXIMUM_SIGNATURE_PACKET_BYTES {
            return Err(());
        }
        let end = length_bytes.checked_add(length).ok_or(())?;
        let packet = input.get(length_bytes..end).ok_or(())?;
        let kind_octet = packet[0];
        let kind = kind_octet & 0x7f;
        if kind_octet & 0x80 != 0 && !matches!(kind, 2 | 16 | 33) {
            return Err(());
        }
        result.push(Subpacket {
            kind,
            body: &packet[1..],
        });
        input = input.get(end..).ok_or(())?;
    }
    Ok(result)
}

fn mpi(input: &[u8]) -> Result<(&[u8], &[u8]), ()> {
    let bit_length = usize::from(u16::from_be_bytes(
        input.get(..2).ok_or(())?.try_into().map_err(|_| ())?,
    ));
    if bit_length == 0 {
        return Err(());
    }
    let byte_length = bit_length.checked_add(7).ok_or(())? / 8;
    let value = input.get(2..2 + byte_length).ok_or(())?;
    let leading = *value.first().ok_or(())?;
    if leading == 0
        || usize::try_from(leading.leading_zeros()).map_err(|_| ())? + bit_length != byte_length * 8
    {
        return Err(());
    }
    Ok((value, input.get(2 + byte_length..).ok_or(())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_ARMOR: &[u8] = include_bytes!("fixtures/key.asc");
    const SIGNATURE_ARMOR: &[u8] = include_bytes!("fixtures/signature.asc");
    const PAYLOAD: &[u8] = b"deterministic retained-program upstream fixture";

    fn decoded_key() -> Vec<u8> {
        super::super::armor::decode_openpgp(
            KEY_ARMOR,
            b"-----BEGIN PGP PUBLIC KEY BLOCK-----\n",
            b"-----END PGP PUBLIC KEY BLOCK-----\n",
            None,
            64 * 1024,
        )
        .expect("fixture key armor")
    }

    fn decoded_signature() -> Vec<u8> {
        super::super::armor::decode_openpgp(
            SIGNATURE_ARMOR,
            b"-----BEGIN PGP SIGNATURE-----\n",
            b"-----END PGP SIGNATURE-----\n",
            None,
            16 * 1024,
        )
        .expect("fixture signature armor")
    }

    #[test]
    fn packet_lengths_and_mpis_reject_noncanonical_or_unbounded_data() {
        assert_eq!(new_length(&[191]), Ok((191, 1)));
        assert_eq!(new_length(&[192, 0]), Ok((192, 2)));
        assert_eq!(new_length(&[255, 0, 0, 1, 0]), Ok((256, 5)));
        for invalid in [&[224][..], &[254], &[]] {
            assert!(new_length(invalid).is_err());
        }
        assert!(mpi(&[0, 8, 0]).is_err());
        assert!(mpi(&[0, 7, 0x80]).is_err());
        assert_eq!(mpi(&[0, 9, 1, 0]).expect("MPI").0, [1, 0]);
    }

    #[test]
    fn certificate_parser_rejects_composition_algorithm_and_packet_drift() {
        let key = decoded_key();
        assert!(parse_certificate(&key).is_ok());
        assert_ne!(key[0] & 0x40, 0);
        let (_, encoded_length_bytes) = new_length(&key[1..]).expect("primary length");
        let first_header_length = 1 + encoded_length_bytes;
        let mut wrong_algorithm = key.clone();
        wrong_algorithm[first_header_length + 5] = 2;
        let mut duplicate = key.clone();
        duplicate.extend_from_slice(&key);
        let mut unknown = key.clone();
        unknown.extend_from_slice(&[0xcb, 0]);
        for invalid in [wrong_algorithm, duplicate, unknown, Vec::new()] {
            assert!(parse_certificate(&invalid).is_err());
        }
        for invalid in [&[0x00][..], &[0x9b], &[0xc2, 224], &[0xc2, 255, 0, 1]] {
            assert!(packets(invalid, 4, 32).is_err());
        }
    }

    #[test]
    fn signature_parser_and_crypto_reject_structural_and_value_mutations() {
        let key = parse_certificate(&decoded_key()).expect("fixture key");
        let encoded = decoded_signature();
        let body_length = packets(&encoded, 1, 16 * 1024).expect("packet")[0]
            .body
            .len();
        let header_length = encoded.len() - body_length;
        let parsed = parse_binary_signature(&encoded).expect("fixture signature");
        assert!(verify(&key, &parsed, PAYLOAD).is_ok());
        for (body_index, value) in [(0, 3), (1, 1), (2, 2), (3, 8)] {
            let mut changed = encoded.clone();
            changed[header_length + body_index] = value;
            assert!(parse_binary_signature(&changed).is_err());
        }
        let mut bad_hashed_length = encoded.clone();
        bad_hashed_length[header_length + 4..header_length + 6]
            .copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(parse_binary_signature(&bad_hashed_length).is_err());
        let mut composed = encoded.clone();
        composed.extend_from_slice(&encoded);
        assert!(parse_binary_signature(&composed).is_err());
        let mut wrong_tag = encoded.clone();
        wrong_tag[0] = (wrong_tag[0] & 0xc0) | 6;
        assert!(parse_binary_signature(&wrong_tag).is_err());

        let mut wrong_hash = parse_binary_signature(&encoded).expect("signature");
        wrong_hash.left_hash[0] ^= 1;
        assert!(verify(&key, &wrong_hash, PAYLOAD).is_err());
        let mut wrong_rsa = parse_binary_signature(&encoded).expect("signature");
        wrong_rsa.rsa_mpi[0] ^= 1;
        assert!(verify(&key, &wrong_rsa, PAYLOAD).is_err());
        let mut oversized_rsa = parse_binary_signature(&encoded).expect("signature");
        oversized_rsa.rsa_mpi.push(0);
        assert!(verify(&key, &oversized_rsa, PAYLOAD).is_err());
        assert!(verify(&key, &parsed, &vec![0; MAXIMUM_SIGNED_MESSAGE_BYTES + 1]).is_err());
    }

    #[test]
    fn subpacket_parser_rejects_partial_zero_and_unknown_critical_packets() {
        for invalid in [&[0][..], &[224], &[1, 0xe3], &[2, 0xe3, 0]] {
            assert!(subpackets(invalid).is_err());
        }
        assert_eq!(be_usize(&[1, 2]), Ok(258));
        assert!(be_usize(&[]).is_err());
        assert!(be_usize(&[0; 5]).is_err());
    }
}
