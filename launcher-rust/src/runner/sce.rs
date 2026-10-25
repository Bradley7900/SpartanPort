//! The SCE container layer: a plaintext SELF or an unpacked ELF, parsed into the ELF image, its
//! program headers and the SELF's segment table. No decryption; every offset read from a file is
//! checked before use.

use std::fs;
use std::path::Path;

use serde::Serialize;

use super::prepare::{
    hex_number, io_error, place, span, u16_at, u32_at, u64_at, PrepareError, IMAGE_LIMIT,
    PT_DYNAMIC, PT_LOAD, PT_SCE_DYNLIBDATA,
};

const SELF_MAGIC: &[u8] = b"O\x15=\x1d";
const ELF_MAGIC: &[u8] = b"\x7fELF";
/// `e_ident[0..7]` of a 64-bit, little-endian, version 1 ELF.
const ELF_IDENT: &[u8] = b"\x7fELF\x02\x01\x01";
const EM_X86_64: u16 = 62;

/// SELF segment flags: the entry carries a program header's bytes (its index in bits 20..32)…
const SELF_HOLDS_SEGMENT: u64 = 0x800;
/// …which are encrypted (2) or compressed (8).
const SELF_ENCRYPTED_OR_COMPRESSED: u64 = 0x2 | 0x8;

const PT_SCE_RELRO: u32 = 0x6100_0010;

/// A segment as the loaders map it: where its bytes sit in the ELF and where they go in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub offset: u64,
    pub vaddr: u64,
    pub filesz: u64,
    pub memsz: u64,
    pub flags: u32,
}

/// One ELF program header (`<IIQQQQQQ`), serialized in `analysis.json`'s key order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgramHeader {
    #[serde(rename = "type")]
    pub kind: u32,
    pub flags: u32,
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub filesz: u64,
    pub memsz: u64,
    pub align: u64,
}

impl ProgramHeader {
    fn segment(&self) -> Segment {
        Segment {
            offset: self.offset,
            vaddr: self.vaddr,
            filesz: self.filesz,
            memsz: self.memsz,
            flags: self.flags,
        }
    }
}

/// One entry of a SELF's segment table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelfSegment {
    #[serde(serialize_with = "hex_number")]
    pub flags: u64,
    pub offset: u64,
    pub size: u64,
}

/// A parsed SELF or unpacked ELF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfInfo {
    /// The ELF with its headers and every segment at its program header's file offset.
    pub elf: Vec<u8>,
    /// `e_entry`.
    pub entry: u64,
    /// The loadable segments (`PT_LOAD`, `PT_SCE_RELRO`) in header order: what the probe maps.
    pub segments: Vec<Segment>,
    pub program_headers: Vec<ProgramHeader>,
    /// The SELF's segment table; empty for an unpacked ELF.
    pub self_segments: Vec<SelfSegment>,
    /// The program headers whose bytes the SELF does not carry (non-loadable metadata only).
    pub missing: Vec<usize>,
}

fn read_program_header(data: &[u8], offset: u64) -> Result<ProgramHeader, PrepareError> {
    let record = span(data, offset, 56)?;
    Ok(ProgramHeader {
        kind: u32_at(record, 0)?,
        flags: u32_at(record, 4)?,
        offset: u64_at(record, 8)?,
        vaddr: u64_at(record, 16)?,
        paddr: u64_at(record, 24)?,
        filesz: u64_at(record, 32)?,
        memsz: u64_at(record, 40)?,
        align: u64_at(record, 48)?,
    })
}

