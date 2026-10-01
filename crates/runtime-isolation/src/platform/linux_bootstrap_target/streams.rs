use crate::{MAXIMUM_STARTUP_STREAM_BYTES, ManagedStartupOutput};

#[derive(Default)]
pub(super) struct Capture {
    output: Vec<u8>,
    error: Vec<u8>,
    output_truncated: bool,
    error_truncated: bool,
}

impl Capture {
    pub(super) fn append(&mut self, streams: &ManagedStartupOutput) {
        self.output_truncated |= streams.standard_output_truncated()
            | append(&mut self.output, streams.standard_output());
        self.error_truncated |=
            streams.standard_error_truncated() | append(&mut self.error, streams.standard_error());
    }

    pub(super) fn finish(self) -> ManagedStartupOutput {
        ManagedStartupOutput::new(
            self.output,
            self.error,
            self.output_truncated,
            self.error_truncated,
        )
    }
}

fn append(destination: &mut Vec<u8>, source: &[u8]) -> bool {
    let retained = MAXIMUM_STARTUP_STREAM_BYTES
        .saturating_sub(destination.len())
        .min(source.len());
    destination.extend_from_slice(&source[..retained]);
    retained != source.len()
}
