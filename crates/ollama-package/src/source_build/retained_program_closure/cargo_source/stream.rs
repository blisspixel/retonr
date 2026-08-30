use std::io::{self, Read};

use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use super::{CargoSourceClosureError, MemberMeasurement};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn checked_count(
    count: usize,
    maximum: usize,
) -> Result<usize, CargoSourceClosureError> {
    count
        .checked_add(1)
        .filter(|count| *count <= maximum)
        .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)
}

pub(super) fn append(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

pub(super) fn digest(hasher: Sha256) -> Digest {
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .expect("SHA-256 formatting always produces a valid digest")
}

pub(super) struct MeasuredReader<'a, R, C> {
    cancelled: &'a mut C,
    cancelled_observed: bool,
    expected: &'a MemberMeasurement,
    hasher: Sha256,
    inner: R,
    remaining: u64,
}

impl<'a, R, C> MeasuredReader<'a, R, C> {
    pub(super) fn new(inner: R, expected: &'a MemberMeasurement, cancelled: &'a mut C) -> Self {
        Self {
            cancelled,
            cancelled_observed: false,
            expected,
            hasher: Sha256::new(),
            inner,
            remaining: expected.bytes,
        }
    }
}

impl<R: Read, C: FnMut() -> bool> MeasuredReader<'_, R, C> {
    pub(super) const fn cancellation_observed(&self) -> bool {
        self.cancelled_observed
    }

    pub(super) fn validate_terminal_block(&mut self) -> Result<(), CargoSourceClosureError> {
        if self.remaining != 512 {
            return Err(CargoSourceClosureError::NoncanonicalArchive);
        }
        let mut terminal = [0_u8; 512];
        self.read_exact(&mut terminal)
            .map_err(|_| CargoSourceClosureError::InvalidArchive)?;
        if terminal.iter().any(|byte| *byte != 0) {
            return Err(CargoSourceClosureError::NoncanonicalArchive);
        }
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<(), CargoSourceClosureError> {
        let mut buffer = vec![0_u8; HASH_BUFFER_BYTES].into_boxed_slice();
        loop {
            let read = self.read(&mut buffer).map_err(|error| match error.kind() {
                io::ErrorKind::Interrupted => CargoSourceClosureError::Cancelled,
                _ => CargoSourceClosureError::ComponentMismatch,
            })?;
            if read == 0 {
                break;
            }
        }
        if self.remaining != 0 || digest(self.hasher) != self.expected.digest {
            Err(CargoSourceClosureError::ComponentMismatch)
        } else {
            Ok(())
        }
    }
}

impl<R: Read, C: FnMut() -> bool> Read for MeasuredReader<'_, R, C> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if (self.cancelled)() {
            self.cancelled_observed = true;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let read = self.inner.read(buffer)?;
        let read_u64 = u64::try_from(read).unwrap_or(u64::MAX);
        if read_u64 > self.remaining {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "component is longer than its measurement",
            ));
        }
        self.remaining -= read_u64;
        self.hasher.update(&buffer[..read]);
        Ok(read)
    }
}
