#[cfg(target_os = "linux")]
use std::{
    ffi::{c_int, c_void},
    fs::File,
    os::fd::AsRawFd as _,
    ptr, thread,
    time::Duration,
};

#[cfg(target_os = "linux")]
const PROT_READ: c_int = 1;
#[cfg(target_os = "linux")]
const MAP_PRIVATE: c_int = 2;

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn mmap(
        address: *mut c_void,
        length: usize,
        protection: c_int,
        flags: c_int,
        descriptor: c_int,
        offset: isize,
    ) -> *mut c_void;
}

#[cfg(target_os = "linux")]
fn main() {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    let model_index = arguments
        .iter()
        .position(|argument| argument == "--model")
        .expect("fixture command contains a model flag");
    let model_path = arguments
        .get(model_index + 1)
        .expect("fixture command contains a model path");
    let model = File::open(model_path).expect("open private model fixture");
    let model_bytes = usize::try_from(model.metadata().expect("read private model metadata").len())
        .expect("model length fits the target address space");
    assert_ne!(model_bytes, 0);
    // SAFETY: the descriptor is a retained regular file, the nonzero mapping
    // length equals its complete size, and the mapping remains live until exit.
    let mapping = unsafe {
        mmap(
            ptr::null_mut(),
            model_bytes,
            PROT_READ,
            MAP_PRIVATE,
            model.as_raw_fd(),
            0,
        )
    };
    assert_ne!(
        mapping,
        usize::MAX as *mut c_void,
        "map private model fixture"
    );
    std::hint::black_box(mapping);
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}
