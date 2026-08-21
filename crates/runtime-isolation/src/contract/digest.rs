use rewrite_types::Digest;

pub(super) struct RedactedDigestBuilder {
    bytes: Vec<u8>,
}

impl RedactedDigestBuilder {
    pub(super) fn new(domain: &[u8]) -> Self {
        let mut builder = Self { bytes: Vec::new() };
        builder.push_bytes(b"retonr/redacted-digest/v1");
        builder.push_bytes(domain);
        builder
    }

    pub(super) fn push_bytes(&mut self, value: &[u8]) {
        self.push_u64(u64::try_from(value.len()).unwrap_or(u64::MAX));
        self.bytes.extend_from_slice(value);
    }

    pub(super) fn push_bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    pub(super) fn push_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(super) fn push_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub(super) fn push_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub(super) fn push_usize(&mut self, value: usize) {
        self.push_u64(u64::try_from(value).unwrap_or(u64::MAX));
    }

    pub(super) fn finish(self) -> Digest {
        Digest::sha256(&self.bytes)
    }
}
