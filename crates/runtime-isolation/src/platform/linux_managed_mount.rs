use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
};

use rustix::{
    mount::{
        MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_bind, mount_change,
        mount_remount, unmount,
    },
    process::getpid,
};

use crate::contract::RetainedRuntimeInputMember;
use crate::{IsolationError, IsolationResult, ManagedDeviceBoundaryEvidence, NamespaceIdentity};

use super::linux_helper_setup::HelperFailure;

const NULL_MAJOR: u32 = 1;
const NULL_MINOR: u32 = 3;
const MAXIMUM_MOUNT_INFO_BYTES: usize = 1024 * 1024;
const NULL_STAGE_PATH: &str = "/tmp/retonr-managed-retained-null";

pub(super) fn open_retained_null() -> Result<File, HelperFailure> {
    let null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryUnavailable)?;
    require_null(&null, HelperFailure::ManagedDeviceBoundaryUnavailable)?;
    Ok(null)
}

pub(super) fn establish_private_device(
    null: &File,
    inputs: &[RetainedRuntimeInputMember],
) -> Result<(), HelperFailure> {
    mount_change(
        "/",
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    )
    .map_err(|_| HelperFailure::ManagedPrivatePropagationSetup)?;
    super::linux_managed_input_mount::stage(inputs)?;
    require_stage_path_absent()?;
    let stage_options = CString::new("size=1048576,nr_inodes=512,mode=0700")
        .map_err(|_| HelperFailure::ManagedDeviceBoundarySetup)?;
    mount(
        "tmpfs",
        "/tmp",
        "tmpfs",
        MountFlags::NODEV | MountFlags::NOSUID | MountFlags::NOEXEC,
        Some(stage_options.as_c_str()),
    )
    .map_err(|_| HelperFailure::ManagedNullStageMount)?;
    let stage = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(NULL_STAGE_PATH)
        .map_err(|_| HelperFailure::ManagedNullStagingSetup)?;
    drop(stage);
    mount_bind("/dev/null", NULL_STAGE_PATH).map_err(|_| HelperFailure::ManagedNullBind)?;
    let staged = File::options()
        .read(true)
        .write(true)
        .open(NULL_STAGE_PATH)
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?;
    require_same_object(null, &staged)?;
    let options = CString::new("size=65536,nr_inodes=4,mode=0755")
        .map_err(|_| HelperFailure::ManagedDeviceBoundarySetup)?;
    mount(
        "tmpfs",
        "/dev",
        "tmpfs",
        MountFlags::NODEV | MountFlags::NOSUID | MountFlags::NOEXEC,
        Some(options.as_c_str()),
    )
    .map_err(|_| HelperFailure::ManagedDeviceTmpfsSetup)?;
    let placeholder = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o666)
        .open("/dev/null")
        .map_err(|_| HelperFailure::ManagedNullStagingSetup)?;
    drop(placeholder);
    fs::set_permissions("/dev/null", fs::Permissions::from_mode(0o666))
        .map_err(|_| HelperFailure::ManagedNullStagingSetup)?;
    mount_bind(NULL_STAGE_PATH, "/dev/null").map_err(|_| HelperFailure::ManagedNullBind)?;
    mount_remount(
        "/dev/null",
        MountFlags::BIND | MountFlags::NOSUID | MountFlags::NOEXEC,
        "",
    )
    .map_err(|_| HelperFailure::ManagedNullMountRemount)?;
    let mut mounted = File::options()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?;
    require_null(&mounted, HelperFailure::ManagedDeviceBoundaryBehavior)?;
    require_same_object(null, &mounted)?;
    require_null_io(&mut mounted)?;
    drop(staged);
    unmount(NULL_STAGE_PATH, UnmountFlags::empty())
        .map_err(|_| HelperFailure::ManagedNullStagingSetup)?;
    fs::remove_file(NULL_STAGE_PATH).map_err(|_| HelperFailure::ManagedNullStagingSetup)?;
    require_stage_path_absent()?;
    require_same_object(null, &mounted)?;
    super::linux_managed_input_mount::establish(inputs)?;
    if !device_mount_flags_are_exact(&read_mount_info("/proc/self/mountinfo")?) {
        return Err(HelperFailure::ManagedDeviceBoundaryBehavior);
    }
    require_exact_device_entries(
        Path::new("/dev"),
        HelperFailure::ManagedDeviceBoundaryBehavior,
    )
}

