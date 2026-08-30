use std::{io, os::fd::AsFd};

use rustix::io::pread;

const ELF_MAGIC: [u8; 4] = *b"\x7fELF";

pub(super) fn has_elf_magic(file: impl AsFd) -> io::Result<bool> {
    let mut magic = [0_u8; ELF_MAGIC.len()];
    let read = pread(file, &mut magic[..], 0)?;
    Ok(read == magic.len() && magic == ELF_MAGIC)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::has_elf_magic;

    #[test]
    fn elf_magic_is_required_at_offset_zero() {
        let mut file = tempfile::tempfile().expect("temporary ELF fixture");
        file.write_all(b"\x7fELFfixture")
            .expect("write ELF fixture");
        assert!(has_elf_magic(&file).expect("read ELF fixture"));

        let mut script = tempfile::tempfile().expect("temporary script fixture");
        script
            .write_all(b"#!/bin/sh\nexit 0\n")
            .expect("write script fixture");
        assert!(!has_elf_magic(&script).expect("read script fixture"));
    }
}
