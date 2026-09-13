extern crate simple_endian;

use super::Error;
use super::binfmt::*;

use core::ffi::CStr;
use core::mem;
use std::ffi::CString;

use super::marker::*;

use simple_endian::{u16be, u32be};

#[derive(Debug, Clone)]
pub enum PoolValue {
    Str(CString),    // IREP_TT_STR = 0 (need free)
    Int32(i32),      // IREP_TT_INT32 = 1
    SStr(CString),   // IREP_TT_SSTR = 2 (static)
    Int64(i64),      // IREP_TT_INT64 = 3
    Float(f64),      // IREP_TT_FLOAT = 5
    BigInt(Vec<u8>), // IREP_TT_BIGINT = 7 (not yet fully supported)
}

#[derive(Debug, Default)]
pub struct Rite<'a> {
    pub binary_header: RiteBinaryHeader,
    pub irep_header: SectionIrepHeader,
    pub irep: Vec<Irep<'a>>,
    pub lvar: Option<LVar>,
}

#[derive(Debug)]
pub struct Irep<'a> {
    pub header: IrepRecord,
    pub insn: &'a [u8],
    pub plen: usize,
    pub pool: Vec<PoolValue>,
    pub slen: usize,
    pub syms: Vec<CString>,
    pub catch_handlers: Vec<CatchHandler>,
    pub lv: Vec<Option<CString>>, // Local variable names (indices into LVar::syms)
    /// Source line changes (instruction-byte offset, line), ascending.
    /// Empty when the blob was compiled without debug info.
    pub lines: Vec<(u32, u32)>,
}

impl Irep<'_> {
    pub fn nlocals(&self) -> usize {
        be16_to_u16(self.header.nlocals) as usize
    }

    pub fn nregs(&self) -> usize {
        be16_to_u16(self.header.nregs) as usize
    }

    pub fn rlen(&self) -> usize {
        be16_to_u16(self.header.rlen) as usize
    }

    pub fn clen(&self) -> usize {
        be16_to_u16(self.header.clen) as usize
    }
}

#[derive(Debug)]
pub struct CatchHandler {
    pub type_: u8,
    pub start: usize,
    pub end: usize,
    pub target: usize,
}

#[derive(Debug)]
pub struct LVar {
    pub header: SectionMiscHeader,
    pub syms: Vec<CString>,
}

const RITE_LV_NULL_MARK: u16 = 0xFFFF;

pub fn load<'a>(src: &'a [u8]) -> Result<Rite<'a>, Error> {
    let mut rite = Rite::default();

    let mut size = src.len();
    let mut head = src;
    let binheader_size = mem::size_of::<RiteBinaryHeader>();
    if size < binheader_size {
        dbg!(size < binheader_size);
        return Err(Error::TooShort);
    }
    let binary_header = RiteBinaryHeader::from_bytes(&head[0..binheader_size])?;
    // Only mruby 4.0 (RITE0400) chunks decode with this opcode table.
    if &binary_header.major_version != b"04" {
        return Err(Error::UnsupportedVersion(binary_header.major_version));
    }
    rite.binary_header = binary_header;
    size -= binheader_size;
    head = &head[binheader_size..];

    // let binsize: u32 = be32_to_u32(rite.binary_header.size);

    let irep_header_size = mem::size_of::<SectionIrepHeader>();
    if size < irep_header_size {
        dbg!(size, irep_header_size, size < irep_header_size);
        return Err(Error::TooShort);
    }

    while let Some(chrs) = peek4(head) {
        match chrs {
            IREP => {
                let (cur, irep_header, irep) = section_irep_1(head)?;
                rite.irep_header = irep_header;
                rite.irep = irep;
                head = &head[cur..];
            }
            LVAR => {
                let (cur, lvar) = section_lvar(head, &mut rite.irep)?;
                rite.lvar = Some(lvar);
                head = &head[cur..];
            }
            DBG => {
                let cur = parse_debug_section(head, &mut rite.irep)?;
                head = &head[cur..];
            }
            END => {
                let cur = section_end(head)?;
                head = &head[cur..];
            }
            _ => {
                eprintln!("{:?}", chrs);
                eprint!("{:?}", head);
                return Err(Error::InvalidFormat);
            }
        }
    }

    Ok(rite)
}

