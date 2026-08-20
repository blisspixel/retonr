use std::{
    ffi::{OsString, c_void},
    fs::{File, OpenOptions},
    mem::size_of,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    os::windows::{
        ffi::OsStringExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::PathBuf,
    ptr,
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};
use windows_sys::Win32::{
    Foundation::{
        ERROR_ACCESS_DENIED, ERROR_INSUFFICIENT_BUFFER, FILETIME, NO_ERROR, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    },
    Networking::WinSock::{AF_INET, AF_INET6},
    System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        QueryFullProcessImageNameW, WaitForSingleObject,
    },
};

use super::file::hash_opened_file;
use crate::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, ListenerEndpoint, ensure_active,
};

const MAXIMUM_IMAGE_PATH_UNITS: usize = 32_768;
const MAXIMUM_TABLE_RETRIES: usize = 3;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
const FILE_SHARE_READ: u32 = 0x0000_0001;

pub(crate) struct Lease {
    endpoint: ListenerEndpoint,
    pid: u32,
    process: OwnedHandle,
    creation_time: u64,
    entrypoint: File,
    initial: AttachedProcessEvidence,
}

impl Lease {
    pub(crate) fn attach(
        endpoint: ListenerEndpoint,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<Self, AttachedProcessWitnessError> {
        let pid = listener_owner(endpoint.socket(), limits, cancellation, started)?;
        let process = open_process(pid)?;
        ensure_process_alive(&process)?;
        let creation_time = process_creation_time(&process)?;
        let path = process_image_path(&process)?;
        let mut entrypoint = open_entrypoint(&path)?;
        let initial = observe_process(
            endpoint,
            pid,
            creation_time,
            &mut entrypoint,
            limits,
            cancellation,
            started,
        )?;
        let confirmed = listener_owner(endpoint.socket(), limits, cancellation, started)?;
        if confirmed != pid {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        ensure_process_alive(&process)?;
        if process_creation_time(&process)? != creation_time {
            return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
        }
        Ok(Self {
            endpoint,
            pid,
            process,
            creation_time,
            entrypoint,
            initial,
        })
    }

    pub(crate) fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.initial
    }

    pub(crate) fn reobserve(
        &mut self,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        ensure_process_alive(&self.process)?;
        if process_creation_time(&self.process)? != self.creation_time {
            return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
        }
        let owner = listener_owner(self.endpoint.socket(), limits, cancellation, started)?;
        if owner != self.pid {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        let retained = entrypoint_metadata(&self.entrypoint)?;
        let retained_digest = hash_opened_file(
            &mut self.entrypoint,
            retained.length,
            limits,
            cancellation,
            started,
        )?;
        if retained_digest != *self.initial.entrypoint_digest() {
            return Err(AttachedProcessWitnessError::EntrypointChanged);
        }
        let path = process_image_path(&self.process)?;
        let mut current = open_entrypoint(&path)?;
        let observed = observe_process(
            self.endpoint,
            self.pid,
            self.creation_time,
            &mut current,
            limits,
            cancellation,
            started,
        )?;
        ensure_process_alive(&self.process)?;
        Ok(observed)
    }
}

fn listener_owner(
    endpoint: SocketAddr,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<u32, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    let table = tcp_table(endpoint.ip(), limits)?;
    let owners = matching_owners(&table, endpoint, limits)?;
    match owners.as_slice() {
        [] => Err(AttachedProcessWitnessError::ListenerNotFound),
        [owner] if *owner != 0 => Ok(*owner),
        [_] => Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        _ => Err(AttachedProcessWitnessError::ListenerOwnershipAmbiguous),
    }
}

fn tcp_table(
    ip: IpAddr,
    limits: AttachedProcessWitnessLimits,
) -> Result<Vec<u32>, AttachedProcessWitnessError> {
    let family = match ip {
        IpAddr::V4(_) => u32::from(AF_INET),
        IpAddr::V6(_) => u32::from(AF_INET6),
    };
    let mut required = 0_u32;
    // SAFETY: The first documented sizing call uses a null output buffer and a
    // valid writable size pointer. No Rust object is aliased through the null pointer.
    let first = unsafe {
        GetExtendedTcpTable(
            ptr::null_mut(),
            &raw mut required,
            0,
            family,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if first != ERROR_INSUFFICIENT_BUFFER && first != NO_ERROR {
        return Err(native_error(first));
    }
    for _ in 0..MAXIMUM_TABLE_RETRIES {
        let required_usize = usize::try_from(required)
            .map_err(|_error| AttachedProcessWitnessError::ResourceLimit)?;
        if required_usize == 0 || required_usize > limits.maximum_socket_table_bytes {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        let words = required_usize
            .checked_add(size_of::<u32>() - 1)
            .and_then(|bytes| bytes.checked_div(size_of::<u32>()))
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        let mut table = vec![0_u32; words];
        let mut available = u32::try_from(words * size_of::<u32>())
            .map_err(|_error| AttachedProcessWitnessError::ResourceLimit)?;
        // SAFETY: `table` is writable for `available` bytes and aligned to at
        // least four bytes, which satisfies every owner-PID table row field.
        let status = unsafe {
            GetExtendedTcpTable(
                table.as_mut_ptr().cast::<c_void>(),
                &raw mut available,
                0,
                family,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if status == NO_ERROR {
            return Ok(table);
        }
        if status != ERROR_INSUFFICIENT_BUFFER {
            return Err(native_error(status));
        }
        required = available;
    }
    Err(AttachedProcessWitnessError::ResourceLimit)
}

fn matching_owners(
    table: &[u32],
    endpoint: SocketAddr,
    limits: AttachedProcessWitnessLimits,
) -> Result<Vec<u32>, AttachedProcessWitnessError> {
    let bytes = table.len().saturating_mul(size_of::<u32>());
    if bytes < size_of::<u32>() {
        return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
    }
    let rows =
        usize::try_from(table[0]).map_err(|_error| AttachedProcessWitnessError::ResourceLimit)?;
    if rows > limits.maximum_socket_table_entries {
        return Err(AttachedProcessWitnessError::ResourceLimit);
    }
    match endpoint.ip() {
        IpAddr::V4(expected) => matching_ipv4_owners(table, rows, bytes, expected, endpoint.port()),
        IpAddr::V6(expected) => matching_ipv6_owners(table, rows, bytes, expected, endpoint.port()),
    }
}

fn matching_ipv4_owners(
    table: &[u32],
    rows: usize,
    bytes: usize,
    expected: Ipv4Addr,
    expected_port: u16,
) -> Result<Vec<u32>, AttachedProcessWitnessError> {
    let row_size = size_of::<MIB_TCPROW_OWNER_PID>();
    validate_table_extent(rows, row_size, bytes)?;
    let base = table.as_ptr().cast::<u8>();
    let mut owners = Vec::new();
    for index in 0..rows {
        let offset = size_of::<u32>() + index * row_size;
        // SAFETY: `validate_table_extent` proves the complete row is within
        // `table`; `read_unaligned` does not require a stronger row alignment.
        let row = unsafe { ptr::read_unaligned(base.add(offset).cast::<MIB_TCPROW_OWNER_PID>()) };
        let address = Ipv4Addr::from(u32::from_be(row.dwLocalAddr));
        let port = u16::from_be(
            u16::try_from(row.dwLocalPort)
                .map_err(|_| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?,
        );
        if address == expected && port == expected_port {
            owners.push(row.dwOwningPid);
        }
    }
    Ok(owners)
}

fn matching_ipv6_owners(
    table: &[u32],
    rows: usize,
    bytes: usize,
    expected: Ipv6Addr,
    expected_port: u16,
) -> Result<Vec<u32>, AttachedProcessWitnessError> {
    let row_size = size_of::<MIB_TCP6ROW_OWNER_PID>();
    validate_table_extent(rows, row_size, bytes)?;
    let base = table.as_ptr().cast::<u8>();
    let mut owners = Vec::new();
    for index in 0..rows {
        let offset = size_of::<u32>() + index * row_size;
        // SAFETY: `validate_table_extent` proves the complete row is within
        // `table`; `read_unaligned` does not require a stronger row alignment.
        let row = unsafe { ptr::read_unaligned(base.add(offset).cast::<MIB_TCP6ROW_OWNER_PID>()) };
        let address = Ipv6Addr::from(row.ucLocalAddr);
        let port = u16::from_be(
            u16::try_from(row.dwLocalPort)
                .map_err(|_| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?,
        );
        if address == expected && port == expected_port {
            owners.push(row.dwOwningPid);
        }
    }
    Ok(owners)
}

fn validate_table_extent(
    rows: usize,
    row_size: usize,
    bytes: usize,
) -> Result<(), AttachedProcessWitnessError> {
    let required = rows
        .checked_mul(row_size)
        .and_then(|size| size.checked_add(size_of::<u32>()))
        .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
    if required > bytes {
        return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
    }
    Ok(())
}

fn open_process(pid: u32) -> Result<OwnedHandle, AttachedProcessWitnessError> {
    // SAFETY: The PID is kernel-reported and the access mask is read-only. The
    // returned owned handle is closed exactly once by `OwnedHandle`.
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if handle.is_null() {
        let code = std::io::Error::last_os_error().raw_os_error();
        return Err(if code == Some(ERROR_ACCESS_DENIED.cast_signed()) {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else {
            AttachedProcessWitnessError::ProcessInstanceUnavailable
        });
    }
    // SAFETY: `OpenProcess` returned a fresh non-null owned process handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

fn ensure_process_alive(process: &OwnedHandle) -> Result<(), AttachedProcessWitnessError> {
    // SAFETY: The handle remains owned and valid for this call; timeout zero is
    // nonblocking and does not mutate caller memory.
    match unsafe { WaitForSingleObject(process.as_raw_handle(), 0) } {
        WAIT_TIMEOUT => Ok(()),
        WAIT_OBJECT_0 => Err(AttachedProcessWitnessError::ProcessExited),
        _ => Err(AttachedProcessWitnessError::ProcessInstanceUnavailable),
    }
}

fn process_creation_time(process: &OwnedHandle) -> Result<u64, AttachedProcessWitnessError> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: Every pointer refers to a distinct initialized writable FILETIME
    // and the retained process handle is valid for query access.
    let success = unsafe {
        GetProcessTimes(
            process.as_raw_handle(),
            &raw mut creation,
            &raw mut exit,
            &raw mut kernel,
            &raw mut user,
        )
    };
    if success == 0 {
        return Err(AttachedProcessWitnessError::ProcessInstanceUnavailable);
    }
    let value = (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
    if value == 0 {
        return Err(AttachedProcessWitnessError::ProcessInstanceUnavailable);
    }
    Ok(value)
}

fn process_image_path(process: &OwnedHandle) -> Result<PathBuf, AttachedProcessWitnessError> {
    let mut path = vec![0_u16; MAXIMUM_IMAGE_PATH_UNITS];
    let mut length = u32::try_from(path.len())
        .map_err(|_error| AttachedProcessWitnessError::EntrypointUnavailable)?;
    // SAFETY: `path` is writable for `length` UTF-16 units and the retained
    // process handle has query access.
    let success = unsafe {
        QueryFullProcessImageNameW(
            process.as_raw_handle(),
            0,
            path.as_mut_ptr(),
            &raw mut length,
        )
    };
    if success == 0 || length == 0 {
        return Err(AttachedProcessWitnessError::EntrypointUnavailable);
    }
    let used = usize::try_from(length)
        .map_err(|_error| AttachedProcessWitnessError::EntrypointUnavailable)?;
    path.truncate(used);
    Ok(PathBuf::from(OsString::from_wide(&path)))
}

fn open_entrypoint(path: &PathBuf) -> Result<File, AttachedProcessWitnessError> {
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                AttachedProcessWitnessError::ProcessAccessDenied
            } else {
                AttachedProcessWitnessError::EntrypointUnavailable
            }
        })
}

fn entrypoint_metadata(file: &File) -> Result<WindowsFileEvidence, AttachedProcessWitnessError> {
    let metadata = file
        .metadata()
        .map_err(|_error| AttachedProcessWitnessError::EntrypointUnavailable)?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(AttachedProcessWitnessError::EntrypointNotRegular);
    }
    let information = winx::winapi_util::file::information(file)
        .map_err(|_error| AttachedProcessWitnessError::EntrypointUnavailable)?;
    if information.number_of_links() != 1 {
        return Err(AttachedProcessWitnessError::EntrypointAliased);
    }
    Ok(WindowsFileEvidence {
        length: metadata.len(),
        volume_serial: information.volume_serial_number(),
        file_index: information.file_index(),
        creation_time: metadata.creation_time(),
        last_write_time: metadata.last_write_time(),
    })
}

fn observe_process(
    endpoint: ListenerEndpoint,
    pid: u32,
    creation_time: u64,
    entrypoint: &mut File,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    let metadata = entrypoint_metadata(entrypoint)?;
    let entrypoint_digest =
        hash_opened_file(entrypoint, metadata.length, limits, cancellation, started)?;
    let process_material = format!("windows-pid-creation-v1\0{pid}\0{creation_time}");
    let ownership_material = format!(
        "windows-listener-owner-v1\0{}\0{pid}\0{creation_time}",
        endpoint.socket()
    );
    let object_material = format!(
        "windows-file-object-v1\0{}\0{}\0{}\0{}\0{}",
        metadata.volume_serial,
        metadata.file_index,
        metadata.length,
        metadata.creation_time,
        metadata.last_write_time
    );
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::WindowsOwnerPidProcessHandle,
        owner_pid: pid,
        process_instance_digest: Digest::sha256(process_material.as_bytes()),
        ownership_snapshot_digest: Digest::sha256(ownership_material.as_bytes()),
        entrypoint_object_digest: Digest::sha256(object_material.as_bytes()),
        entrypoint_digest,
        entrypoint_bytes: metadata.length,
        platform_evidence_digest: Digest::sha256(b"windows-public-owner-pid-process-handle-v1"),
    })
}

fn native_error(code: u32) -> AttachedProcessWitnessError {
    if code == ERROR_ACCESS_DENIED {
        AttachedProcessWitnessError::ProcessAccessDenied
    } else {
        AttachedProcessWitnessError::PlatformObservationFailed
    }
}

struct WindowsFileEvidence {
    length: u64,
    volume_serial: u64,
    file_index: u64,
    creation_time: u64,
    last_write_time: u64,
}
