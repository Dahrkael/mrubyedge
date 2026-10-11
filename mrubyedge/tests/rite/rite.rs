extern crate mrubyedge;

use super::helpers::*;

#[test]
fn test_rite_parse_hello_world() {
    let code = r#"
    puts "Hello, World!"
    "#;
    let binary = mrbc_compile("hello", code);
    let rite = mrubyedge::rite::load(&binary).unwrap();

    // Check binary header
    assert_eq!(&rite.binary_header.ident, b"RITE");
    assert_eq!(&rite.binary_header.major_version, b"04");
    assert_eq!(&rite.binary_header.minor_version, b"00");

    // Check IREP section
    assert_eq!(rite.irep.len(), 1);
    let irep = &rite.irep[0];

    // Should have at least some instructions
    assert!(!irep.insn.is_empty());

    // Should have some pool values (for the string)
    assert!(irep.plen > 0);
}

#[test]
fn test_rite_parse_with_local_variables() {
    let code = r#"
    def greet(name)
      message = "Hello, #{name}!"
      puts message
      message
    end
    
    greet("Alice")
    "#;
    let binary = mrbc_compile("greet", code);
    let rite = mrubyedge::rite::load(&binary).unwrap();

    // Check binary header
    assert_eq!(&rite.binary_header.ident, b"RITE");

    // Should have multiple ireps (main + function definition)
    assert!(rite.irep.len() == 2);
    assert!(rite.irep[0].lv.is_empty());
    assert!(rite.irep[1].lv.len() == 3); // name, (&nil), message

    // Check LVAR section exists
    let lvar = rite.lvar.unwrap();
    assert!(lvar.syms.len() == 2);

    let name = rite.irep[1].lv[0].as_ref().cloned().unwrap();
    assert_eq!(&name.to_string_lossy(), "name");

    let message = rite.irep[1].lv[2].as_ref().cloned().unwrap();
    assert_eq!(&message.to_string_lossy(), "message");
}

#[test]
fn test_rite_parse_pool_values() {
    let code = r#"
    a = "string"
    b = 42
    c = 3.14
    d = 9999999999
    [a, b, c, d]
    "#;
    let binary = mrbc_compile("pool", code);
    let rite = mrubyedge::rite::load(&binary).unwrap();

    assert_eq!(&rite.binary_header.ident, b"RITE");

    let irep = &rite.irep[0];
    assert!(rite.lvar.unwrap().syms.len() == 4);

    // Should have multiple pool values
    assert!(irep.pool.len() == 3);
    assert!(irep.lv.len() == 4);

    // Check pool value types
    use mrubyedge::rite::PoolValue;
    let has_string = irep
        .pool
        .iter()
        .any(|p| matches!(p, PoolValue::Str(_) | PoolValue::SStr(_)));
    let has_float = irep.pool.iter().any(|p| matches!(p, PoolValue::Float(_)));
    let has_int64 = irep.pool.iter().any(|p| matches!(p, PoolValue::Int64(_)));

    assert!(has_string, "Should have string in pool");
    assert!(has_float, "Should have float in pool");
    assert!(has_int64, "Should have int64 in pool");
}

#[test]
fn test_rite_parse_bigint_pool_value() {
    let code = r#"
    x = 99999999999999999999999
    x
    "#;
    let binary = mrbc_compile("bigint", code);

    let rite = mrubyedge::rite::load(&binary).unwrap();

    use mrubyedge::rite::PoolValue;
    let bigint = rite.irep[0]
        .pool
        .iter()
        .find_map(|p| match p {
            PoolValue::BigInt(bytes) => Some(bytes),
            _ => None,
        })
        .expect("Should have a BigInt in pool");

    // `[base][digits]`: base 10 followed by the 23 decimal digits.
    assert_eq!(bigint.len(), 24);
    assert_eq!(bigint[0], 10);
    assert!(bigint[1..].iter().all(|&b| b == b'9'));
}

#[test]
fn test_rite_truncated_bigint_is_error_not_panic() {
    let binary = mrbc_compile("bigint", "x = 99999999999999999999999");
    let digits = [b'9'; 23];
    let digits_start = binary
        .windows(digits.len())
        .position(|w| w == digits)
        .expect("Should find the bigint digits");

    // Layout before the digits is [type=7][len:u8][base:i8].
    let len_pos = digits_start - 2;
    let mut chunk = binary.clone();
    chunk[len_pos] = 0xff;

    // An out-of-bounds length must be reported, not panic.
    assert!(mrubyedge::rite::load(&chunk).is_err());
}

#[test]
fn a_chunk_of_an_unknown_format_version_is_refused_test() {
    let binary = mrbc_compile("compiled", "1 + 1");

    // Bytes 4 and 5 of the header are the major version. mruby 3.x chunks are
    // refused too: the opcode numbering is not the one this VM decodes.
    for major in [b"02", b"03", b"05"] {
        let mut chunk = binary.clone();
        chunk[4..6].copy_from_slice(major);

        // Assert
        let err = mrubyedge::rite::load(&chunk).unwrap_err();
        assert!(matches!(err, mrubyedge::rite::Error::UnsupportedVersion(m) if &m == major));
    }
}