pub fn section_irep_1(head: &[u8]) -> Result<(usize, SectionIrepHeader, Vec<Irep<'_>>), Error> {
    let mut cur = 0;

    let irep_header_size = mem::size_of::<SectionIrepHeader>();
    let irep_header = SectionIrepHeader::from_bytes(&head[cur..irep_header_size])?;
    let irep_size = be32_to_u32(irep_header.size) as usize;
    if head.len() < irep_size {
        dbg!((head.len(), irep_size, head.len() < irep_size));
        return Err(Error::TooShort);
    }
    cur += irep_header_size;

    let mut ireps: Vec<Irep> = Vec::new();

    while cur < irep_size {
        let mut pool = Vec::<PoolValue>::new();
        let mut syms = Vec::<CString>::new();

        let start_cur = cur;
        // insn
        let record_size = mem::size_of::<IrepRecord>();
        let irep_record = IrepRecord::from_bytes(&head[cur..cur + record_size])?;
        let irep_rec_size = be32_to_u32(irep_record.size) as usize;
        let ilen = be32_to_u32(irep_record.ilen) as usize;
        cur += record_size;

        let insns = &head[cur..cur + ilen];

        cur += ilen;

        let mut catch_handlers = Vec::<CatchHandler>::new();
        let clen = be16_to_u16(irep_record.clen) as usize;
        if clen > 0 {
            for _ in 0..clen {
                let value = CatchHandler {
                    type_: head[cur],
                    start: be32_to_u32([head[cur + 1], head[cur + 2], head[cur + 3], head[cur + 4]])
                        as usize,
                    end: be32_to_u32([head[cur + 5], head[cur + 6], head[cur + 7], head[cur + 8]])
                        as usize,
                    target: be32_to_u32([
                        head[cur + 9],
                        head[cur + 10],
                        head[cur + 11],
                        head[cur + 12],
                    ]) as usize,
                };
                catch_handlers.push(value);
                cur += mem::size_of::<IrepCatchHandler>();
            }
        }

        // pool
        let data = &head[cur..cur + 2];
        let plen = be16_to_u16([data[0], data[1]]) as usize;
        cur += 2;

        for _ in 0..plen {
            let typ = head[cur];
            cur += 1;
            match typ {
                0 => {
                    // IREP_TT_STR: String (need free)
                    let data = &head[cur..cur + 2];
                    let strlen = be16_to_u16([data[0], data[1]]) as usize + 1;
                    cur += 2;
                    let strval = CStr::from_bytes_with_nul(&head[cur..cur + strlen])
                        .or(Err(Error::InvalidFormat))?;
                    pool.push(PoolValue::Str(strval.to_owned()));
                    cur += strlen;
                }
                1 => {
                    // IREP_TT_INT32: 32-bit integer
                    let data = &head[cur..cur + 4];
                    let mut bytes = [0u8; 4];
                    bytes.copy_from_slice(data);
                    let intval = i32::from_be_bytes(bytes);
                    pool.push(PoolValue::Int32(intval));
                    cur += 4;
                }
                2 => {
                    // IREP_TT_SSTR: Static string
                    let data = &head[cur..cur + 2];
                    let strlen = be16_to_u16([data[0], data[1]]) as usize + 1;
                    cur += 2;
                    let strval = CStr::from_bytes_with_nul(&head[cur..cur + strlen])
                        .or(Err(Error::InvalidFormat))?;
                    pool.push(PoolValue::SStr(strval.to_owned()));
                    cur += strlen;
                }
                3 => {
                    // IREP_TT_INT64: 64-bit integer
                    let data = &head[cur..cur + 8];
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(data);
                    let intval = i64::from_le_bytes(bytes);
                    pool.push(PoolValue::Int64(intval));
                    cur += 8;
                }
                5 => {
                    // IREP_TT_FLOAT: Float (double/float)
                    let data = &head[cur..cur + 8];
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(data);
                    let floatval = f64::from_le_bytes(bytes);
                    pool.push(PoolValue::Float(floatval));
                    cur += 8;
                }
                7 => {
                    // IREP_TT_BIGINT: Big integer (not yet fully supported)
                    let data = &head[cur..cur + 2];
                    let bigint_len = be16_to_u16([data[0], data[1]]) as usize;
                    cur += 2;
                    let bigint_data = head[cur..cur + bigint_len].to_vec();
                    pool.push(PoolValue::BigInt(bigint_data));
                    cur += bigint_len;
                }
                v => {
                    return Err(Error::UnknownPoolType(v));
                }
            }
        }

        // syms
        let data = &head[cur..cur + 2];
        let slen = be16_to_u16([data[0], data[1]]) as usize;
        cur += 2;
        for _ in 0..slen {
            let data = &head[cur..cur + 2];
            let symlen = be16_to_u16([data[0], data[1]]) as usize + 1;
            cur += 2;
            let symval = CStr::from_bytes_with_nul(&head[cur..cur + symlen])
                .or(Err(Error::InvalidFormat))?;
            syms.push(symval.to_owned());
            cur += symlen;
        }

        cur = start_cur + irep_rec_size;

        let irep = Irep {
            header: irep_record,
            insn: insns,
            plen,
            pool,
            slen,
            syms,
            catch_handlers,
            lv: Vec::new(), // Will be filled by section_lvar if present
            lines: Vec::new(),
        };
        ireps.push(irep);
    }

    Ok((irep_size, irep_header, ireps))
}

