use std::{fs::File, os::fd::BorrowedFd, time::Instant};

use crate::ControlledBuildInputFile;

use super::super::{
    linux_build_protocol::{InputDeclaration, decode_input_declarations},
    linux_control::{MessageKind, receive},
    linux_helper_setup::HelperFailure,
};
use super::deadlines::startup_control_failure;
pub(super) fn receive_input_files(
    control: BorrowedFd<'_>,
    input_count: usize,
    deadline: Instant,
) -> Result<(Vec<ControlledBuildInputFile>, Vec<InputDeclaration>), HelperFailure> {
    let mut input_files = Vec::with_capacity(input_count);
    let mut retained_declarations = Vec::with_capacity(input_count);
    while input_files.len() < input_count {
        let message = receive(control, deadline, None).map_err(startup_control_failure)?;
        if message.kind != MessageKind::BuildInputFiles {
            return Err(HelperFailure::InvalidLaunch);
        }
        let declarations =
            decode_input_declarations(&message.payload).ok_or(HelperFailure::InvalidLaunch)?;
        if declarations.len() != message.descriptors.len()
            || declarations.len() > input_count.saturating_sub(input_files.len())
        {
            return Err(HelperFailure::InvalidLaunch);
        }
        for (declaration, descriptor) in declarations.into_iter().zip(message.descriptors) {
            let input = ControlledBuildInputFile::new(
                declaration.relative_path.clone(),
                declaration.expected_digest.clone(),
                declaration.expected_bytes,
                File::from(descriptor),
            )
            .map_err(|_| HelperFailure::InvalidLaunch)?;
            retained_declarations.push(declaration);
            input_files.push(input);
        }
    }
    Ok((input_files, retained_declarations))
}
