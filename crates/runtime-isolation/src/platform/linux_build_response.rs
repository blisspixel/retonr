use std::process::ExitStatus;

use crate::{ControlledBuildOutput, IsolationError, IsolationResult};

use super::{
    linux_build_protocol::{decode_finished, decode_helper_failure},
    linux_control::{ControlMessage, MessageKind},
};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum BuildResponse {
    Finished(ControlledBuildOutput),
    Failed(IsolationError),
}

impl BuildResponse {
    pub(super) fn complete(
        self,
        helper_status: ExitStatus,
    ) -> IsolationResult<ControlledBuildOutput> {
        match self {
            Self::Finished(output) if helper_status.success() => Ok(output),
            Self::Finished(_output) => Err(IsolationError::HelperProtocol),
            Self::Failed(error) => Err(error),
        }
    }
}

pub(super) fn decode(message: &ControlMessage) -> IsolationResult<BuildResponse> {
    decode_for_kind(message, MessageKind::BuildFinished)
}

pub(super) fn decode_bootstrap(message: &ControlMessage) -> IsolationResult<BuildResponse> {
    decode_for_kind(message, MessageKind::BootstrapFinished)
}

fn decode_for_kind(
    message: &ControlMessage,
    finished_kind: MessageKind,
) -> IsolationResult<BuildResponse> {
    if !message.descriptors.is_empty() {
        return Err(IsolationError::HelperProtocol);
    }
    match message.kind {
        kind if kind == finished_kind => decode_finished(&message.payload)
            .map(BuildResponse::Finished)
            .ok_or(IsolationError::HelperProtocol),
        MessageKind::Error => decode_helper_failure(&message.payload)
            .map(BuildResponse::Failed)
            .ok_or(IsolationError::HelperProtocol),
        _ => Err(IsolationError::HelperProtocol),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        os::{fd::AsFd as _, unix::process::ExitStatusExt as _},
        time::{Duration, Instant},
    };

    use super::{BuildResponse, decode};
    use crate::{
        ControlledBuildOutput, ControlledBuildProcessStatus, IsolationError, ManagedStartupOutput,
        platform::{
            linux_build_protocol::{encode_finished, encode_helper_failure},
            linux_control::{ControlMessage, MessageKind, pair, receive, send},
            linux_helper_setup::HelperFailure,
        },
    };

    #[test]
    fn late_helper_failure_is_decoded_without_losing_its_type() {
        let (helper, parent) = pair().expect("control pair");
        let payload = encode_helper_failure(HelperFailure::FilesystemAliasOutput);
        send(
            helper.as_fd(),
            MessageKind::Error,
            &payload,
            &[],
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("send late failure");
        let received = receive(
            parent.as_fd(),
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("receive late failure");
        let response = decode(&received);
        assert_eq!(
            response,
            Ok(BuildResponse::Failed(IsolationError::FilesystemAliasSetup(
                "output"
            )))
        );
        assert_eq!(
            decode(&ControlMessage {
                kind: MessageKind::Error,
                payload: b"\x01controlled-build-object-mismatch\n".to_vec(),
                descriptors: Vec::new(),
            }),
            Err(IsolationError::HelperProtocol)
        );

        let output = ControlledBuildOutput::new(
            ControlledBuildProcessStatus::ExitCode(7),
            ManagedStartupOutput::new(Vec::new(), Vec::new(), false, false),
        );
        assert_eq!(
            decode(&ControlMessage {
                kind: MessageKind::BuildFinished,
                payload: encode_finished(&output).expect("finished payload"),
                descriptors: Vec::new(),
            }),
            Ok(BuildResponse::Finished(output))
        );

        let (descriptor, _peer) = pair().expect("descriptor pair");
        assert_eq!(
            decode(&ControlMessage {
                kind: MessageKind::Error,
                payload,
                descriptors: vec![descriptor],
            }),
            Err(IsolationError::HelperProtocol)
        );
    }

    #[test]
    fn finished_response_requires_a_successful_helper_exit() {
        let output = || {
            ControlledBuildOutput::new(
                ControlledBuildProcessStatus::ExitCode(7),
                ManagedStartupOutput::new(Vec::new(), Vec::new(), false, false),
            )
        };
        assert_eq!(
            BuildResponse::Finished(output()).complete(std::process::ExitStatus::from_raw(0)),
            Ok(output())
        );
        assert_eq!(
            BuildResponse::Finished(output()).complete(std::process::ExitStatus::from_raw(7 << 8)),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            BuildResponse::Finished(output())
                .complete(std::process::ExitStatus::from_raw(libc::SIGKILL)),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            BuildResponse::Failed(IsolationError::FilesystemIsolationBehavior)
                .complete(std::process::ExitStatus::from_raw(7 << 8)),
            Err(IsolationError::FilesystemIsolationBehavior)
        );
    }
}