pub fn section_end(head: &[u8]) -> Result<usize, Error> {
    let header = SectionMiscHeader::from_bytes(head)?;
    // eprintln!("end section detected");
    Ok(be32_to_u32(header.size) as usize)
}

pub fn section_lvar(head: &[u8], ireps: &mut [Irep]) -> Result<(usize, LVar), Error> {
    let mut cur = 0;
    let header_size = mem::size_of::<SectionMiscHeader>();
    let header = SectionMiscHeader::from_bytes(&head[cur..cur + header_size])?;
    cur += header_size;

    // Read syms_len (4 bytes)
    let syms_len = be32_to_u32([head[cur], head[cur + 1], head[cur + 2], head[cur + 3]]) as usize;
    cur += 4;

    // Read symbols
    let mut syms = Vec::new();
    for _ in 0..syms_len {
        let str_len = be16_to_u16([head[cur], head[cur + 1]]) as usize;
        cur += 2;

        // Read string bytes (NOT null-terminated in the binary)
        let str_bytes = &head[cur..cur + str_len];
        let c_str = CString::new(str_bytes).map_err(|_| Error::InvalidFormat)?;
        syms.push(c_str);
        cur += str_len;
    }

    // Read lv records recursively
    let _ = read_lv_records(&head[cur..], ireps, 0, &syms, syms_len)?;

    let lvar = LVar { header, syms };
    Ok((be32_to_u32(lvar.header.size) as usize, lvar))
}

