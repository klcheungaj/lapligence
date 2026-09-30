//! C-local copies of resume-stable coroutine frame fields (design §13.2 rule 2).
//!
//! A frame field that is written only where it is initialized and read
//! afterwards costs GCC a possibly-aliased load at every read. Such a field is
//! mirrored by a C local of the same name. The local is declared without an
//! initializer before the resume dispatch, assigned together with the field,
//! and reloaded from the frame right after every suspension point: a resume
//! jumps into the middle of a suspension macro, so the local is indeterminate
//! there and the frame is the only state that survives a return.
//!
//! Why the candidates are stable:
//! - `_llg_t` and `_llg_frame_base` are assigned once by the prologue, which
//!   the resume dispatch skips. `llg_value_scope_values` returns the scope's
//!   fixed array; no runtime path reallocates it or reassigns the field, and a
//!   frame re-entered at state 0 (a repeated call) runs the prologue again.
//! - A cell pointer (`_llg_local_N`) is assigned at its declaration and points
//!   into a scope that lives until its block ends. The declaration reassigns
//!   both copies on every execution, so a loop body stays consistent.
//!
//! Emission still verifies this from the final text instead of trusting it: a
//! candidate with any write besides its declaration, or whose address is taken,
//! stays in the frame.

use std::collections::{HashMap, HashSet};

/// A field is cached only when it is read more often than it is reloaded,
/// scaled by this factor: every reload costs one load per suspension point
/// where the field is in scope, every cached read saves one.
pub(super) const CACHE_MIN_READS_PER_RELOAD: usize = 1;

const RELOAD_PREFIX: &str = "/*__llg_reload_";
const RELOAD_SUFFIX: &str = "__*/";

#[derive(Clone, Debug)]
struct Candidate {
    ty: String,
    name: String,
    /// Assigned by the function prologue rather than by an emitted declaration.
    prologue: bool,
}

#[derive(Default)]
pub(super) struct CachedFields {
    candidates: Vec<Candidate>,
    /// Candidates whose C scope is open at the current emission point.
    scope: Vec<usize>,
    /// `scope.len()` at each open structural block.
    marks: Vec<usize>,
    /// Candidates in scope at each suspension point, indexed by placeholder.
    reloads: Vec<Vec<usize>>,
    cached: HashSet<String>,
}

impl CachedFields {
    pub(super) fn register(&mut self, ty: &str, name: &str, prologue: bool) {
        self.scope.push(self.candidates.len());
        self.candidates.push(Candidate {
            ty: ty.to_owned(),
            name: name.to_owned(),
            prologue,
        });
    }

    pub(super) fn open_block(&mut self) {
        self.marks.push(self.scope.len());
    }

    pub(super) fn close_block(&mut self) {
        if let Some(mark) = self.marks.pop() {
            self.scope.truncate(mark);
        }
    }

    /// Placeholder line emitted right after a suspension point, expanded by
    /// `finish` once it is known which candidates are cached.
    pub(super) fn reload_placeholder(&mut self) -> Option<String> {
        if self.scope.is_empty() {
            return None;
        }
        let index = self.reloads.len();
        self.reloads.push(self.scope.clone());
        Some(format!("    {RELOAD_PREFIX}{index}{RELOAD_SUFFIX}\n"))
    }

    pub(super) fn is_cached(&self, name: &str) -> bool {
        self.cached.contains(name)
    }

    /// Choose the cached set from the finished body. `is_frame_field` says
    /// whether a candidate stayed a frame field after storage narrowing; one
    /// that became a C local needs nothing.
    pub(super) fn decide(&mut self, code: &str, is_frame_field: impl Fn(&str) -> bool) {
        if self.candidates.is_empty() {
            return;
        }
        let names = self
            .candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<HashSet<_>>();
        let uses = scan_uses(code, &names);
        let mut reload_counts = vec![0usize; self.candidates.len()];
        for reload in &self.reloads {
            for &candidate in reload {
                reload_counts[candidate] += 1;
            }
        }
        for (index, candidate) in self.candidates.iter().enumerate() {
            let usage = uses
                .get(candidate.name.as_str())
                .copied()
                .unwrap_or_default();
            if is_frame_field(&candidate.name)
                && !usage.unstable
                && usage.reads > reload_counts[index] * CACHE_MIN_READS_PER_RELOAD
            {
                self.cached.insert(candidate.name.clone());
            }
        }
    }

