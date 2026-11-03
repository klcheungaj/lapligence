//! Packed VPI catalog tables (`llg_vpi_packed_t`).
//!
//! A catalog model's object rows and expression/statement rows are written
//! as one byte blob that the runtime decodes when they are first needed (the
//! object rows on the first VPI use that can reach a design object, the code
//! rows on the first access to one), instead of as C struct initializers: compilers
//! spend most of a large catalog model's build parsing those initializers
//! (hundreds of thousands of rows of designated fields), while a string
//! literal of the same information parses in a fraction of the time.
//!
//! The blob is a string pool (NUL-terminated strings) followed by the
//! object stream and the code stream. Streams hold unsigned varints in a
//! printable alphabet, five data bits per character, least significant
//! group first, the `0x20` bit set while more groups follow; signed values
//! are zigzag coded. Symbolic C constants (`vpiModule`, flag expressions)
//! are indices into an emitted symbol table, so the emitter never restates
//! a header value. Model storage the rows point at goes into a reference
//! table, taken in stream order. The decoder is `catalog_unpack` in
//! `llg_vpi.c`; the stream layout is documented in `src/sim/rt/io.md`.

use std::collections::HashMap;

/// Bytes per blob row, `LLG_VPI_PACK_ROW` in `llg_vpi.h`: each row holds
/// `PACK_ROW - 1` payload bytes and the literal's NUL.
pub(super) const PACK_ROW: usize = 4096;
/// Payload bytes per literal piece: with every byte escaped (four source
/// characters) a piece stays well under MSVC's 16,380-character limit for
/// one string literal (C2026).
const PIECE: usize = 1024;
/// The varint alphabet: no quote, backslash or `?` (trigraphs).
pub(super) const DIGITS: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Builds the pool, the symbol and reference tables and one stream at a time.
#[derive(Default)]
pub(super) struct Pack {
    pool: Vec<u8>,
    strings: HashMap<String, u32>,
    symbols: Vec<String>,
    symbol_index: HashMap<String, u32>,
    refs: Vec<String>,
    stream: Vec<u8>,
}

/// One stream taken from a [`Pack`] (see [`Pack::take_stream`]).
pub(super) type Stream = Vec<u8>;

impl Pack {
    pub(super) fn uint(&mut self, mut value: u64) {
        loop {
            let group = (value & 0x1f) as usize;
            value >>= 5;
            if value == 0 {
                self.stream.push(DIGITS[group]);
                return;
            }
            self.stream.push(DIGITS[group | 0x20]);
        }
    }

    pub(super) fn int(&mut self, value: i64) {
        self.uint(((value << 1) ^ (value >> 63)) as u64);
    }

    pub(super) fn flag(&mut self, value: bool) {
        self.uint(u64::from(value));
    }

    /// An optional string: its pool offset + 1, 0 for none. Equal strings
    /// share one pool entry.
    pub(super) fn string(&mut self, text: Option<&str>) {
        let Some(text) = text else {
            self.uint(0);
            return;
        };
        let offset = match self.strings.get(text) {
            Some(offset) => *offset,
            None => {
                let offset = u32::try_from(self.pool.len()).unwrap_or(u32::MAX);
                self.pool.extend_from_slice(text.as_bytes());
                self.pool.push(0);
                self.strings.insert(text.to_owned(), offset);
                offset
            }
        };
        self.uint(u64::from(offset) + 1);
    }

    /// A symbolic C constant expression (an `int64_t` initializer): its
    /// index in the symbol table.
    pub(super) fn symbol(&mut self, expression: &str) {
        let next = u32::try_from(self.symbols.len()).unwrap_or(u32::MAX);
        let index = *self
            .symbol_index
            .entry(expression.to_owned())
            .or_insert_with(|| {
                self.symbols.push(expression.to_owned());
                next
            });
        self.uint(u64::from(index));
    }

    /// Append an address constant to the reference table; the stream says
    /// where the decoder takes it.
    pub(super) fn reference(&mut self, address: String) {
        self.refs.push(address);
    }

    pub(super) fn take_stream(&mut self) -> Stream {
        std::mem::take(&mut self.stream)
    }