fn read_lv_records(
    head: &[u8],
    ireps: &mut [Irep],
    irep_idx: usize,
    syms: &[CString],
    syms_len: usize,
) -> Result<(usize, usize), Error> {
    if irep_idx >= ireps.len() {
        return Ok((0, irep_idx));
    }

    let mut cur = 0;
    let nlocals = ireps[irep_idx].nlocals();
    let rlen = ireps[irep_idx].rlen();

    if nlocals == 0 {
        return Err(Error::InvalidFormat);
    }

    // Read local variable names for this irep (nlocals - 1 entries)
    let mut lv = Vec::new();
    for _ in 0..(nlocals - 1) {
        let sym_idx = be16_to_u16([head[cur], head[cur + 1]]);
        cur += 2;

        if sym_idx == RITE_LV_NULL_MARK {
            lv.push(None);
        } else {
            let sym_idx = sym_idx as usize;
            if sym_idx >= syms_len {
                return Err(Error::InvalidFormat);
            }
            lv.push(Some(syms[sym_idx].clone()));
        }
    }

    ireps[irep_idx].lv = lv;

    // Recursively read child ireps
    let mut child_irep_idx = irep_idx + 1;
    for _ in 0..rlen {
        let (bytes_read, next_irep_idx) =
            read_lv_records(&head[cur..], ireps, child_irep_idx, syms, syms_len)?;
        cur += bytes_read;
        child_irep_idx = next_irep_idx;
    }

    Ok((cur, child_irep_idx))
}

