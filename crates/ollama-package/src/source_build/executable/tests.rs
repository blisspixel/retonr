use super::*;

#[derive(Clone, Copy)]
struct ProgramSegment {
    kind: u32,
    flags: u32,
    file_offset: u64,
    virtual_address: u64,
    file_bytes: u64,
    memory_bytes: u64,
    alignment: u64,
}

fn write_segment(bytes: &mut [u8], index: usize, segment: ProgramSegment) {
    let offset = ELF_HEADER_BYTES + index * ELF_PROGRAM_HEADER_BYTES;
    let header = &mut bytes[offset..offset + ELF_PROGRAM_HEADER_BYTES];
    header[..4].copy_from_slice(&segment.kind.to_le_bytes());
    header[4..8].copy_from_slice(&segment.flags.to_le_bytes());
    header[8..16].copy_from_slice(&segment.file_offset.to_le_bytes());
    header[16..24].copy_from_slice(&segment.virtual_address.to_le_bytes());
    header[32..40].copy_from_slice(&segment.file_bytes.to_le_bytes());
    header[40..48].copy_from_slice(&segment.memory_bytes.to_le_bytes());
    header[48..56].copy_from_slice(&segment.alignment.to_le_bytes());
}

fn executable(extra_program_headers: usize) -> Vec<u8> {
    let count = 2 + extra_program_headers;
    let length = ELF_HEADER_BYTES + count * ELF_PROGRAM_HEADER_BYTES;
    let mut bytes = vec![0_u8; length];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[16..18].copy_from_slice(&ET_DYN.to_le_bytes());
    bytes[18..20].copy_from_slice(&EM_X86_64.to_le_bytes());
    bytes[20..24].copy_from_slice(&EV_CURRENT.to_le_bytes());
    bytes[24..32].copy_from_slice(&0x1_040_u64.to_le_bytes());
    bytes[32..40].copy_from_slice(&(ELF_HEADER_BYTES as u64).to_le_bytes());
    bytes[52..54].copy_from_slice(&ELF_HEADER_SIZE.to_le_bytes());
    bytes[54..56].copy_from_slice(&ELF_PROGRAM_HEADER_SIZE.to_le_bytes());
    bytes[56..58].copy_from_slice(
        &u16::try_from(count)
            .expect("test program-header count fits in u16")
            .to_le_bytes(),
    );
    write_segment(
        &mut bytes,
        0,
        ProgramSegment {
            kind: PT_LOAD,
            flags: PF_R | PF_X,
            file_offset: 0,
            virtual_address: 0x1_000,
            file_bytes: u64::try_from(length).expect("test executable length fits in u64"),
            memory_bytes: u64::try_from(length).expect("test executable length fits in u64"),
            alignment: 0x1_000,
        },
    );
    write_segment(
        &mut bytes,
        1,
        ProgramSegment {
            kind: PT_GNU_STACK,
            flags: PF_R | PF_W,
            file_offset: 0,
            virtual_address: 0,
            file_bytes: 0,
            memory_bytes: 0,
            alignment: 16,
        },
    );
    bytes
}

#[test]
fn accepts_a_complete_static_pie() {
    let verified = SelfContainedLinuxExecutable::verify(&executable(0)).expect("static PIE");
    assert_eq!(verified.executable_type(), ET_DYN);
    assert_eq!(verified.program_headers(), 2);
}