    /// Declarations for the function entry, before the resume dispatch.
    pub(super) fn locals(&self) -> String {
        self.candidates
            .iter()
            .filter(|candidate| self.cached.contains(&candidate.name))
            .map(|candidate| format!("    {} {};\n", candidate.ty, candidate.name))
            .collect()
    }

    /// Initial copies of the fields the prologue assigns, emitted right after it.
    pub(super) fn prologue_loads<'a>(&self, access: impl Fn(&str) -> Option<&'a str>) -> String {
        self.candidates
            .iter()
            .filter(|candidate| candidate.prologue && self.cached.contains(&candidate.name))
            .filter_map(|candidate| {
                access(&candidate.name)
                    .map(|access| format!("    {} = F->{access};\n", candidate.name))
            })
            .collect()
    }

    /// Replace every reload placeholder by the loads of the cached fields that
    /// were in scope at its suspension point; a placeholder with none vanishes.
    pub(super) fn expand_reloads<'a>(
        &self,
        code: &str,
        access: impl Fn(&str) -> Option<&'a str>,
    ) -> Result<String, String> {
        let mut output = String::with_capacity(code.len());
        let mut rest = code;
        while let Some(start) = rest.find(RELOAD_PREFIX) {
            let line_start = rest[..start].rfind('\n').map_or(0, |offset| offset + 1);
            output.push_str(&rest[..line_start]);
            let suffix = &rest[start + RELOAD_PREFIX.len()..];
            let end = suffix
                .find(RELOAD_SUFFIX)
                .ok_or_else(|| "unterminated coroutine reload marker".to_owned())?;
            let index = suffix[..end]
                .parse::<usize>()
                .map_err(|_| "invalid coroutine reload marker".to_owned())?;
            let reload = self
                .reloads
                .get(index)
                .ok_or_else(|| format!("unknown coroutine reload marker {index}"))?;
            let mut line = String::new();
            for &candidate in reload {
                let name = &self.candidates[candidate].name;
                if !self.cached.contains(name) {
                    continue;
                }
                let access = access(name)
                    .ok_or_else(|| format!("cached field `{name}` lost its frame storage"))?;
                if line.is_empty() {
                    line.push_str("    ");
                } else {
                    line.push(' ');
                }
                line.push_str(&format!("{name} = F->{access};"));
            }
            if !line.is_empty() {
                output.push_str(&line);
                output.push('\n');
            }
            rest = &suffix[end + RELOAD_SUFFIX.len()..];
            rest = rest.strip_prefix('\n').unwrap_or(rest);
        }
        output.push_str(rest);
        Ok(output)
    }
}

#[derive(Clone, Copy, Default)]
struct Usage {
    reads: usize,
    /// Written after initialization, address taken or reached through a member
    /// access: the frame copy could then change behind the local.
    unstable: bool,
}

/// Count the uses of each named identifier in emitted C. The body only names
/// a field bare; its initialization is a deferred declaration marker, so every
/// occurrence counted here is a use.
fn scan_uses<'n>(code: &str, names: &HashSet<&'n str>) -> HashMap<&'n str, Usage> {
    let bytes = code.as_bytes();
    let mut uses: HashMap<&'n str, Usage> = HashMap::new();
    let mut index = 0;
    let mut quoted = None;
    let mut escaped = false;
    while index < bytes.len() {
        if let Some(quote) = quoted {
            let byte = bytes[index];
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
        } else if bytes[index..].starts_with(b"/*") {
            index = code[index + 2..]
                .find("*/")
                .map_or(bytes.len(), |offset| index + 2 + offset + 2);
        } else if matches!(bytes[index], b'\'' | b'"') {
            quoted = Some(bytes[index]);
            index += 1;
        } else if bytes[index] == b'_' || bytes[index].is_ascii_alphabetic() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
            {
                index += 1;
            }
            let ident = &code[start..index];
            let Some(&name) = names.get(ident) else {
                continue;
            };
            let usage = uses.entry(name).or_default();
            let before = code[..start].trim_end().as_bytes();
            let after = code[index..].trim_start().as_bytes();
            let member = matches!(before.last(), Some(b'.' | b'>'));
            let dereferenced = before.last() == Some(&b'*');
            let address = before.last() == Some(&b'&')
                && before.get(before.len().wrapping_sub(2)) != Some(&b'&')
                && after.first() != Some(&b'[');
            let stepped = before.ends_with(b"++") || before.ends_with(b"--");
            if member || address || stepped || (!dereferenced && writes_through(after)) {
                usage.unstable = true;
            } else {
                usage.reads += 1;
            }
        } else {
            index += 1;
        }
    }
    uses
}