    /// Emit the symbol and reference tables, the blob and the
    /// `llg_vpi_packed_t` named `name`, for streams `objects` and `code`.
    pub(super) fn render(&self, objects: &Stream, code: &Stream, name: &str, out: &mut String) {
        use std::fmt::Write as _;
        let symbols = if self.symbols.is_empty() {
            "NULL".to_owned()
        } else {
            out.push_str("static const int64_t llg_vpi_pack_symbols[] = {\n");
            for symbol in &self.symbols {
                let _ = writeln!(out, "    {symbol},");
            }
            out.push_str("};\n");
            "llg_vpi_pack_symbols".to_owned()
        };
        let refs = if self.refs.is_empty() {
            "NULL".to_owned()
        } else {
            out.push_str("static const void* const llg_vpi_pack_refs[] = {\n");
            for reference in &self.refs {
                let _ = writeln!(out, "    {reference},");
            }
            out.push_str("};\n");
            "llg_vpi_pack_refs".to_owned()
        };
        let mut payload = Vec::with_capacity(self.pool.len() + objects.len() + code.len());
        payload.extend_from_slice(&self.pool);
        payload.extend_from_slice(objects);
        payload.extend_from_slice(code);
        let rows = payload.chunks(PACK_ROW - 1).collect::<Vec<_>>();
        let chunks = if rows.is_empty() {
            "NULL".to_owned()
        } else {
            let _ = writeln!(
                out,
                "static const char llg_vpi_pack_chunks[{}][LLG_VPI_PACK_ROW] = {{",
                rows.len()
            );
            for row in &rows {
                for piece in row.chunks(PIECE) {
                    out.push_str("    \"");
                    escape_into(piece, out);
                    out.push_str("\"\n");
                }
                out.push_str("    ,\n");
            }
            out.push_str("};\n");
            "&llg_vpi_pack_chunks[0][0]".to_owned()
        };
        let _ = writeln!(
            out,
            "static const llg_vpi_packed_t {name} = {{ {chunks}, {}u, {}u, {}u, {}u, {symbols}, {}u, {refs}, {}u }};",
            rows.len(),
            self.pool.len(),
            objects.len(),
            code.len(),
            self.symbols.len(),
            self.refs.len(),
        );
    }
}

/// C string literal text of `bytes`: printable ASCII as is, everything else
/// (and `"`, `\`, `?`) as three-digit octal escapes, which never absorb a
/// following digit.
fn escape_into(bytes: &[u8], out: &mut String) {
    for &byte in bytes {
        match byte {
            b'"' | b'\\' | b'?' => out.push_str(&format!("\\{byte:03o}")),
            0x20..=0x7e => out.push(char::from(byte)),
            _ => out.push_str(&format!("\\{byte:03o}")),
        }
    }
}

/// Test decoder mirroring `catalog_unpack`'s stream reader.
#[cfg(test)]
pub(super) struct Reader<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) at: usize,
}

#[cfg(test)]
impl Reader<'_> {
    pub(super) fn uint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = *self.bytes.get(self.at)?;
            self.at += 1;
            let digit = DIGITS.iter().position(|digit| *digit == byte)? as u64;
            if shift >= 64 {
                return None;
            }
            value |= (digit & 0x1f) << shift;
            shift += 5;
            if digit & 0x20 == 0 {
                return Some(value);
            }
        }
    }

    pub(super) fn int(&mut self) -> Option<i64> {
        let value = self.uint()?;
        Some(((value >> 1) as i64) ^ -((value & 1) as i64))
    }
}

/// The tables `render` wrote, read back from the generated C (tests).
#[cfg(test)]
pub(super) struct Rendered {
    pub(super) symbols: Vec<String>,
    pub(super) refs: Vec<String>,
    pub(super) pool: Vec<u8>,
    pub(super) objects: Vec<u8>,
    pub(super) code: Vec<u8>,
    /// `llg_vpi_object_count`, when the output declares it.
    pub(super) object_count: Option<usize>,
}

/// A decoded `llg_vpi_conn_t`; symbols as their C text.
#[cfg(test)]
#[derive(Debug, Default, PartialEq)]
pub(super) struct Conn {
    pub(super) kind: String,
    pub(super) const_type: String,
    pub(super) object: i64,
    pub(super) offset: u64,
    pub(super) select_width: u64,
    pub(super) left: i64,
    pub(super) right: i64,
    pub(super) width: u64,
    pub(super) signed: u64,
    pub(super) code: u64,
    pub(super) direction: String,
    pub(super) value: Option<String>,
    pub(super) role: String,
    pub(super) edge: u64,
    pub(super) condition: u64,
}

