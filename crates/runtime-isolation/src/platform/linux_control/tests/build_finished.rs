use std::os::fd::AsFd as _;

use crate::{
    ControlledBuildOutput, ControlledBuildOutputTree, ControlledBuildOutputTreeEntry,
    ControlledBuildProcessStatus, MAXIMUM_STARTUP_STREAM_BYTES, ManagedStartupOutput,
};
use rewrite_types::Digest;

use super::super::super::linux_build_protocol::{decode_finished, encode_finished};
use super::{ControlError, MessageKind, deadline, exact_header, pair, receive, send};

fn maximum_success() -> ControlledBuildOutput {
    let tree = ControlledBuildOutputTree::compile(vec![
        ControlledBuildOutputTreeEntry::regular_file("program", 4, Digest::sha256(b"ELF!"), 0o755)
            .expect("regular output"),
    ])
    .expect("output tree");
    ControlledBuildOutput::new(
        ControlledBuildProcessStatus::Success,
        ManagedStartupOutput::new(
            vec![b'o'; MAXIMUM_STARTUP_STREAM_BYTES],
            vec![b'e'; MAXIMUM_STARTUP_STREAM_BYTES],
            true,
            true,
        ),
    )
    .with_tree(tree)
}

#[test]
fn finished_build_transports_both_maximum_streams_and_verified_tree() {
    let output = maximum_success();
    let payload = encode_finished(&output).expect("encode finished output");
    assert_eq!(payload.len(), 2 * MAXIMUM_STARTUP_STREAM_BYTES + 96);
    for kind in [MessageKind::BuildFinished, MessageKind::BootstrapFinished] {
        let (sender, receiver) = pair().expect("control pair");
        send(sender.as_fd(), kind, &payload, &[], deadline(), None)
            .expect("send complete legal finished output");
        let frame = receive(receiver.as_fd(), deadline(), None).expect("receive finished output");
        assert_eq!(frame.kind, kind);
        assert_eq!(frame.payload, payload);
        assert_eq!(decode_finished(&frame.payload), Some(output.clone()));
    }
}

#[test]
fn only_finished_messages_accept_the_exact_larger_envelope() {
    let payload = encode_finished(&maximum_success()).expect("encode finished output");
    let (sender, receiver) = pair().expect("control pair");
    assert_eq!(
        send(
            sender.as_fd(),
            MessageKind::Captured,
            &payload,
            &[],
            deadline(),
            None
        ),
        Err(ControlError::Invalid)
    );
    for kind in [MessageKind::BuildFinished, MessageKind::BootstrapFinished] {
        let oversized = vec![0; kind.maximum_payload_bytes() + 1];
        assert_eq!(
            send(sender.as_fd(), kind, &oversized, &[], deadline(), None),
            Err(ControlError::Invalid)
        );
    }
    let mut frame = exact_header(
        MessageKind::Captured as u8,
        0,
        u32::try_from(payload.len()).expect("bounded payload"),
    )
    .to_vec();
    frame.extend_from_slice(&payload);
    rustix::net::send(sender.as_fd(), &frame, rustix::net::SendFlags::empty())
        .expect("send raw wrongly classified frame");
    assert!(matches!(
        receive(receiver.as_fd(), deadline(), None),
        Err(ControlError::Invalid)
    ));
}

#[test]
fn finished_frames_reject_truncation_oversize_and_false_lengths() {
    let payload = encode_finished(&maximum_success()).expect("encode finished output");
    for retained_bytes in [payload.len() - 1, payload.len() + 1] {
        let (sender, receiver) = pair().expect("control pair");
        let mut frame = exact_header(
            MessageKind::BuildFinished as u8,
            0,
            u32::try_from(payload.len()).expect("bounded payload"),
        )
        .to_vec();
        frame.extend_from_slice(&payload);
        frame.resize(16 + retained_bytes, 0);
        rustix::net::send(sender.as_fd(), &frame, rustix::net::SendFlags::empty())
            .expect("send raw malformed finished frame");
        assert!(matches!(
            receive(receiver.as_fd(), deadline(), None),
            Err(ControlError::Invalid)
        ));
    }
    let mut malformed = payload;
    malformed[7 + 80 + 8] = 4;
    assert!(decode_finished(&malformed).is_none());
}