/// Whether the text after an identifier assigns to it or steps it.
fn writes_through(after: &[u8]) -> bool {
    match after {
        [b'=', next, ..] => *next != b'=',
        [b'=', ..] => true,
        [b'+', b'+', ..] | [b'-', b'-', ..] => true,
        [b'<', b'<', b'=', ..] | [b'>', b'>', b'=', ..] => true,
        [b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^', b'=', ..] => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(code: &str, name: &str) -> Usage {
        let names = HashSet::from([name]);
        scan_uses(code, &names)
            .get(name)
            .copied()
            .unwrap_or_default()
    }

    #[test]
    fn plain_reads_and_element_addresses_are_reads() {
        let code = "sv4_move(&_llg_t[0], &_llg_t[1]); x = _llg_t[2]; *_llg_t = 1;";
        let found = usage(code, "_llg_t");
        assert_eq!(found.reads, 4);
        assert!(!found.unstable);
    }

    #[test]
    fn writes_address_and_member_uses_are_unstable() {
        for code in [
            "_llg_t = other;",
            "_llg_t += 1;",
            "_llg_t++;",
            "--_llg_t;",
            "f(&_llg_t);",
            "F->_llg_t = other;",
        ] {
            assert!(usage(code, "_llg_t").unstable, "{code}");
        }
        assert!(!usage("if (_llg_t == other) g();", "_llg_t").unstable);
        assert!(!usage("a && _llg_t;", "_llg_t").unstable);
    }

    #[test]
    fn literals_and_comments_are_not_uses() {
        let code = "puts(\"_llg_t\"); /* _llg_t */ g(_llg_t);";
        assert_eq!(usage(code, "_llg_t").reads, 1);
    }

    #[test]
    fn reload_placeholders_expand_to_cached_fields_in_scope_only() {
        let mut cached = CachedFields::default();
        cached.register("sv4_t*", "_llg_t", true);
        cached.open_block();
        cached.register("sv4_t*", "_llg_local_1", false);
        let inner = cached.reload_placeholder().expect("placeholder");
        cached.close_block();
        let outer = cached.reload_placeholder().expect("placeholder");
        let code =
            format!("{inner}g(_llg_t, _llg_local_1, _llg_local_1);\n{outer}h(_llg_t, _llg_t);\n");
        cached.decide(&code, |_| true);
        let access = |name: &str| match name {
            "_llg_t" => Some("_llg_t"),
            "_llg_local_1" => Some("b1._llg_local_1"),
            _ => None,
        };
        let expanded = cached.expand_reloads(&code, access).expect("expand");
        assert_eq!(
            expanded,
            "    _llg_t = F->_llg_t; _llg_local_1 = F->b1._llg_local_1;\n\
             g(_llg_t, _llg_local_1, _llg_local_1);\n    _llg_t = F->_llg_t;\nh(_llg_t, _llg_t);\n"
        );
    }

    #[test]
    fn a_field_read_no_more_than_reloaded_stays_in_the_frame() {
        let mut cached = CachedFields::default();
        cached.register("sv4_t*", "_llg_t", true);
        let first = cached.reload_placeholder().expect("placeholder");
        let second = cached.reload_placeholder().expect("placeholder");
        let code = format!("{first}{second}g(_llg_t);\n");
        cached.decide(&code, |_| true);
        assert!(!cached.is_cached("_llg_t"));
        let expanded = cached.expand_reloads(&code, |_| Some("x")).expect("expand");
        assert_eq!(expanded, "g(_llg_t);\n");
    }
}
