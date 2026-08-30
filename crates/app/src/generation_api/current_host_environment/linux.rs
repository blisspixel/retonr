use std::collections::BTreeSet;
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rewrite_model::{
    ComputeBackend, ExecutionPlacement, HostAcceleratorScopeV1, HostArchitectureV1Input,
    HostEnvironmentV1, HostEnvironmentV1Input, HostExecutionClassV1Input, HostExecutionProfileV1,
    HostHardwareEnvelopeV1Input, HostOperatingSystemV1Input, ObserverBinaryAssertionModeV1,
    RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
};
use rewrite_types::CancellationToken;
use rustix::fs::{AtFlags, Mode, OFlags, StatxFlags};

use super::cgroup::{
    CgroupControlFile, require_initial_namespace_inode, require_returned_mount_id,
    validate_cgroup2_mount_root, validate_controls,
};
use super::parser::{
    MEMORY_ROUNDING_GRANULARITY_MIB, parse_cpu_info, parse_kernel_release, parse_online_cpu_list,
    parse_total_memory_mib, read_bounded_stream, validate_affinity as validate_affinity_projection,
};
use super::{CurrentHostEnvironmentError, CurrentHostEnvironmentSource, ensure_active};

const PROC_ROOT: &str = "/proc";
const SYS_ROOT: &str = "/sys";
const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const OS_RELEASE_PATH: &str = "sys/kernel/osrelease";
const ONLINE_CPUS_PATH: &str = "devices/system/cpu/online";
const CPU_INFO_PATH: &str = "cpuinfo";
const MEMORY_INFO_PATH: &str = "meminfo";
const THREAD_CGROUP_PATH: &str = "thread-self/cgroup";
const SELF_CGROUP_NAMESPACE_PATH: &str = "self/ns/cgroup";
const SELF_MOUNT_INFO_PATH: &str = "self/mountinfo";

const PROC_SUPER_MAGIC: u64 = 0x0000_9FA0;
const SYSFS_MAGIC: u64 = 0x6265_6572;
const CGROUP2_SUPER_MAGIC: u64 = 0x6367_7270;
const NSFS_MAGIC: u64 = 0x6E73_6673;

const SHORT_OBSERVATION_BYTES: usize = 4 * 1_024;
const CPU_INFO_BYTES: usize = 16 * 1_024 * 1_024;
const MEMORY_INFO_BYTES: usize = 256 * 1_024;
const MOUNT_INFO_BYTES: usize = 4 * 1_024 * 1_024;

pub(super) struct ProductionCurrentHostEnvironmentSource;

struct VerifiedMountRoot {
    handle: File,
    mount_id: u64,
    filesystem_magic: u64,
}

struct ObservationMounts {
    procfs: VerifiedMountRoot,
    sysfs: VerifiedMountRoot,
    cgroup2: VerifiedMountRoot,
}

impl CurrentHostEnvironmentSource for ProductionCurrentHostEnvironmentSource {
    fn observe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<HostEnvironmentV1, CurrentHostEnvironmentError> {
        observe_linux(cancellation)
    }
}

fn observe_linux(
    cancellation: &CancellationToken,
) -> Result<HostEnvironmentV1, CurrentHostEnvironmentError> {
    ensure_supported_build()?;
    ensure_active(cancellation)?;
    let mounts = ObservationMounts::open(cancellation)?;
    let release_bytes =
        mounts
            .procfs
            .read_bounded(OS_RELEASE_PATH, SHORT_OBSERVATION_BYTES, cancellation)?;
    let release = parse_kernel_release(&release_bytes)?;
    validate_uname(&release)?;

    let online_bytes =
        mounts
            .sysfs
            .read_bounded(ONLINE_CPUS_PATH, SHORT_OBSERVATION_BYTES, cancellation)?;
    let online_cpus = parse_online_cpu_list(&online_bytes)?;
    validate_affinity(&online_cpus)?;

    let cpu_info = mounts
        .procfs
        .read_bounded(CPU_INFO_PATH, CPU_INFO_BYTES, cancellation)?;
    let cpu = parse_cpu_info(&cpu_info)?;
    if cpu.logical_cpus != online_cpus {
        return Err(CurrentHostEnvironmentError::ObservationUnavailable);
    }

    validate_cgroup(&mounts, &online_cpus, cancellation)?;
    let memory_info =
        mounts
            .procfs
            .read_bounded(MEMORY_INFO_PATH, MEMORY_INFO_BYTES, cancellation)?;
    let total_system_memory_mib = parse_total_memory_mib(&memory_info)?;
    validate_affinity(&online_cpus)?;
    mounts.revalidate(cancellation)?;
    ensure_active(cancellation)?;

    HostEnvironmentV1::new(HostEnvironmentV1Input {
        operating_system: HostOperatingSystemV1Input {
            family: RuntimeOperatingSystem::Linux,
            version: release,
        },
        architecture: HostArchitectureV1Input {
            instruction_set: RuntimeArchitecture::X86_64,
            abi: RuntimeAbi::LinuxGnuLibc,
        },
        execution_class: HostExecutionClassV1Input {
            profile: HostExecutionProfileV1::ManagedLinuxNativeCpu,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
            observer_binary_assertion_mode: if cfg!(debug_assertions) {
                ObserverBinaryAssertionModeV1::Enabled
            } else {
                ObserverBinaryAssertionModeV1::Disabled
            },
            accelerator_scope: HostAcceleratorScopeV1::NotAssessedForManagedNativeCpu,
        },
        hardware_envelope: HostHardwareEnvelopeV1Input {
            cpu_model: cpu.cpu_model,
            physical_core_count: cpu.physical_core_count,
            logical_core_count: u32::try_from(cpu.logical_cpus.len())
                .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?,
            total_system_memory_mib,
            memory_rounding_granularity_mib: MEMORY_ROUNDING_GRANULARITY_MIB,
        },
    })
    .map_err(CurrentHostEnvironmentError::from)
}

