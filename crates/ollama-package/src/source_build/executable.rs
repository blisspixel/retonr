use super::RuntimeSourceBuildInputError;

const ELF_HEADER_BYTES: usize = 64;
const ELF_PROGRAM_HEADER_BYTES: usize = 56;
const ELF_HEADER_SIZE: u16 = 64;
const ELF_PROGRAM_HEADER_SIZE: u16 = 56;
const MAXIMUM_PROGRAM_HEADERS: usize = 128;
const MAXIMUM_EXECUTABLE_BYTES: usize = 64 * 1024 * 1024;
const EM_X86_64: u16 = 62;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EV_CURRENT: u32 = 1;
const ELFOSABI_SYSV: u8 = 0;
const ELFOSABI_LINUX: u8 = 3;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_SONAME: u64 = 14;
const DT_RPATH: u64 = 15;
const DT_RUNPATH: u64 = 29;
const DT_CONFIG: u64 = 0x6fff_fefa;
const DT_DEPAUDIT: u64 = 0x6fff_fefb;
const DT_AUDIT: u64 = 0x6fff_fefc;
const DT_AUXILIARY: u64 = 0x7fff_fffd;
const DT_USED: u64 = 0x7fff_fffe;
const DT_FILTER: u64 = 0x7fff_ffff;
const DYNAMIC_ENTRY_BYTES: usize = 16;

/// Verified fully static Linux x86-64 ELF executable.
///
/// This validates the complete bounded file, not only the ELF header. It proves
/// that the kernel does not load an interpreter, that the dynamic table names no
/// runtime dependency or search path, and that load and stack permissions are
/// structurally safe. It does not claim that later program behavior cannot open
/// files. The controlled-build filesystem boundary enforces that separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelfContainedLinuxExecutable {
    executable_type: u16,
    program_headers: u16,
}

impl SelfContainedLinuxExecutable {
    /// Maximum accepted bytes for one retained build executable.
    pub const MAXIMUM_BYTES: usize = MAXIMUM_EXECUTABLE_BYTES;

    /// Verifies one complete, bounded ELF executable.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildInputError::NonSelfContainedExecutable`] when
    /// the file is not a complete little-endian Linux x86-64 executable, has a
    /// separately loaded interpreter or dynamic dependency, has unsafe segment
    /// permissions, or contains malformed or overlapping load segments.
    pub fn verify(bytes: &[u8]) -> Result<Self, RuntimeSourceBuildInputError> {
        if bytes.len() < ELF_HEADER_BYTES
            || bytes.len() > Self::MAXIMUM_BYTES
            || bytes[..4] != *b"\x7fELF"
            || bytes[4] != 2
            || bytes[5] != 1
            || bytes[6] != 1
            || !matches!(bytes[7], ELFOSABI_SYSV | ELFOSABI_LINUX)
            || bytes[8] != 0
            || bytes[9..16].iter().any(|byte| *byte != 0)
            || read_u16(bytes, 18) != Some(EM_X86_64)
            || read_u32(bytes, 20) != Some(EV_CURRENT)
            || read_u32(bytes, 48) != Some(0)
            || read_u16(bytes, 52) != Some(ELF_HEADER_SIZE)
            || read_u16(bytes, 54) != Some(ELF_PROGRAM_HEADER_SIZE)
        {
            return invalid();
        }
        let executable_type = read_u16(bytes, 16)
            .filter(|value| matches!(*value, ET_EXEC | ET_DYN))
            .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
        let entrypoint = read_u64(bytes, 24)
            .filter(|value| *value != 0)
            .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
        let program_headers = read_u16(bytes, 56)
            .filter(|value| usize::from(*value) <= MAXIMUM_PROGRAM_HEADERS && *value > 0)
            .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
        let offset = read_usize(bytes, 32)?;
        if offset < ELF_HEADER_BYTES {
            return invalid();
        }
        let table_bytes = usize::from(program_headers)
            .checked_mul(ELF_PROGRAM_HEADER_BYTES)
            .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
        let end = checked_file_end(offset, table_bytes, bytes.len())?;

        let mut load_segments = Vec::new();
        let mut dynamic_segment = None;
        let mut stack_segment = None;
        for header in bytes[offset..end].chunks_exact(ELF_PROGRAM_HEADER_BYTES) {
            let segment = ProgramSegment::parse(header, bytes.len())?;
            match segment.kind {
                PT_INTERP => return invalid(),
                PT_LOAD => {
                    if segment.file_bytes > segment.memory_bytes
                        || segment.flags & !(PF_R | PF_W | PF_X) != 0
                        || segment.flags & (PF_W | PF_X) == (PF_W | PF_X)
                        || !segment.alignment_is_valid()
                    {
                        return invalid();
                    }
                    load_segments.push(segment);
                }
                PT_DYNAMIC => {
                    if dynamic_segment.replace(segment).is_some() {
                        return invalid();
                    }
                }
                PT_GNU_STACK
                    if stack_segment.is_some()
                        || segment.flags & PF_X != 0
                        || segment.file_bytes != 0 =>
                {
                    return invalid();
                }
                PT_GNU_STACK => stack_segment = Some(segment),
                _ => {}
            }
        }
        if load_segments.is_empty() || stack_segment.is_none() {
            return invalid();
        }
        validate_load_segments(&load_segments, entrypoint)?;
        if let Some(dynamic) = dynamic_segment {
            validate_dynamic_table(bytes, dynamic)?;
        }
        Ok(Self {
            executable_type,
            program_headers,
        })
    }