/// A decoded `llg_vpi_timing_t`; `ops` names the referenced ops array.
#[cfg(test)]
#[derive(Debug, Default, PartialEq)]
pub(super) struct Timing {
    pub(super) op_count: u64,
    pub(super) ops: Option<String>,
    pub(super) declared: Vec<i64>,
    pub(super) count: u64,
    pub(super) polarity: String,
    pub(super) data_polarity: String,
    pub(super) has_ifnone: u64,
    pub(super) delay: u64,
}

/// A decoded `llg_vpi_instance_t`.
#[cfg(test)]
#[derive(Debug, Default, PartialEq)]
pub(super) struct Instance {
    pub(super) def_file: Option<String>,
    pub(super) def_line: u64,
    pub(super) library: Option<String>,
    pub(super) cell: Option<String>,
    pub(super) config: Option<String>,
    pub(super) net_type: String,
    pub(super) unconnected_drive: String,
    pub(super) cell_define: u64,
    pub(super) automatic: u64,
}

/// A decoded catalog row, mirroring `pack_objects` in `llg_vpi.c`.
#[cfg(test)]
#[derive(Debug, Default, PartialEq)]
pub(super) struct Object {
    pub(super) type_code: String,
    pub(super) name: Option<String>,
    pub(super) full_name: Option<String>,
    pub(super) definition_name: Option<String>,
    pub(super) file: Option<String>,
    pub(super) line: u64,
    pub(super) width: u64,
    pub(super) bits: u64,
    pub(super) parent: u64,
    pub(super) time_unit_fs: u64,
    pub(super) subtype: String,
    pub(super) flags: String,
    pub(super) left: i64,
    pub(super) right: i64,
    pub(super) dims: Vec<(i64, i64)>,
    pub(super) port_index: u64,
    pub(super) target: u64,
    pub(super) constant: Option<String>,
    pub(super) real_constant: f64,
    pub(super) typespec: u64,
    pub(super) detail: String,
    pub(super) conns: Vec<Conn>,
    pub(super) activation: (u64, u64),
    pub(super) timing: Option<Timing>,
    pub(super) shape: u64,
    pub(super) code: u64,
    pub(super) instance: Option<Instance>,
    pub(super) strength: Option<(String, String)>,
    /// (mask bit, reference text) of each storage pointer.
    pub(super) refs: Vec<(u64, String)>,
}

/// A decoded `llg_vpi_code_t`, mirroring `pack_code` in `llg_vpi.c`.
#[cfg(test)]
#[derive(Debug, Default, PartialEq)]
pub(super) struct Code {
    pub(super) type_code: String,
    pub(super) subtype: String,
    pub(super) role: String,
    pub(super) qualifier: String,
    pub(super) flags: String,
    pub(super) width: u64,
    pub(super) file: u64,
    pub(super) line: i64,
    pub(super) object: i64,
    pub(super) scope: i64,
    pub(super) first: i64,
    pub(super) count: u64,
    pub(super) data: u64,
    pub(super) typespec: u64,
    pub(super) span: u64,
}

#[cfg(test)]
impl Rendered {
    /// Parse the output of [`Pack::render`] (one model's catalog).
    pub(super) fn parse(out: &str) -> Self {
        let table = |head: &str| -> Vec<String> {
            let Some(start) = out.find(head) else {
                return Vec::new();
            };
            let body = &out[start + head.len()..];
            body[..body.find("};\n").unwrap()]
                .lines()
                .filter_map(|line| line.trim().strip_suffix(','))
                .map(str::to_owned)
                .collect()
        };
        let symbols = table("static const int64_t llg_vpi_pack_symbols[] = {\n");
        let refs = table("static const void* const llg_vpi_pack_refs[] = {\n");
        let mut payload = Vec::new();
        if let Some(start) = out.find("[LLG_VPI_PACK_ROW] = {\n") {
            let body = &out[start..];
            for line in body[..body.find("};\n").unwrap()].lines().skip(1) {
                let line = line.trim();
                if let Some(literal) = line.strip_prefix('"').and_then(|l| l.strip_suffix('"')) {
                    let bytes = literal.as_bytes();
                    let mut at = 0;
                    while at < bytes.len() {
                        if bytes[at] == b'\\' {
                            let digits = std::str::from_utf8(&bytes[at + 1..at + 4]).unwrap();
                            payload.push(u8::from_str_radix(digits, 8).unwrap());
                            at += 4;
                        } else {
                            payload.push(bytes[at]);
                            at += 1;
                        }
                    }
                }
            }
        }
        let value = out
            .lines()
            .find(|line| line.starts_with("static const llg_vpi_packed_t "))
            .expect("packed value");
        let fields = value[value.find("= {").unwrap() + 3..]
            .trim_end_matches("};")
            .trim_end_matches(" }")
            .split(',')
            .map(|field| field.trim().trim_end_matches('u').to_owned())
            .collect::<Vec<_>>();
        let number = |index: usize| fields[index].parse::<usize>().unwrap();
        let (pool, objects) = (number(2), number(3));
        let code = number(4);
        assert_eq!(payload.len(), pool + objects + code);
        let object_count = out
            .lines()
            .find_map(|line| line.strip_prefix("static const size_t llg_vpi_object_count = "))
            .and_then(|count| {
                count
                    .trim_end_matches(";")
                    .trim_end_matches('u')
                    .parse()
                    .ok()
            });
        Self {
            object_count,
            symbols,
            refs,
            pool: payload[..pool].to_vec(),
            objects: payload[pool..pool + objects].to_vec(),
            code: payload[pool + objects..].to_vec(),
        }
    }