fn ensure_supported_build() -> Result<(), CurrentHostEnvironmentError> {
    if cfg!(all(target_arch = "x86_64", target_env = "gnu")) {
        Ok(())
    } else {
        Err(CurrentHostEnvironmentError::UnsupportedPlatform)
    }
}

fn validate_uname(release: &str) -> Result<(), CurrentHostEnvironmentError> {
    let uname = rustix::system::uname();
    if uname.sysname().to_bytes() == b"Linux"
        && uname.machine().to_bytes() == b"x86_64"
        && uname.release().to_bytes() == release.as_bytes()
    {
        Ok(())
    } else {
        Err(CurrentHostEnvironmentError::ObservationUnavailable)
    }
}

fn validate_affinity(online_cpus: &BTreeSet<u32>) -> Result<(), CurrentHostEnvironmentError> {
    let affinity = rustix::thread::sched_getaffinity(None)
        .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?;
    validate_affinity_projection(online_cpus, rustix::thread::CpuSet::MAX_CPU, |cpu| {
        affinity.is_set(cpu)
    })
}

fn validate_cgroup(
    mounts: &ObservationMounts,
    online_cpus: &BTreeSet<u32>,
    cancellation: &CancellationToken,
) -> Result<(), CurrentHostEnvironmentError> {
    validate_cgroup_mount(mounts, cancellation)?;
    require_initial_cgroup_namespace(mounts, cancellation)?;
    let membership =
        mounts
            .procfs
            .read_bounded(THREAD_CGROUP_PATH, SHORT_OBSERVATION_BYTES, cancellation)?;
    validate_controls(&membership, online_cpus, |components, control| {
        let mut path = PathBuf::new();
        for component in components {
            path.push(component);
        }
        path.push(match control {
            CgroupControlFile::CpuMax => "cpu.max",
            CgroupControlFile::MemoryMax => "memory.max",
            CgroupControlFile::MemoryHigh => "memory.high",
            CgroupControlFile::CgroupType => "cgroup.type",
            CgroupControlFile::EffectiveCpuSet => "cpuset.cpus.effective",
        });
        mounts
            .cgroup2
            .read_bounded(path, SHORT_OBSERVATION_BYTES, cancellation)
    })?;
    let confirmation =
        mounts
            .procfs
            .read_bounded(THREAD_CGROUP_PATH, SHORT_OBSERVATION_BYTES, cancellation)?;
    if confirmation != membership {
        return Err(CurrentHostEnvironmentError::ObservationDrift);
    }
    require_initial_cgroup_namespace(mounts, cancellation)?;
    validate_cgroup_mount(mounts, cancellation)
}

fn validate_cgroup_mount(
    mounts: &ObservationMounts,
    cancellation: &CancellationToken,
) -> Result<(), CurrentHostEnvironmentError> {
    let mountinfo =
        mounts
            .procfs
            .read_bounded(SELF_MOUNT_INFO_PATH, MOUNT_INFO_BYTES, cancellation)?;
    validate_cgroup2_mount_root(&mountinfo, mounts.cgroup2.mount_id)
}