#[test]
fn accepts_a_dependency_free_static_pie_dynamic_table() {
    let mut bytes = executable(1);
    let dynamic_offset = bytes.len();
    bytes.extend_from_slice(&7_u64.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&DT_NULL.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    let length = u64::try_from(bytes.len()).expect("test executable length fits in u64");
    write_segment(
        &mut bytes,
        0,
        ProgramSegment {
            kind: PT_LOAD,
            flags: PF_R | PF_X,
            file_offset: 0,
            virtual_address: 0x1_000,
            file_bytes: length,
            memory_bytes: length,
            alignment: 0x1_000,
        },
    );
    write_segment(
        &mut bytes,
        2,
        ProgramSegment {
            kind: PT_DYNAMIC,
            flags: PF_R,
            file_offset: u64::try_from(dynamic_offset).expect("test offset fits in u64"),
            virtual_address: 0x1_000
                + u64::try_from(dynamic_offset).expect("test offset fits in u64"),
            file_bytes: 32,
            memory_bytes: 32,
            alignment: 8,
        },
    );
    SelfContainedLinuxExecutable::verify(&bytes).expect("dependency-free dynamic table");
}

#[test]
fn rejects_interpreter_dependencies_and_dynamic_paths() {
    for (kind, dynamic_tag) in [
        (PT_INTERP, None),
        (PT_DYNAMIC, Some(DT_NEEDED)),
        (PT_DYNAMIC, Some(DT_SONAME)),
        (PT_DYNAMIC, Some(DT_RPATH)),
        (PT_DYNAMIC, Some(DT_RUNPATH)),
        (PT_DYNAMIC, Some(DT_CONFIG)),
        (PT_DYNAMIC, Some(DT_DEPAUDIT)),
        (PT_DYNAMIC, Some(DT_AUDIT)),
        (PT_DYNAMIC, Some(DT_AUXILIARY)),
        (PT_DYNAMIC, Some(DT_USED)),
        (PT_DYNAMIC, Some(DT_FILTER)),
    ] {
        let mut bytes = executable(1);
        let payload_offset = bytes.len();
        bytes.extend_from_slice(&dynamic_tag.unwrap_or(DT_NULL).to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&DT_NULL.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        let length = u64::try_from(bytes.len()).expect("test executable length fits in u64");
        write_segment(
            &mut bytes,
            0,
            ProgramSegment {
                kind: PT_LOAD,
                flags: PF_R | PF_X,
                file_offset: 0,
                virtual_address: 0x1_000,
                file_bytes: length,
                memory_bytes: length,
                alignment: 0x1_000,
            },
        );
        write_segment(
            &mut bytes,
            2,
            ProgramSegment {
                kind,
                flags: PF_R,
                file_offset: u64::try_from(payload_offset).expect("test offset fits in u64"),
                virtual_address: 0x1_000
                    + u64::try_from(payload_offset).expect("test offset fits in u64"),
                file_bytes: 32,
                memory_bytes: 32,
                alignment: 8,
            },
        );
        assert_eq!(
            SelfContainedLinuxExecutable::verify(&bytes),
            Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable),
            "kind {kind:#x}, tag {dynamic_tag:?}"
        );
    }
}

#[test]
fn rejects_unsafe_permissions_and_malformed_ranges() {
    let mut writable_executable = executable(0);
    let length =
        u64::try_from(writable_executable.len()).expect("test executable length fits in u64");
    write_segment(
        &mut writable_executable,
        0,
        ProgramSegment {
            kind: PT_LOAD,
            flags: PF_R | PF_W | PF_X,
            file_offset: 0,
            virtual_address: 0x1_000,
            file_bytes: length,
            memory_bytes: length,
            alignment: 0x1_000,
        },
    );
    assert_eq!(
        SelfContainedLinuxExecutable::verify(&writable_executable),
        Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
    );

    let mut executable_stack = executable(0);
    write_segment(
        &mut executable_stack,
        1,
        ProgramSegment {
            kind: PT_GNU_STACK,
            flags: PF_R | PF_W | PF_X,
            file_offset: 0,
            virtual_address: 0,
            file_bytes: 0,
            memory_bytes: 0,
            alignment: 16,
        },
    );
    assert_eq!(
        SelfContainedLinuxExecutable::verify(&executable_stack),
        Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
    );

    let mut outside_file = executable(0);
    let length = u64::try_from(outside_file.len()).expect("test executable length fits in u64");
    write_segment(
        &mut outside_file,
        0,
        ProgramSegment {
            kind: PT_LOAD,
            flags: PF_R | PF_X,
            file_offset: 0,
            virtual_address: 0x1_000,
            file_bytes: length + 1,
            memory_bytes: length + 1,
            alignment: 0x1_000,
        },
    );
    assert_eq!(
        SelfContainedLinuxExecutable::verify(&outside_file),
        Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
    );
}

#[test]
fn rejects_wrong_architecture_incomplete_tables_and_missing_stack() {
    let mut wrong_architecture = executable(0);
    wrong_architecture[18..20].copy_from_slice(&183_u16.to_le_bytes());
    assert_eq!(
        SelfContainedLinuxExecutable::verify(&wrong_architecture),
        Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
    );
    let mut incomplete = executable(0);
    incomplete.truncate(ELF_HEADER_BYTES);
    assert_eq!(
        SelfContainedLinuxExecutable::verify(&incomplete),
        Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
    );
    let mut missing_stack = executable(0);
    write_segment(
        &mut missing_stack,
        1,
        ProgramSegment {
            kind: 4,
            flags: PF_R,
            file_offset: 0,
            virtual_address: 0,
            file_bytes: 0,
            memory_bytes: 0,
            alignment: 0,
        },
    );
    assert_eq!(
        SelfContainedLinuxExecutable::verify(&missing_stack),
        Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable)
    );
}