/// Parses a plaintext SELF or an unpacked ELF (x86-64, little-endian) and lays its segments out at
/// their program headers' file offsets. Encrypted or compressed segments are refused.
pub fn parse_self(data: &[u8]) -> Result<SelfInfo, PrepareError> {
    if data.is_empty() {
        // read_self names the file; this covers every other way in.
        return Err(PrepareError::EmptySelf);
    }
    let magic = span(data, 0, 4)?;
    let plain_elf = magic == ELF_MAGIC;
    let (count, base) = if plain_elf {
        (0, 0)
    } else if magic == SELF_MAGIC {
        let count = u64::from(u16_at(data, 24)?);
        (count, 32 + count * 32)
    } else {
        return Err(PrepareError::NotSelf);
    };
    let header = span(data, base, 64)?;
    let ident = span(header, 0, 7)?;
    if ident.get(5) != Some(&1) {
        return Err(PrepareError::BigEndian);
    }
    if ident != ELF_IDENT || u16_at(header, 18)? != EM_X86_64 {
        return Err(PrepareError::NotX86_64);
    }
    let entry = u64_at(header, 24)?;
    let phoff = u64_at(header, 32)?;
    let phnum = u16_at(header, 56)?;
    if u16_at(header, 54)? != 56 || !(1..256).contains(&phnum) {
        return Err(PrepareError::BadProgramHeader);
    }
    let program_headers = (0..u64::from(phnum))
        .map(|i| read_program_header(data, base.saturating_add(phoff).saturating_add(i * 56)))
        .collect::<Result<Vec<_>, _>>()?;
    let header_end = phoff.saturating_add(u64::from(phnum) * 56);
    let end = program_headers
        .iter()
        .map(|p| p.offset.saturating_add(p.filesz))
        .fold(header_end, u64::max);
    if end > IMAGE_LIMIT {
        return Err(PrepareError::ImageTooLarge);
    }
    let segments = program_headers
        .iter()
        .filter(|p| p.kind == PT_LOAD || p.kind == PT_SCE_RELRO)
        .map(ProgramHeader::segment)
        .collect();
    if plain_elf {
        // An unpacked ELF already holds its segments at their file offsets.
        return Ok(SelfInfo {
            elf: span(data, 0, end)?.to_vec(),
            entry,
            segments,
            program_headers,
            self_segments: Vec::new(),
            missing: Vec::new(),
        });
    }
    let mut elf = vec![0; usize::try_from(end).map_err(|_| PrepareError::ImageTooLarge)?];
    place(&mut elf, 0, span(data, base, header_end)?)?;
    let mut covered = vec![(0, header_end)];
    let mut self_segments = Vec::new();
    for i in 0..count {
        let record = span(data, 32 + i * 32, 32)?;
        let segment = SelfSegment {
            flags: u64_at(record, 0)?,
            offset: u64_at(record, 8)?,
            size: u64_at(record, 16)?,
        };
        let memory = u64_at(record, 24)?;
        let (flags, offset, size) = (segment.flags, segment.offset, segment.size);
        self_segments.push(segment);
        if flags & SELF_HOLDS_SEGMENT == 0 {
            continue;
        }
        if flags & SELF_ENCRYPTED_OR_COMPRESSED != 0 {
            return Err(PrepareError::Encrypted);
        }
        let header = usize::try_from((flags >> 20) & 0xfff)
            .ok()
            .and_then(|index| program_headers.get(index))
            .ok_or(PrepareError::BadSegmentIndex)?;
        if size != header.filesz || size != memory {
            return Err(PrepareError::BlockedSegment);
        }
        place(&mut elf, header.offset, span(data, offset, size)?)?;
        covered.push((header.offset, header.offset.saturating_add(size)));
    }
    let mut missing = Vec::new();
    for (i, p) in program_headers.iter().enumerate() {
        let stop = p.offset.saturating_add(p.filesz);
        if p.filesz != 0 && !covered.iter().any(|&(a, z)| a <= p.offset && stop <= z) {
            missing.push(i);
            if matches!(
                p.kind,
                PT_LOAD | PT_DYNAMIC | PT_SCE_DYNLIBDATA | PT_SCE_RELRO
            ) {
                return Err(PrepareError::RequiredSegment(i));
            }
        }
    }
    Ok(SelfInfo {
        elf,
        entry,
        segments,
        program_headers,
        self_segments,
        missing,
    })
}

/// The bytes of a SELF file, which must hold something: an empty `eboot.bin` or prx (a copy that
/// failed, a placeholder made by hand) is named here instead of failing deep in the parser.
pub fn read_self_bytes(path: &Path) -> Result<Vec<u8>, PrepareError> {
    let data = fs::read(path).map_err(io_error(path))?;
    if data.is_empty() {
        return Err(PrepareError::EmptyFile(path.to_path_buf()));
    }
    Ok(data)
}