// The test compiler does not emit DBG sections, so the parser is exercised
// with synthetic sections spliced into a compiled chunk before END.

fn splice_before_end(mut chunk: Vec<u8>, section: &[u8]) -> Vec<u8> {
    let end_at = chunk.len() - 8; // "END\0" + size
    let total = u32::from_be_bytes(chunk[8..12].try_into().unwrap()) + section.len() as u32;
    chunk[8..12].copy_from_slice(&total.to_be_bytes());
    let mut out = chunk[..end_at].to_vec();
    out.extend_from_slice(section);
    out.extend_from_slice(&chunk[end_at..]);
    out
}

/// One-record DBG section for a single-irep chunk: filename table with one
/// entry, then a record with one file entry of the given line_type.
fn dbg_section(line_type: u8, entry_count: u32, payload: &[u8]) -> Vec<u8> {
    let mut record = Vec::new();
    record.extend_from_slice(&1u16.to_be_bytes()); // file entries in the record
    record.extend_from_slice(&0u32.to_be_bytes()); // start_pos
    record.extend_from_slice(&0u16.to_be_bytes()); // filename index
    record.extend_from_slice(&entry_count.to_be_bytes());
    record.push(line_type);
    record.extend_from_slice(payload);

    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_be_bytes()); // filename table: 1 entry
    body.extend_from_slice(&4u16.to_be_bytes());
    body.extend_from_slice(b"a.rb");
    // The record size includes its own 4 length bytes.
    body.extend_from_slice(&((record.len() as u32 + 4).to_be_bytes()));
    body.extend_from_slice(&record);

    let mut section = Vec::new();
    section.extend_from_slice(b"DBG\0");
    section.extend_from_slice(&((8 + body.len()) as u32).to_be_bytes());
    section.extend_from_slice(&body);
    section
}

fn compiled_two_line_snippet() -> Vec<u8> {
    mrbc_compile("dbg_snip", "x = 1\ny = 2\n")
}

#[test]
fn dbg_flat_map_lines_are_parsed_test() {
    // (pos, line) pairs: (0,1), (2,2).
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.extend_from_slice(&2u32.to_be_bytes());
    payload.extend_from_slice(&2u16.to_be_bytes());
    let chunk = splice_before_end(compiled_two_line_snippet(), &dbg_section(1, 2, &payload));

    let rite = mrubyedge::rite::load(&chunk).unwrap();
    assert_eq!(rite.irep[0].lines, vec![(0, 1), (2, 2)]);

    use mrubyedge::rite::line_at;
    assert_eq!(line_at(&rite.irep[0].lines, 0), Some(1));
    assert_eq!(line_at(&rite.irep[0].lines, 1), Some(1));
    assert_eq!(line_at(&rite.irep[0].lines, 2), Some(2));
    assert_eq!(line_at(&rite.irep[0].lines, 99), Some(2));
}

#[test]
fn dbg_line_ary_lines_are_parsed_test() {
    // One line per pc starting at start_pos; consecutive equal lines dedup.
    let payload = [1u16, 1, 2]
        .iter()
        .flat_map(|l| l.to_be_bytes())
        .collect::<Vec<u8>>();
    let chunk = splice_before_end(compiled_two_line_snippet(), &dbg_section(0, 3, &payload));

    let rite = mrubyedge::rite::load(&chunk).unwrap();
    assert_eq!(rite.irep[0].lines, vec![(0, 1), (2, 2)]);
}

#[test]
fn dbg_packed_map_lines_are_parsed_test() {
    // entry_count is the byte length; varint (pos_delta, line_delta) pairs.
    let payload = [0x00u8, 0x01, 0x02, 0x01];
    let chunk = splice_before_end(
        compiled_two_line_snippet(),
        &dbg_section(2, payload.len() as u32, &payload),
    );

    let rite = mrubyedge::rite::load(&chunk).unwrap();
    assert_eq!(rite.irep[0].lines, vec![(0, 1), (2, 2)]);
}

#[test]
fn dbg_record_size_mismatch_is_rejected_test() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u16.to_be_bytes());
    let mut section = dbg_section(1, 1, &payload);
    // Corrupt the record size field: layout is DBG\0(4) size(4) count(2)
    // len(2) "a.rb"(4) record_size(4), so it starts at offset 16.
    section[16] = 0xff;

    let chunk = splice_before_end(compiled_two_line_snippet(), &section);
    assert!(mrubyedge::rite::load(&chunk).is_err());
}

#[test]
fn dbg_section_shorter_than_declared_is_rejected_test() {
    let mut section = dbg_section(0, 0, &[]);
    // Declare more section bytes than exist.
    let size = u32::from_be_bytes(section[4..8].try_into().unwrap()) + 64;
    section[4..8].copy_from_slice(&size.to_be_bytes());

    let chunk = splice_before_end(compiled_two_line_snippet(), &section);
    let err = mrubyedge::rite::load(&chunk).unwrap_err();
    assert!(matches!(err, mrubyedge::rite::Error::TooShort));
}

#[test]
fn chunk_without_dbg_section_still_loads_with_empty_lines_test() {
    let binary = mrbc_compile("no_dbg", "x = 1");
    let rite = mrubyedge::rite::load(&binary).unwrap();
    assert!(rite.irep[0].lines.is_empty());
}
