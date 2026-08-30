use std::io::{self, Write};

use serde::Serialize;

const INITIAL_CAPACITY_BYTES: usize = 8 * 1024;

pub(super) fn encode_bounded_json<T: Serialize>(
    value: &T,
    maximum_bytes: usize,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut writer = BoundedJsonWriter {
        bytes: Vec::with_capacity(maximum_bytes.min(INITIAL_CAPACITY_BYTES)),
        maximum_bytes,
    };
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.bytes)
}

struct BoundedJsonWriter {
    bytes: Vec<u8>,
    maximum_bytes: usize,
}

impl Write for BoundedJsonWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(buffer.len())
            .is_none_or(|length| length > self.maximum_bytes)
        {
            return Err(io::Error::other(
                "canonical JSON exceeds its fixed byte limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
