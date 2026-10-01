use super::*;
use rustix::net::connect;
use std::{
    io::{Read as _, Write as _},
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
};

#[test]
fn kernel_self_connected_socket_cannot_be_transferred_as_a_target_channel() {
    let descriptor = socket_with(
        AddressFamily::INET,
        SocketType::STREAM,
        SocketFlags::CLOEXEC,
        Some(rustix::net::ipproto::TCP),
    )
    .expect("TCP socket");
    bind(&descriptor, &SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).expect("private ephemeral bind");
    let endpoint = SocketAddrV4::try_from(getsockname(&descriptor).expect("bound endpoint"))
        .expect("IPv4 endpoint");
    connect(&descriptor, &endpoint).expect("actual kernel self-connection without a listener");
    let mut stream = TcpStream::from(descriptor);
    assert_eq!(
        stream.local_addr().expect("local"),
        SocketAddr::V4(endpoint)
    );
    assert_eq!(stream.peer_addr().expect("peer"), SocketAddr::V4(endpoint));
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("bounded kernel fixture");
    stream.write_all(b"looped bytes").expect("write to self");
    let mut returned = [0; 12];
    stream
        .read_exact(&mut returned)
        .expect("self receives its own bytes");
    assert_eq!(&returned, b"looped bytes");
    assert!(!has_distinct_peer(&stream, endpoint.into()).expect("valid loopback shape"));
}

#[test]
fn actual_listener_connection_is_accepted_only_for_the_exact_endpoint() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("listener");
    let endpoint = listener.local_addr().expect("endpoint");
    let stream = TcpStream::connect_timeout(&endpoint, Duration::from_secs(1))
        .expect("real listener connection");
    let (_accepted, peer) = listener.accept().expect("accepted peer");
    assert_eq!(stream.local_addr().expect("client"), peer);
    assert_ne!(peer, endpoint);
    assert!(has_distinct_peer(&stream, endpoint).expect("distinct target"));
    let wrong = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    assert!(has_distinct_peer(&stream, wrong).is_err());
}

#[test]
fn expired_original_channel_deadline_never_attempts_a_connection() {
    let mut child = std::process::Command::new("/bin/sh")
        .args(["-c", "exit 0"])
        .spawn()
        .expect("child");
    let result = connect_exact_loopback(
        &mut child,
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        Instant::now(),
    );
    assert!(result.is_err());
    child.wait().expect("reaped fixture");
}