/// Source line for an instruction byte offset, from a `lines` map.
pub fn line_at(map: &[(u32, u32)], pos: u32) -> Option<u32> {
    let mut lo = 0usize;
    let mut hi = map.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if map[mid].0 <= pos {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo == 0 { None } else { Some(map[lo - 1].1) }
}

fn u16_at(head: &[u8], cur: usize) -> Result<u16, Error> {
    if cur + 2 > head.len() {
        return Err(Error::TooShort);
    }
    Ok(be16_to_u16([head[cur], head[cur + 1]]))
}

fn u32_at(head: &[u8], cur: usize) -> Result<u32, Error> {
    if cur + 4 > head.len() {
        return Err(Error::TooShort);
    }
    Ok(be32_to_u32([
        head[cur],
        head[cur + 1],
        head[cur + 2],
        head[cur + 3],
    ]))
}

/// Parses the debug section. File names are consumed but not kept: only the
/// per-irep source line maps are stored, in the same DFS pre-order as the
/// irep records.
fn parse_debug_section(head: &[u8], ireps: &mut [Irep]) -> Result<usize, Error> {
    let header = SectionMiscHeader::from_bytes(head)?;
    let section_size = be32_to_u32(header.size) as usize;
    if head.len() < section_size {
        return Err(Error::TooShort);
    }
    // Parse within the section only: a malformed section must not read into
    // the following ones (LVAR, END).
    let sec = &head[..section_size];
    let mut cur = mem::size_of::<SectionMiscHeader>();

    // Filename table: (u16 len, bytes) entries, not null-terminated.
    let file_count = u16_at(sec, cur)? as usize;
    cur += 2;
    for _ in 0..file_count {
        let len = u16_at(sec, cur)? as usize;
        cur += 2 + len;
    }

    let mut idx = 0usize;
    cur = read_debug_record(sec, cur, ireps, &mut idx, section_size)?;

    // Tolerate a short section (trailing padding), reject an overrun.
    if cur > section_size {
        return Err(Error::InvalidFormat);
    }
    Ok(section_size)
}

fn read_debug_record(
    head: &[u8],
    mut cur: usize,
    ireps: &mut [Irep],
    irep_idx: &mut usize,
    limit: usize,
) -> Result<usize, Error> {
    if *irep_idx >= ireps.len() || cur >= limit {
        return Ok(cur);
    }
    let rlen = ireps[*irep_idx].rlen();

    let record_start = cur;
    let _record_size = u32_at(head, cur)? as usize;
    cur += 4;

    let file_count = u16_at(head, cur)? as usize;
    cur += 2;

    let mut lines: Vec<(u32, u32)> = Vec::new();
    for _ in 0..file_count {
        let start_pos = u32_at(head, cur)?;
        cur += 4;
        let _filename_idx = u16_at(head, cur)?;
        cur += 2;
        let entry_count = u32_at(head, cur)? as usize;
        cur += 4;
        let line_type = if cur < head.len() {
            head[cur]
        } else {
            return Err(Error::TooShort);
        };
        cur += 1;

        match line_type {
            // mrb_debug_line_ary: one line per pc, starting at start_pos.
            0 => {
                for i in 0..entry_count {
                    let line = u16_at(head, cur)? as u32;
                    cur += 2;
                    push_line(&mut lines, start_pos + i as u32, line);
                }
            }
            // mrb_debug_line_flat_map: (start_pos, line) pairs.
            1 => {
                for _ in 0..entry_count {
                    let pos = u32_at(head, cur)?;
                    cur += 4;
                    let line = u16_at(head, cur)? as u32;
                    cur += 2;
                    push_line(&mut lines, pos, line);
                }
            }
            // mrb_debug_line_packed_map: (pos_delta, line_delta) varint
            // pairs; entry_count is the byte length of the packed data.
            2 => {
                let data_end = cur + entry_count;
                let mut pos: u32 = 0;
                let mut line: u32 = 0;
                while cur < data_end {
                    let (delta, used) = decode_packed_int(head, cur)?;
                    cur += used;
                    pos = pos.wrapping_add(delta);
                    let (delta, used) = decode_packed_int(head, cur)?;
                    cur += used;
                    // Deltas carry sign via two's complement, matching the
                    // C decoder's unsigned wraparound adds.
                    line = line.wrapping_add(delta);
                    push_line(&mut lines, pos, line);
                }
            }
            _ => return Err(Error::InvalidFormat),
        }
    }

    ireps[*irep_idx].lines = lines;
    if cur - record_start != _record_size {
        return Err(Error::InvalidFormat);
    }
    *irep_idx += 1;
    let mut child = *irep_idx;
    for _ in 0..rlen {
        cur = read_debug_record(head, cur, ireps, &mut child, limit)?;
    }
    *irep_idx = child;
    Ok(cur)
}

/// 7-bit varint used by packed line maps (mirrors mrc_packed_int_decode).
fn decode_packed_int(head: &[u8], cur: usize) -> Result<(u32, usize), Error> {
    let mut value: u32 = 0;
    let mut shift = 0u32;
    let mut i = 0usize;
    loop {
        if cur + i >= head.len() {
            return Err(Error::TooShort);
        }
        let byte = head[cur + i];
        value |= ((byte & 0x7f) as u32) << shift;
        i += 1;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 32 {
            return Err(Error::InvalidFormat);
        }
    }
    Ok((value, i))
}

fn push_line(lines: &mut Vec<(u32, u32)>, pos: u32, line: u32) {
    if let Some((_, last)) = lines.last()
        && *last == line
    {
        return;
    }
    lines.push((pos, line));
}

pub fn section_skip(head: &[u8]) -> Result<usize, Error> {
    let header = SectionMiscHeader::from_bytes(head)?;
    Ok(be32_to_u32(header.size) as usize)
}

pub fn peek4(src: &[u8]) -> Option<[char; 4]> {
    if src.len() < 4 {
        // EoD
        return None;
    }
    if let [a, b, c, d] = src[0..4] {
        let a = char::from_u32(a as u32).unwrap();
        let b = char::from_u32(b as u32).unwrap();
        let c = char::from_u32(c as u32).unwrap();
        let d = char::from_u32(d as u32).unwrap();
        Some([a, b, c, d])
    } else {
        None
    }
}

pub fn be32_to_u32(be32: [u8; 4]) -> u32 {
    let binsize_be = unsafe { mem::transmute::<[u8; 4], u32be>(be32) };
    let binsize: u32 = binsize_be.into();
    binsize
}

pub fn be16_to_u16(be16: [u8; 2]) -> u16 {
    let binsize_be = unsafe { mem::transmute::<[u8; 2], u16be>(be16) };
    let binsize: u16 = binsize_be.into();
    binsize
}
