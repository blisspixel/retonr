use super::*;
use std::time::{Duration, Instant};

fn receive<R: Send + 'static>(worker: &Worker<R>) -> Response {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(response) = worker.response() {
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "bounded worker response deadline"
        );
        thread::yield_now();
    }
}

#[test]
fn one_operation_and_one_response_are_retained_and_tagged() {
    let (entered, entry) = mpsc::sync_channel(1);
    let (resume, pause) = mpsc::sync_channel(1);
    let mut worker = Worker::new(move |text: String, _| {
        entered.send(()).expect("entry");
        pause.recv().expect("resume");
        Ok(Snapshot {
            source: text,
            ..Snapshot::default()
        })
    })
    .expect("worker");
    assert!(worker.response().is_none());
    assert!(worker.submit(7, "first".into()));
    entry
        .recv_timeout(Duration::from_secs(3))
        .expect("worker entered");
    assert!(!worker.submit(8, "second".into()));
    resume.send(()).expect("resume operation");
    let response = receive(&worker);
    assert_eq!(response.operation, 7);
    assert!(!response.stopped);
    assert_eq!(response.result.expect("snapshot").source, "first");
    assert!(worker.submit(9, "next".into()));
    entry
        .recv_timeout(Duration::from_secs(3))
        .expect("second entered");
    resume.send(()).expect("resume second");
    assert_eq!(receive(&worker).operation, 9);
    worker.shutdown();
    assert!(!worker.submit(10, "closed".into()));
    assert!(worker.response().is_none());
}

#[test]
fn operation_errors_and_shutdown_are_bounded_and_cooperative() {
    let mut worker = Worker::new(|(): (), _| Err("read refused")).expect("worker");
    assert!(worker.submit(1, ()));
    assert_eq!(receive(&worker).result.err(), Some("read refused"));
    worker.shutdown();
    worker.shutdown();
    let (entered, entry) = mpsc::sync_channel(1);
    let mut worker = Worker::new(move |(): (), cancellation| {
        entered.send(()).expect("entry");
        while !cancellation.is_cancelled() {
            thread::yield_now();
        }
        Err("cancelled")
    })
    .expect("cancellable worker");
    assert!(worker.submit(2, ()));
    entry.recv_timeout(Duration::from_secs(3)).expect("entered");
    worker.shutdown();
    let _ = worker.response();
    assert!(worker.response().is_none());
}

#[test]
fn a_stopped_worker_reports_its_operation_without_leaving_permanent_loading_state() {
    let worker = Worker::new(|(): (), _| panic!("scripted reader unwind")).expect("worker");
    assert!(worker.submit(23, ()));
    let response = receive(&worker);
    assert_eq!(response.operation, 23);
    assert!(response.stopped);
    assert!(response.result.is_err());
    assert!(worker.response().is_none());
    assert!(!worker.submit(24, ()));
}