fn require_stage_path_absent() -> Result<(), HelperFailure> {
    match fs::symlink_metadata(NULL_STAGE_PATH) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(HelperFailure::ManagedNullStagingSetup),
    }
}

pub(super) fn mount_private_proc() -> Result<(), HelperFailure> {
    mount(
        "proc",
        "/proc",
        "proc",
        MountFlags::NODEV | MountFlags::NOSUID | MountFlags::NOEXEC,
        None,
    )
    .map_err(|_| HelperFailure::ManagedProcSetup)?;
    if getpid().as_raw_nonzero().get() != 1
        || !private_proc_pid_one_is_exact(Path::new("/proc/1/status"))
        || !mounts_are_private(&read_mount_info("/proc/self/mountinfo")?)
    {
        return Err(HelperFailure::ManagedDeviceBoundaryBehavior);
    }
    let mut numeric = fs::read_dir("/proc")
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
        .collect::<Vec<_>>();
    numeric.sort_unstable();
    if numeric == [1] {
        Ok(())
    } else {
        Err(HelperFailure::ManagedDeviceBoundaryBehavior)
    }
}

pub(super) fn observe_current() -> Result<ManagedDeviceBoundaryEvidence, HelperFailure> {
    let device_mount = identity("/dev", HelperFailure::ManagedDeviceBoundaryBehavior)?;
    let proc_mount = identity("/proc", HelperFailure::ManagedDeviceBoundaryBehavior)?;
    let mut null = File::options()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?;
    require_null(&null, HelperFailure::ManagedDeviceBoundaryBehavior)?;
    require_null_io(&mut null)?;
    require_exact_device_entries(
        Path::new("/dev"),
        HelperFailure::ManagedDeviceBoundaryBehavior,
    )?;
    let mount_info = read_mount_info("/proc/self/mountinfo")?;
    if !mounts_are_private(&mount_info) || !managed_mount_flags_are_exact(&mount_info) {
        return Err(HelperFailure::ManagedDeviceBoundaryBehavior);
    }
    let metadata = null
        .metadata()
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?;
    Ok(ManagedDeviceBoundaryEvidence::verified(
        device_mount,
        proc_mount,
        NamespaceIdentity::new(metadata.dev(), metadata.ino()),
        libc::major(metadata.rdev()),
        libc::minor(metadata.rdev()),
        1,
    ))
}

pub(super) fn reobserve(
    target_pid: u32,
    expected: &ManagedDeviceBoundaryEvidence,
) -> IsolationResult<()> {
    let root = PathBuf::from(format!("/proc/{target_pid}/root"));
    let device = root.join("dev");
    let proc = root.join("proc");
    let null_path = device.join("null");
    let mut null = File::options()
        .read(true)
        .write(true)
        .open(&null_path)
        .map_err(|_| IsolationError::EvidenceChanged)?;
    null.write_all(b"managed-device-boundary")
        .map_err(|_| IsolationError::EvidenceChanged)?;
    let mut byte = [0_u8; 1];
    if null
        .read(&mut byte)
        .map_err(|_| IsolationError::EvidenceChanged)?
        != 0
    {
        return Err(IsolationError::EvidenceChanged);
    }
    let null_metadata = null
        .metadata()
        .map_err(|_| IsolationError::EvidenceChanged)?;
    let device_metadata = fs::metadata(&device).map_err(|_| IsolationError::EvidenceChanged)?;
    let proc_metadata = fs::metadata(&proc).map_err(|_| IsolationError::EvidenceChanged)?;
    let mount_info = read_bounded_mount_info(&format!("/proc/{target_pid}/mountinfo"))
        .map_err(|()| IsolationError::EvidenceChanged)?;
    if !expected.all_visibility_canaries_passed()
        || identity_from(&device_metadata) != expected.device_mount()
        || identity_from(&proc_metadata) != expected.proc_mount()
        || identity_from(&null_metadata) != expected.null_device()
        || (
            libc::major(null_metadata.rdev()),
            libc::minor(null_metadata.rdev()),
        ) != expected.null_device_numbers()
        || require_exact_device_entries(&device, HelperFailure::ManagedDeviceBoundaryBehavior)
            .is_err()
        || !mounts_are_private(&mount_info)
        || !managed_mount_flags_are_exact(&mount_info)
        || !private_proc_pid_one_is_exact(&proc.join("1/status"))
    {
        return Err(IsolationError::EvidenceChanged);
    }
    Ok(())
}