    /// Returns the ELF object type (`ET_EXEC` or `ET_DYN`).
    #[must_use]
    pub const fn executable_type(self) -> u16 {
        self.executable_type
    }

    /// Returns the validated number of program headers.
    #[must_use]
    pub const fn program_headers(self) -> u16 {
        self.program_headers
    }
}

#[derive(Clone, Copy, Debug)]
struct ProgramSegment {
    kind: u32,
    flags: u32,
    file_offset: usize,
    file_bytes: u64,
    virtual_address: u64,
    memory_bytes: u64,
    alignment: u64,
}

impl ProgramSegment {
    fn parse(header: &[u8], file_length: usize) -> Result<Self, RuntimeSourceBuildInputError> {
        let segment = Self {
            kind: read_u32(header, 0)
                .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?,
            flags: read_u32(header, 4)
                .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?,
            file_offset: read_usize(header, 8)?,
            virtual_address: read_u64(header, 16)
                .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?,
            file_bytes: read_u64(header, 32)
                .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?,
            memory_bytes: read_u64(header, 40)
                .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?,
            alignment: read_u64(header, 48)
                .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?,
        };
        let file_bytes = usize::try_from(segment.file_bytes)
            .map_err(|_| RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
        checked_file_end(segment.file_offset, file_bytes, file_length)?;
        Ok(segment)
    }

    fn alignment_is_valid(self) -> bool {
        self.alignment <= 1
            || (self.alignment.is_power_of_two()
                && self.virtual_address % self.alignment
                    == u64::try_from(self.file_offset).unwrap_or(u64::MAX) % self.alignment)
    }

    fn file_range(self) -> Result<std::ops::Range<usize>, RuntimeSourceBuildInputError> {
        let bytes = usize::try_from(self.file_bytes)
            .map_err(|_| RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
        let end = self
            .file_offset
            .checked_add(bytes)
            .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
        Ok(self.file_offset..end)
    }

    fn memory_end(self) -> Result<u64, RuntimeSourceBuildInputError> {
        self.virtual_address
            .checked_add(self.memory_bytes)
            .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
    }
}

fn validate_load_segments(
    load_segments: &[ProgramSegment],
    entrypoint: u64,
) -> Result<(), RuntimeSourceBuildInputError> {
    let mut executable_entrypoint = false;
    for (index, segment) in load_segments.iter().copied().enumerate() {
        let memory_end = segment.memory_end()?;
        if segment.memory_bytes == 0 {
            return invalid();
        }
        if segment.flags & PF_X != 0
            && entrypoint >= segment.virtual_address
            && entrypoint < memory_end
        {
            executable_entrypoint = true;
        }
        let file_range = segment.file_range()?;
        for other in load_segments[index + 1..].iter().copied() {
            let other_file_range = other.file_range()?;
            if ranges_overlap(&file_range, &other_file_range)
                || ranges_overlap_u64(
                    segment.virtual_address,
                    memory_end,
                    other.virtual_address,
                    other.memory_end()?,
                )
            {
                return invalid();
            }
        }
    }
    if executable_entrypoint {
        Ok(())
    } else {
        invalid()
    }
}

fn validate_dynamic_table(
    bytes: &[u8],
    segment: ProgramSegment,
) -> Result<(), RuntimeSourceBuildInputError> {
    let range = segment.file_range()?;
    if range.is_empty() || range.len() % DYNAMIC_ENTRY_BYTES != 0 {
        return invalid();
    }
    let mut terminated = false;
    for entry in bytes[range].chunks_exact(DYNAMIC_ENTRY_BYTES) {
        let tag =
            read_u64(entry, 0).ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
        if terminated {
            if entry.iter().any(|byte| *byte != 0) {
                return invalid();
            }
            continue;
        }
        match tag {
            DT_NULL => terminated = true,
            DT_NEEDED | DT_SONAME | DT_RPATH | DT_RUNPATH | DT_CONFIG | DT_DEPAUDIT | DT_AUDIT
            | DT_AUXILIARY | DT_USED | DT_FILTER => return invalid(),
            _ => {}
        }
    }
    if terminated { Ok(()) } else { invalid() }
}

fn checked_file_end(
    offset: usize,
    bytes: usize,
    file_length: usize,
) -> Result<usize, RuntimeSourceBuildInputError> {
    offset
        .checked_add(bytes)
        .filter(|end| *end <= file_length)
        .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
}

fn ranges_overlap(left: &std::ops::Range<usize>, right: &std::ops::Range<usize>) -> bool {
    !left.is_empty() && !right.is_empty() && left.start < right.end && right.start < left.end
}

fn ranges_overlap_u64(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

fn invalid<T>() -> Result<T, RuntimeSourceBuildInputError> {
    Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_usize(bytes: &[u8], offset: usize) -> Result<usize, RuntimeSourceBuildInputError> {
    read_u64(bytes, offset)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
}

#[cfg(test)]
mod tests;