    fn text(&self, offset: u64) -> Option<String> {
        let offset = usize::try_from(offset.checked_sub(1)?).ok()?;
        let end = self.pool[offset..].iter().position(|byte| *byte == 0)? + offset;
        Some(String::from_utf8(self.pool[offset..end].to_vec()).unwrap())
    }

    /// The object rows; panics where the C decoder would reject.
    pub(super) fn objects(&self, rows: usize) -> Vec<Object> {
        let mut r = Reader {
            bytes: &self.objects,
            at: 0,
        };
        let mut next_ref = 0;
        let u = |r: &mut Reader| r.uint().expect("uint");
        let i = |r: &mut Reader| r.int().expect("int");
        let sym = |r: &mut Reader| self.symbols[r.uint().expect("symbol") as usize].clone();
        let text = |r: &mut Reader| self.text(r.uint().expect("string"));
        let totals = (0..5).map(|_| u(&mut r)).collect::<Vec<_>>();
        let mut objects = Vec::new();
        for _ in 0..rows {
            let mut object = Object {
                type_code: sym(&mut r),
                name: text(&mut r),
                full_name: text(&mut r),
                definition_name: text(&mut r),
                file: text(&mut r),
                line: u(&mut r),
                width: u(&mut r),
                bits: u(&mut r),
                parent: u(&mut r),
                time_unit_fs: u(&mut r),
                subtype: sym(&mut r),
                flags: sym(&mut r),
                left: i(&mut r),
                right: i(&mut r),
                ..Object::default()
            };
            for _ in 0..u(&mut r) {
                object.dims.push((i(&mut r), i(&mut r)));
            }
            object.port_index = u(&mut r);
            object.target = u(&mut r);
            object.constant = text(&mut r);
            object.real_constant = f64::from_bits(u(&mut r));
            object.typespec = u(&mut r);
            object.detail = sym(&mut r);
            for _ in 0..u(&mut r) {
                object.conns.push(Conn {
                    kind: sym(&mut r),
                    const_type: sym(&mut r),
                    object: i(&mut r),
                    offset: u(&mut r),
                    select_width: u(&mut r),
                    left: i(&mut r),
                    right: i(&mut r),
                    width: u(&mut r),
                    signed: u(&mut r),
                    code: u(&mut r),
                    direction: sym(&mut r),
                    value: text(&mut r),
                    role: sym(&mut r),
                    edge: u(&mut r),
                    condition: u(&mut r),
                });
            }
            object.activation = (u(&mut r), u(&mut r));
            if u(&mut r) != 0 {
                let mut timing = Timing {
                    op_count: u(&mut r),
                    ..Timing::default()
                };
                if timing.op_count != 0 {
                    timing.ops = Some(self.refs[next_ref].clone());
                    next_ref += 1;
                }
                for _ in 0..u(&mut r) {
                    timing.declared.push(i(&mut r));
                }
                timing.count = u(&mut r);
                timing.polarity = sym(&mut r);
                timing.data_polarity = sym(&mut r);
                timing.has_ifnone = u(&mut r);
                timing.delay = u(&mut r);
                object.timing = Some(timing);
            }
            object.shape = u(&mut r);
            object.code = u(&mut r);
            if u(&mut r) != 0 {
                object.instance = Some(Instance {
                    def_file: text(&mut r),
                    def_line: u(&mut r),
                    library: text(&mut r),
                    cell: text(&mut r),
                    config: text(&mut r),
                    net_type: sym(&mut r),
                    unconnected_drive: sym(&mut r),
                    cell_define: u(&mut r),
                    automatic: u(&mut r),
                });
            }
            if u(&mut r) != 0 {
                object.strength = Some((sym(&mut r), sym(&mut r)));
            }
            let mask = u(&mut r);
            for bit in 0..6 {
                if mask & (1 << bit) != 0 {
                    object.refs.push((1 << bit, self.refs[next_ref].clone()));
                    next_ref += 1;
                }
            }
            objects.push(object);
        }
        assert_eq!(r.at, self.objects.len(), "object stream fully read");
        assert_eq!(next_ref, self.refs.len(), "every reference taken");
        let conns = objects.iter().map(|o| o.conns.len() as u64).sum::<u64>();
        let dims = objects.iter().map(|o| o.dims.len() as u64).sum::<u64>();
        let instances = objects.iter().filter(|o| o.instance.is_some()).count() as u64;
        let timings = objects.iter().filter(|o| o.timing.is_some()).count() as u64;
        let declared = objects
            .iter()
            .filter_map(|o| o.timing.as_ref())
            .map(|t| t.declared.len() as u64)
            .sum::<u64>();
        assert_eq!(totals, [conns, dims, instances, timings, declared]);
        objects
    }