fn require_null(file: &File, failure: HelperFailure) -> Result<(), HelperFailure> {
    let metadata = file.metadata().map_err(|_| failure)?;
    if metadata.file_type().is_char_device()
        && libc::major(metadata.rdev()) == NULL_MAJOR
        && libc::minor(metadata.rdev()) == NULL_MINOR
    {
        Ok(())
    } else {
        Err(failure)
    }
}

fn require_null_io(file: &mut File) -> Result<(), HelperFailure> {
    file.write_all(b"managed-device-boundary")
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?;
    let mut byte = [0_u8; 1];
    if file
        .read(&mut byte)
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?
        == 0
    {
        Ok(())
    } else {
        Err(HelperFailure::ManagedDeviceBoundaryBehavior)
    }
}

fn require_same_object(expected: &File, actual: &File) -> Result<(), HelperFailure> {
    let expected = expected
        .metadata()
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?;
    let actual = actual
        .metadata()
        .map_err(|_| HelperFailure::ManagedDeviceBoundaryBehavior)?;
    if identity_from(&expected) == identity_from(&actual) && expected.rdev() == actual.rdev() {
        Ok(())
    } else {
        Err(HelperFailure::ManagedDeviceBoundaryBehavior)
    }
}

fn require_exact_device_entries(path: &Path, failure: HelperFailure) -> Result<(), HelperFailure> {
    let mut entries = fs::read_dir(path)
        .map_err(|_| failure)?
        .map(|entry| entry.map(|entry| entry.file_name()).map_err(|_| failure))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_unstable();
    if entries == ["null"] {
        Ok(())
    } else {
        Err(failure)
    }
}

fn read_mount_info(path: &str) -> Result<String, HelperFailure> {
    read_bounded_mount_info(path).map_err(|()| HelperFailure::ManagedDeviceBoundaryBehavior)
}

pub(super) fn read_bounded_mount_info(path: &str) -> Result<String, ()> {
    let file = File::open(path).map_err(|_| ())?;
    let limit = u64::try_from(MAXIMUM_MOUNT_INFO_BYTES + 1).map_err(|_| ())?;
    let mut bytes = Vec::with_capacity(MAXIMUM_MOUNT_INFO_BYTES.min(64 * 1024));
    file.take(limit).read_to_end(&mut bytes).map_err(|_| ())?;
    if bytes.len() > MAXIMUM_MOUNT_INFO_BYTES {
        return Err(());
    }
    String::from_utf8(bytes).map_err(|_| ())
}

pub(super) fn mounts_are_private(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAXIMUM_MOUNT_INFO_BYTES
        && text.lines().all(|line| {
            let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
            fields
                .iter()
                .position(|field| *field == "-")
                .is_some_and(|separator| {
                    separator >= 6
                        && fields[6..separator].iter().all(|field| {
                            !field.starts_with("shared:")
                                && !field.starts_with("master:")
                                && !field.starts_with("propagate_from:")
                        })
                })
        })
}

fn device_mount_flags_are_exact(text: &str) -> bool {
    mount_options(text, "/dev").is_some_and(|options| {
        ["rw", "nosuid", "nodev", "noexec"]
            .iter()
            .all(|required| options.contains(required))
    }) && mount_options(text, "/dev/null").is_some_and(|options| {
        ["rw", "nosuid", "noexec"]
            .iter()
            .all(|required| options.contains(required))
            && !options.contains(&"nodev")
    })
}

fn managed_mount_flags_are_exact(text: &str) -> bool {
    device_mount_flags_are_exact(text)
        && mount_options(text, "/proc").is_some_and(|options| {
            ["rw", "nosuid", "nodev", "noexec"]
                .iter()
                .all(|required| options.contains(required))
        })
}

pub(super) fn mount_options<'a>(text: &'a str, mount_point: &str) -> Option<Vec<&'a str>> {
    text.lines().rev().find_map(|line| {
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        (fields.len() >= 7 && fields[4] == mount_point)
            .then(|| fields[5].split(',').collect::<Vec<_>>())
    })
}

fn private_proc_pid_one_is_exact(path: &Path) -> bool {
    let Ok(status) = fs::read_to_string(path) else {
        return false;
    };
    let Some(pid_line) = status.lines().find_map(|line| line.strip_prefix("Pid:")) else {
        return false;
    };
    let Some(namespace_line) = status.lines().find_map(|line| line.strip_prefix("NSpid:")) else {
        return false;
    };
    let Ok(pid) = pid_line.trim().parse::<u32>() else {
        return false;
    };
    let Ok(namespace_values) = namespace_line
        .split_ascii_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    pid == 1 && namespace_values == [1]
}