fn require_initial_cgroup_namespace(
    mounts: &ObservationMounts,
    cancellation: &CancellationToken,
) -> Result<(), CurrentHostEnvironmentError> {
    ensure_active(cancellation)?;
    mounts
        .procfs
        .require_entry_on_mount(SELF_CGROUP_NAMESPACE_PATH)?;
    let namespace = mounts
        .procfs
        .open_relative_without_mount_requirement(SELF_CGROUP_NAMESPACE_PATH)?;
    if filesystem_magic(&namespace)? != NSFS_MAGIC {
        return Err(CurrentHostEnvironmentError::ConstrainedEnvironment);
    }
    let inode = namespace
        .metadata()
        .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?
        .ino();
    ensure_active(cancellation)?;
    require_initial_namespace_inode(Some(inode))
}

impl ObservationMounts {
    fn open(cancellation: &CancellationToken) -> Result<Self, CurrentHostEnvironmentError> {
        ensure_active(cancellation)?;
        let value = Self {
            procfs: VerifiedMountRoot::open(PROC_ROOT, PROC_SUPER_MAGIC)?,
            sysfs: VerifiedMountRoot::open(SYS_ROOT, SYSFS_MAGIC)?,
            cgroup2: VerifiedMountRoot::open(CGROUP_ROOT, CGROUP2_SUPER_MAGIC)?,
        };
        ensure_active(cancellation)?;
        Ok(value)
    }

    fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), CurrentHostEnvironmentError> {
        ensure_active(cancellation)?;
        self.procfs.revalidate(PROC_ROOT)?;
        self.sysfs.revalidate(SYS_ROOT)?;
        self.cgroup2.revalidate(CGROUP_ROOT)?;
        ensure_active(cancellation)
    }
}

impl VerifiedMountRoot {
    fn open(path: &str, filesystem_magic: u64) -> Result<Self, CurrentHostEnvironmentError> {
        let handle = File::open(path)
            .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?;
        let mount_id = verified_mount_id(&handle, filesystem_magic)?;
        Ok(Self {
            handle,
            mount_id,
            filesystem_magic,
        })
    }

    fn read_bounded(
        &self,
        relative_path: impl AsRef<Path>,
        maximum_bytes: usize,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, CurrentHostEnvironmentError> {
        ensure_active(cancellation)?;
        let file = self.open_relative_on_mount(relative_path)?;
        let bytes = read_bounded_stream(file, maximum_bytes)?;
        ensure_active(cancellation)?;
        Ok(bytes)
    }

    fn open_relative_on_mount(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<File, CurrentHostEnvironmentError> {
        let file = self.open_relative_without_mount_requirement(relative_path)?;
        let observed_mount_id = verified_mount_id(&file, self.filesystem_magic)?;
        require_returned_mount_id(true, observed_mount_id, Some(self.mount_id))?;
        Ok(file)
    }

    fn open_relative_without_mount_requirement(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<File, CurrentHostEnvironmentError> {
        let descriptor = rustix::fs::openat(
            &self.handle,
            relative_path.as_ref(),
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?;
        Ok(File::from(descriptor))
    }

    fn require_entry_on_mount(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<(), CurrentHostEnvironmentError> {
        let stat = rustix::fs::statx(
            &self.handle,
            relative_path.as_ref(),
            AtFlags::NO_AUTOMOUNT | AtFlags::SYMLINK_NOFOLLOW,
            StatxFlags::MNT_ID,
        )
        .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?;
        require_returned_mount_id(
            stat.stx_mask & StatxFlags::MNT_ID.bits() != 0,
            stat.stx_mnt_id,
            Some(self.mount_id),
        )?;
        Ok(())
    }

    fn revalidate(&self, path: &str) -> Result<(), CurrentHostEnvironmentError> {
        let current = Self::open(path, self.filesystem_magic)?;
        if current.mount_id == self.mount_id {
            Ok(())
        } else {
            Err(CurrentHostEnvironmentError::ObservationDrift)
        }
    }
}

fn verified_mount_id(
    file: &File,
    expected_filesystem_magic: u64,
) -> Result<u64, CurrentHostEnvironmentError> {
    if filesystem_magic(file)? != expected_filesystem_magic {
        return Err(CurrentHostEnvironmentError::ConstrainedEnvironment);
    }
    let stat = rustix::fs::statx(file, "", AtFlags::EMPTY_PATH, StatxFlags::MNT_ID)
        .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?;
    require_returned_mount_id(
        stat.stx_mask & StatxFlags::MNT_ID.bits() != 0,
        stat.stx_mnt_id,
        None,
    )
}

fn filesystem_magic(file: &File) -> Result<u64, CurrentHostEnvironmentError> {
    let stat = rustix::fs::fstatfs(file)
        .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?;
    u64::try_from(stat.f_type).map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)
}