    /// The code rows; panics where the C decoder would reject.
    pub(super) fn code(&self, rows: usize) -> Vec<Code> {
        let mut r = Reader {
            bytes: &self.code,
            at: 0,
        };
        let u = |r: &mut Reader| r.uint().expect("uint");
        let i = |r: &mut Reader| r.int().expect("int");
        let sym = |r: &mut Reader| self.symbols[r.uint().expect("symbol") as usize].clone();
        let (mut line, mut scope) = (0i64, 0i64);
        let mut code = Vec::new();
        for row in 0..rows {
            let mut node = Code {
                type_code: sym(&mut r),
                subtype: sym(&mut r),
                role: sym(&mut r),
                qualifier: sym(&mut r),
                flags: sym(&mut r),
                width: u(&mut r),
                file: u(&mut r),
                ..Code::default()
            };
            line += i(&mut r);
            node.line = line;
            node.object = i(&mut r);
            scope += i(&mut r);
            node.scope = scope;
            node.first = row as i64 + i(&mut r);
            node.count = u(&mut r);
            node.data = u(&mut r);
            node.typespec = u(&mut r);
            node.span = u(&mut r);
            code.push(node);
        }
        assert_eq!(r.at, self.code.len(), "code stream fully read");
        code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode the C literal text `render` wrote back into payload bytes.
    fn unescape(literal: &str) -> Vec<u8> {
        let bytes = literal.as_bytes();
        let mut out = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at] == b'\\' {
                let digits = std::str::from_utf8(&bytes[at + 1..at + 4]).unwrap();
                out.push(u8::from_str_radix(digits, 8).unwrap());
                at += 4;
            } else {
                out.push(bytes[at]);
                at += 1;
            }
        }
        out
    }

    #[test]
    fn varints_round_trip_at_group_boundaries() {
        let unsigned = [
            0u64,
            1,
            31,
            32,
            1023,
            1024,
            u64::from(u32::MAX),
            u64::MAX - 1,
            u64::MAX,
        ];
        let signed = [0i64, -1, 1, -16, 16, i64::MIN, i64::MAX, -(1 << 40)];
        let mut pack = Pack::default();
        for value in unsigned {
            pack.uint(value);
        }
        for value in signed {
            pack.int(value);
        }
        let stream = pack.take_stream();
        assert!(stream.iter().all(|byte| DIGITS.contains(byte)));
        let mut reader = Reader {
            bytes: &stream,
            at: 0,
        };
        for value in unsigned {
            assert_eq!(reader.uint(), Some(value));
        }
        for value in signed {
            assert_eq!(reader.int(), Some(value));
        }
        assert_eq!(reader.at, stream.len());
        // A truncated varint is no value.
        let mut reader = Reader {
            bytes: &stream[..1],
            at: 0,
        };
        assert_eq!(reader.uint(), Some(0));
        let mut long = Pack::default();
        long.uint(u64::MAX);
        let long = long.take_stream();
        let mut reader = Reader {
            bytes: &long[..long.len() - 1],
            at: 0,
        };
        assert_eq!(reader.uint(), None);
    }

    #[test]
    fn strings_symbols_and_rows_round_trip_through_the_literal() {
        let mut pack = Pack::default();
        let texts = [
            "top.u[3].q",
            "quote\" back\\ query?? tab\t nl\n",
            "caf\u{e9} \u{1f}\u{1e}",
            "",
        ];
        for text in texts {
            pack.string(Some(text));
        }
        pack.string(None);
        pack.string(Some(texts[0]));
        pack.symbol("vpiModule");
        pack.symbol("LLG_VPI_OBJ_TOP | LLG_VPI_OBJ_RANGE");
        pack.symbol("vpiModule");
        // Long enough to span several blob rows and literal pieces.
        for value in 0..5000u64 {
            pack.uint(value * 7919);
        }
        let objects = pack.take_stream();
        pack.int(-5);
        let code = pack.take_stream();
        pack.reference("&G_a".to_owned());
        let mut out = String::new();
        pack.render(&objects, &code, "llg_vpi_packed_value", &mut out);

        // The blob rows: each row's pieces concatenate to at most
        // PACK_ROW - 1 bytes; every piece is a short literal.
        let start = out.find("[LLG_VPI_PACK_ROW] = {\n").unwrap();
        let body = &out[start..out[start..].find("};\n").unwrap() + start];
        let mut payload = Vec::new();
        let mut row = Vec::new();
        let mut rows = 0;
        for line in body.lines().skip(1) {
            let line = line.trim();
            if line == "," {
                assert!(row.len() < PACK_ROW);
                payload.append(&mut row);
                rows += 1;
            } else {
                let literal = line.strip_prefix('"').unwrap().strip_suffix('"').unwrap();
                assert!(literal.len() <= 4 * PIECE);
                assert!(!literal.contains('?'));
                row.extend(unescape(literal));
            }
        }
        assert!(rows > 1);
        assert_eq!(payload.len(), pack.pool.len() + objects.len() + code.len());
        assert!(out.contains(&format!(
            "{{ &llg_vpi_pack_chunks[0][0], {rows}u, {}u, {}u, {}u, llg_vpi_pack_symbols, 2u, llg_vpi_pack_refs, 1u }}",
            pack.pool.len(),
            objects.len(),
            code.len()
        )));
        assert!(out.contains("    vpiModule,\n    LLG_VPI_OBJ_TOP | LLG_VPI_OBJ_RANGE,\n"));
        assert!(out.contains("    &G_a,\n"));

        let (pool, streams) = payload.split_at(pack.pool.len());
        let text_at = |offset: u64| -> Option<String> {
            let offset = usize::try_from(offset.checked_sub(1)?).ok()?;
            let end = pool[offset..].iter().position(|byte| *byte == 0)? + offset;
            Some(String::from_utf8(pool[offset..end].to_vec()).unwrap())
        };
        let mut reader = Reader {
            bytes: &streams[..objects.len()],
            at: 0,
        };
        for text in texts {
            assert_eq!(text_at(reader.uint().unwrap()).as_deref(), Some(text));
        }
        assert_eq!(reader.uint(), Some(0));
        let shared = reader.uint().unwrap();
        assert_eq!(text_at(shared).as_deref(), Some(texts[0]));
        assert_eq!(shared, 1);
        assert_eq!(reader.uint(), Some(0));
        assert_eq!(reader.uint(), Some(1));
        assert_eq!(reader.uint(), Some(0));
        for value in 0..5000u64 {
            assert_eq!(reader.uint(), Some(value * 7919));
        }
        assert_eq!(reader.at, objects.len());
        let mut reader = Reader {
            bytes: &streams[objects.len()..],
            at: 0,
        };
        assert_eq!(reader.int(), Some(-5));
    }

    #[test]
    fn row_width_matches_the_runtime_header() {
        let header = include_str!("../../../rt/llg_vpi.h");
        assert!(header.contains(&format!("#define LLG_VPI_PACK_ROW {PACK_ROW}u")));
        let digits = std::str::from_utf8(DIGITS).unwrap();
        let runtime = include_str!("../../../rt/llg_vpi.c");
        assert!(runtime.contains(&format!("\"{digits}\"")));
    }
}
