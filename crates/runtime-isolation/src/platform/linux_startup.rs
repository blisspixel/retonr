use std::{
    io::Read,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use crate::{MAXIMUM_STARTUP_STREAM_BYTES, ManagedStartupOutput};

const CAPTURE_HEADER_BYTES: usize = 9;

#[derive(Default)]
struct StreamCapture {
    prefix: Vec<u8>,
    truncated: bool,
}

pub(super) struct StartupDrains {
    standard_output: Arc<Mutex<StreamCapture>>,
    standard_error: Arc<Mutex<StreamCapture>>,
    #[cfg(test)]
    readers: Vec<JoinHandle<()>>,
}

impl StartupDrains {
    pub(super) fn start(
        standard_output: impl Read + Send + 'static,
        standard_error: impl Read + Send + 'static,
    ) -> Self {
        let output = Arc::new(Mutex::new(StreamCapture::default()));
        let error = Arc::new(Mutex::new(StreamCapture::default()));
        let output_reader = spawn_drain(standard_output, Arc::clone(&output));
        let error_reader = spawn_drain(standard_error, Arc::clone(&error));
        #[cfg(not(test))]
        {
            drop(output_reader);
            drop(error_reader);
        }
        Self {
            standard_output: output,
            standard_error: error,
            #[cfg(test)]
            readers: vec![output_reader, error_reader],
        }
    }

    pub(super) fn snapshot(&self) -> ManagedStartupOutput {
        let output = snapshot_stream(&self.standard_output);
        let error = snapshot_stream(&self.standard_error);
        ManagedStartupOutput::new(output.0, error.0, output.1, error.1)
    }

    #[cfg(test)]
    pub(super) fn finish(mut self) -> ManagedStartupOutput {
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
        self.snapshot()
    }
}

fn spawn_drain(
    mut source: impl Read + Send + 'static,
    capture: Arc<Mutex<StreamCapture>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            let Ok(read) = source.read(&mut buffer) else {
                return;
            };
            if read == 0 {
                return;
            }
            let Ok(mut state) = capture.lock() else {
                return;
            };
            let available = MAXIMUM_STARTUP_STREAM_BYTES.saturating_sub(state.prefix.len());
            let retained = available.min(read);
            state.prefix.extend_from_slice(&buffer[..retained]);
            state.truncated |= retained != read;
        }
    })
}

fn snapshot_stream(capture: &Mutex<StreamCapture>) -> (Vec<u8>, bool) {
    capture.lock().map_or_else(
        |_error| (Vec::new(), true),
        |value| (value.prefix.clone(), value.truncated),
    )
}

pub(super) fn encode(output: &ManagedStartupOutput) -> Option<Vec<u8>> {
    let stdout = u32::try_from(output.standard_output().len()).ok()?;
    let stderr = u32::try_from(output.standard_error().len()).ok()?;
    let mut bytes = Vec::with_capacity(
        CAPTURE_HEADER_BYTES
            .saturating_add(output.standard_output().len())
            .saturating_add(output.standard_error().len()),
    );
    bytes.extend_from_slice(&stdout.to_be_bytes());
    bytes.extend_from_slice(&stderr.to_be_bytes());
    bytes.push(
        u8::from(output.standard_output_truncated())
            | (u8::from(output.standard_error_truncated()) << 1),
    );
    bytes.extend_from_slice(output.standard_output());
    bytes.extend_from_slice(output.standard_error());
    Some(bytes)
}

pub(super) fn decode(bytes: &[u8]) -> Option<ManagedStartupOutput> {
    if bytes.len() < CAPTURE_HEADER_BYTES {
        return None;
    }
    let stdout = usize::try_from(u32::from_be_bytes(bytes[..4].try_into().ok()?)).ok()?;
    let stderr = usize::try_from(u32::from_be_bytes(bytes[4..8].try_into().ok()?)).ok()?;
    let flags = bytes[8];
    if flags & !0b11 != 0
        || stdout > MAXIMUM_STARTUP_STREAM_BYTES
        || stderr > MAXIMUM_STARTUP_STREAM_BYTES
        || CAPTURE_HEADER_BYTES
            .checked_add(stdout)?
            .checked_add(stderr)?
            != bytes.len()
    {
        return None;
    }
    let stdout_start = CAPTURE_HEADER_BYTES;
    let stderr_start = stdout_start + stdout;
    Some(ManagedStartupOutput::new(
        bytes[stdout_start..stderr_start].to_vec(),
        bytes[stderr_start..].to_vec(),
        flags & 1 != 0,
        flags & 2 != 0,
    ))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{StartupDrains, decode, encode};
    use crate::{MAXIMUM_STARTUP_STREAM_BYTES, ManagedStartupOutput};

    #[test]
    fn capture_codec_is_exact_and_rejects_trailing_or_oversized_lengths() {
        let output = ManagedStartupOutput::new(b"out".to_vec(), b"err".to_vec(), false, true);
        let encoded = encode(&output).expect("encode capture");
        assert_eq!(decode(&encoded), Some(output));

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_none());

        let mut oversized = encoded;
        oversized[..4].copy_from_slice(
            &u32::try_from(MAXIMUM_STARTUP_STREAM_BYTES + 1)
                .expect("bounded constant")
                .to_be_bytes(),
        );
        assert!(decode(&oversized).is_none());
    }

    #[test]
    fn drains_retain_bounded_prefixes_and_report_truncation() {
        let drains = StartupDrains::start(
            Cursor::new(vec![b'o'; MAXIMUM_STARTUP_STREAM_BYTES + 1]),
            Cursor::new(b"error".to_vec()),
        );
        let output = drains.finish();
        assert_eq!(output.standard_output().len(), MAXIMUM_STARTUP_STREAM_BYTES);
        assert!(output.standard_output().iter().all(|byte| *byte == b'o'));
        assert!(output.standard_output_truncated());
        assert_eq!(output.standard_error(), b"error");
        assert!(!output.standard_error_truncated());
    }
}