fn identity(path: &str, failure: HelperFailure) -> Result<NamespaceIdentity, HelperFailure> {
    fs::metadata(path)
        .map(|metadata| identity_from(&metadata))
        .map_err(|_| failure)
}

fn identity_from(metadata: &fs::Metadata) -> NamespaceIdentity {
    NamespaceIdentity::new(metadata.dev(), metadata.ino())
}

#[cfg(test)]
mod tests {
    use super::{
        MAXIMUM_MOUNT_INFO_BYTES, device_mount_flags_are_exact, managed_mount_flags_are_exact,
        mounts_are_private, private_proc_pid_one_is_exact, read_bounded_mount_info,
    };
    use std::{fs, path::Path};

    #[test]
    fn mount_info_requires_every_mount_to_be_private_and_well_formed() {
        assert!(mounts_are_private("1 0 0:1 / / rw - rootfs rootfs rw\n"));
        assert!(!mounts_are_private(
            "1 0 0:1 / / rw shared:7 - rootfs rootfs rw\n"
        ));
        assert!(!mounts_are_private(
            "1 0 0:1 / / rw master:4 - rootfs rootfs rw\n"
        ));
        assert!(!mounts_are_private("malformed\n"));
        assert!(!mounts_are_private(""));
    }

    #[test]
    fn device_mount_flags_distinguish_nodev_parent_from_usable_null_child() {
        let exact = concat!(
            "0 0 0:0 / /dev rw,nosuid - devtmpfs devtmpfs rw\n",
            "1 0 0:1 / /dev rw,nosuid,nodev,noexec - tmpfs tmpfs rw\n",
            "2 1 0:2 /null /dev/null rw,nosuid,noexec - devtmpfs devtmpfs rw\n",
            "3 0 0:3 / /proc rw,nosuid,nodev,noexec - proc proc rw\n",
        );
        assert!(device_mount_flags_are_exact(exact));
        assert!(managed_mount_flags_are_exact(exact));
        assert!(!device_mount_flags_are_exact(&exact.replace(
            "/dev/null rw,nosuid,noexec",
            "/dev/null rw,nosuid,nodev,noexec"
        )));
        assert!(!managed_mount_flags_are_exact(&exact.replace(
            "/proc rw,nosuid,nodev,noexec",
            "/proc rw,nosuid,nodev"
        )));
    }

    #[test]
    fn fresh_proc_status_requires_exact_private_pid_one_identity() {
        let temporary = tempfile::tempdir().expect("temporary status root");
        let status = temporary.path().join("status");
        fs::write(&status, "Name:\thelper\nPid:\t1\nNSpid:\t1\n").expect("status fixture");
        assert!(private_proc_pid_one_is_exact(Path::new(&status)));
        for drifted in [
            "Name:\thelper\nPid:\t42\nNSpid:\t1\n",
            "Name:\thelper\nPid:\t1\nNSpid:\t42\t1\n",
            "Name:\thelper\nPid:\t1\nNSpid:\t2\n",
        ] {
            fs::write(&status, drifted).expect("drifted status");
            assert!(!private_proc_pid_one_is_exact(Path::new(&status)));
        }
    }

    #[test]
    fn mount_info_reader_rejects_over_limit_and_invalid_utf8_before_parsing() {
        let temporary = tempfile::tempdir().expect("temporary mount-info root");
        let mount_info = temporary.path().join("mountinfo");
        fs::write(&mount_info, vec![b'a'; MAXIMUM_MOUNT_INFO_BYTES]).expect("exact-limit fixture");
        assert_eq!(
            read_bounded_mount_info(mount_info.to_str().expect("UTF-8 fixture path"))
                .expect("read exact limit")
                .len(),
            MAXIMUM_MOUNT_INFO_BYTES
        );
        fs::write(&mount_info, vec![b'a'; MAXIMUM_MOUNT_INFO_BYTES + 1])
            .expect("over-limit fixture");
        assert!(read_bounded_mount_info(mount_info.to_str().expect("UTF-8 fixture path")).is_err());
        fs::write(&mount_info, [0xff]).expect("invalid UTF-8 fixture");
        assert!(read_bounded_mount_info(mount_info.to_str().expect("UTF-8 fixture path")).is_err());
    }
}