/// The SELF or ELF in `path`, parsed.
pub fn read_self(path: &Path) -> Result<SelfInfo, PrepareError> {
    parse_self(&read_self_bytes(path)?)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Little-endian fields as `struct.pack('<…')` lays them out: `(value, width in bytes)` each.
    pub(in crate::runner) fn pack(fields: &[(u64, usize)]) -> Vec<u8> {
        fields
            .iter()
            .flat_map(|&(value, width)| value.to_le_bytes()[..width].to_vec())
            .collect()
    }

    pub(in crate::runner) fn put(data: &mut [u8], at: usize, bytes: &[u8]) {
        data[at..at + bytes.len()].copy_from_slice(bytes);
    }

    /// `<16sHHIQQQIHHHHHH`: an x86-64 ELF header whose program headers follow it at offset 64.
    pub(in crate::runner) fn elf_header(e_type: u64, phnum: u64) -> Vec<u8> {
        let mut header = b"\x7fELF\x02\x01\x01\x09".to_vec();
        header.resize(16, 0);
        header.extend(pack(&[
            (e_type, 2),
            (62, 2),
            (1, 4),
            (0, 8),
            (64, 8),
            (0, 8),
            (0, 4),
            (64, 2),
            (56, 2),
            (phnum, 2),
            (0, 2),
            (0, 2),
            (0, 2),
        ]));
        header
    }

    /// `<IIQQQQQQ`: type, flags, offset, vaddr, paddr, filesz, memsz, align.
    pub(in crate::runner) fn program_header([kind, flags, rest @ ..]: [u64; 8]) -> Vec<u8> {
        let mut header = pack(&[(kind, 4), (flags, 4)]);
        for value in rest {
            header.extend(value.to_le_bytes());
        }
        header
    }

    /// `<QQQQ`: a SELF segment entry (flags, file offset, size, memory size).
    pub(in crate::runner) fn self_entry(fields: [u64; 4]) -> Vec<u8> {
        fields
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    /// `tests/test_prepare.py::fixture`: a SELF whose one plaintext segment lands at file offset
    /// 0x1000 of the ELF while its PT_LOAD maps it at address 0.
    pub(in crate::runner) fn synthetic_self() -> Vec<u8> {
        let mut data = vec![0; 0x210];
        put(&mut data, 0, b"O\x15=\x1d");
        put(&mut data, 24, &pack(&[(1, 2)]));
        put(&mut data, 32, &self_entry([0x800, 0x200, 16, 16]));
        put(&mut data, 64, &elf_header(0xfe10, 1));
        put(
            &mut data,
            128,
            &program_header([1, 5, 0x1000, 0, 0, 16, 32, 0x1000]),
        );
        put(&mut data, 0x200, &(0..16).collect::<Vec<u8>>());
        data
    }

    /// `result` must be an error whose message contains `text` (the Python's `assertRaisesRegex`).
    #[track_caller]
    pub(in crate::runner) fn assert_fails<T: std::fmt::Debug>(
        result: Result<T, PrepareError>,
        text: &str,
    ) {
        let message = result.unwrap_err().to_string();
        assert!(message.contains(text), "{message:?} lacks {text:?}");
    }

    #[test]
    fn unpacked_elf_has_the_same_headers_and_payload_as_self() {
        let expected = parse_self(&synthetic_self()).unwrap();
        let actual = parse_self(&expected.elf).unwrap();
        assert_eq!(actual.elf, expected.elf);
        assert_eq!(actual.entry, expected.entry);
        assert_eq!(actual.program_headers, expected.program_headers);
        assert_eq!(actual.segments, expected.segments);
        assert_eq!(actual.self_segments, []);
        assert_eq!(actual.missing, Vec::<usize>::new());
    }

    #[test]
    fn unpacked_elf_rejects_truncated_segments() {
        let elf = parse_self(&synthetic_self()).unwrap().elf;
        assert_fails(parse_self(&elf[..elf.len() - 1]), "out of bounds");
    }

    #[test]
    fn unpacked_elf_rejects_other_architectures_and_byte_orders() {
        let elf = parse_self(&synthetic_self()).unwrap().elf;
        for (offset, value) in [(4, 1), (5, 2), (18, 3)] {
            let mut bad = elf.clone();
            bad[offset] = value;
            assert_fails(parse_self(&bad), "little-endian x86-64 ELF");
        }
    }

    #[test]
    fn unpacked_elf_rejects_truncated_program_headers() {
        let elf = parse_self(&synthetic_self()).unwrap().elf;
        assert_fails(parse_self(&elf[..100]), "out of bounds");
    }

    #[test]
    fn unpacked_elf_checks_image_limit_before_allocating() {
        let mut elf = parse_self(&synthetic_self()).unwrap().elf;
        put(&mut elf, 64 + 8, &(512u64 * 1024 * 1024).to_le_bytes());
        assert_fails(parse_self(&elf), "512 MiB");
    }

    #[test]
    fn file_offset_is_not_virtual_address() {
        let info = parse_self(&synthetic_self()).unwrap();
        assert_eq!(info.elf[0x1000..0x1010], (0..16).collect::<Vec<u8>>());
        assert_eq!(info.program_headers[0].vaddr, 0);
        assert_eq!(info.missing, Vec::<usize>::new());
    }

    #[test]
    fn truncated_payload_rejected() {
        let data = synthetic_self();
        assert_fails(parse_self(&data[..data.len() - 1]), "out of bounds");
    }

    #[test]
    fn encrypted_or_compressed_payload_rejected() {
        for flag in [2u64, 8] {
            let mut data = synthetic_self();
            put(&mut data, 32, &(0x800 | flag).to_le_bytes());
            assert_fails(parse_self(&data), "encrypted/compressed");
        }
    }

    #[test]
    fn missing_required_segment_rejected() {
        let mut data = synthetic_self();
        put(&mut data, 32, &0u64.to_le_bytes());
        assert_fails(parse_self(&data), "required segment");
    }

    #[test]
    fn bad_program_header_index_rejected() {
        let mut data = synthetic_self();
        put(&mut data, 32, &0x10_0800u64.to_le_bytes());
        assert_fails(parse_self(&data), "segment index");
    }
}
