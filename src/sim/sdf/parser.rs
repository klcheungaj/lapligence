//! Versioned streaming SDF parser (OVI SDF 3.0 "SDF File Syntax"; OVI SDF
//! 2.1 for files that declare version 2.1).
//!
//! [`SdfReader::new`] reads the header; [`SdfReader::next_cell`] returns
//! one CELL entry at a time, so memory does not grow with the file. The
//! grammar is selected by the SDFVERSION entry: the first of `1.0`, `2.0`,
//! `2.1`, `3.0` or `4.0` found in its string (OVI SDF 3.0 3-3). Versions
//! 2.1 and 3.0 are read with their published grammars; for the others no
//! approved grammar is available and the file is rejected.
//!
//! Differences handled by version:
//!
//! | Construct | 2.1 | 3.0 |
//! | --- | --- | --- |
//! | repeated `(INSTANCE a)(INSTANCE b)`, `(INSTANCE p) port` port paths, CORRELATION, GLOBALPATHPULSE, NETDELAY, constraints in TIMINGCHECK | yes | no |
//! | PATHPULSEPERCENT, CONDELSE, RETAIN, COND/SCOND/CCOND labels, delvals with pulse limits, REMOVAL, RECREM, SCOND/CCOND, TIMINGENV, empty `()` values | no | yes |
//! | negative SETUP, HOLD, RECOVERY limits | yes | no |

use super::expr::{split_identifier, ExprParser, SdfBits};
use super::lexer::{Lexer, Mode, Tok, Token};
use super::number::{NumberError, SdfNumber, SdfValue};
use super::plan::SdfWarning;
use super::syntax::*;
use super::{SdfError, SdfLimits, SdfPos};
use std::fmt;
use std::io::BufRead;

/// A whole file, for callers that want every cell at once.
#[derive(Clone, Debug, PartialEq)]
pub struct SdfFile {
    pub header: SdfHeader,
    pub cells: Vec<SdfCell>,
    pub warnings: Vec<SdfWarning>,
}

/// Parse a complete file.
pub fn parse<R: BufRead>(input: R, limits: SdfLimits) -> Result<SdfFile, SdfError> {
    let mut reader = SdfReader::new(input, limits)?;
    let mut cells = Vec::new();
    while let Some(cell) = reader.next_cell()? {
        cells.push(cell);
    }
    let header = reader.header().clone();
    Ok(SdfFile {
        header,
        cells,
        warnings: reader.take_warnings(),
    })
}

/// The header entries after SDFVERSION, in the order the grammar requires.
const HEADER_ORDER: &[&str] = &[
    "DESIGN",
    "DATE",
    "VENDOR",
    "PROGRAM",
    "VERSION",
    "DIVIDER",
    "VOLTAGE",
    "PROCESS",
    "TEMPERATURE",
    "TIMESCALE",
];

/// Whether a value may be negative: an rvalue (RNUMBER) or a value
/// (NUMBER).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sign {
    Any,
    NonNegative,
}

/// Which written forms of numbers the file has used so far.
#[derive(Clone, Copy, Default)]
struct Forms {
    single: bool,
    triple: bool,
    warned: bool,
}

pub struct SdfReader<R> {
    lexer: Lexer<R>,
    limits: SdfLimits,
    header: SdfHeader,
    /// `(CELL` has been read and its body is next.
    cell_open: Option<SdfPos>,
    finished: bool,
    forms: Forms,
    warnings: Vec<SdfWarning>,
}

fn keyword_is(word: &str, keyword: &str) -> bool {
    word.eq_ignore_ascii_case(keyword)
}

impl<R: BufRead> SdfReader<R> {
    /// Read up to the first CELL entry.
    pub fn new(input: R, limits: SdfLimits) -> Result<Self, SdfError> {
        let mut reader = Self {
            lexer: Lexer::new(input, limits),
            limits,
            header: SdfHeader {
                version: SdfVersion::V3_0,
                version_text: String::new(),
                design: None,
                date: None,
                vendor: None,
                program: None,
                program_version: None,
                divider: '.',
                voltage: None,
                process: None,
                temperature: None,
                timescale: SdfTimescale::default(),
            },
            cell_open: None,
            finished: false,
            forms: Forms::default(),
            warnings: Vec::new(),
        };
        reader.read_header()?;
        Ok(reader)
    }

    pub fn header(&self) -> &SdfHeader {
        &self.header
    }

    pub fn version(&self) -> SdfVersion {
        self.header.version
    }

    /// Warnings found while reading (forms the specification discourages).
    pub fn take_warnings(&mut self) -> Vec<SdfWarning> {
        std::mem::take(&mut self.warnings)
    }

    fn v21(&self) -> bool {
        self.header.version == SdfVersion::V2_1
    }

    fn warn(&mut self, pos: SdfPos, message: impl Into<String>) {
        self.warnings.push(SdfWarning::new(pos, message));
    }

    /// A construct of the other version: read as that version defines it,
    /// with a warning ("File readers should use the SDFVERSION entry if
    /// they are unable to adapt to the differences automatically", OVI SDF
    /// 3.0 1-2; the versions do not give one construct two meanings).
    fn other_version(&mut self, what: &str, pos: SdfPos) {
        let (declared, owner) = if self.v21() {
            ("2.1", "3.0")
        } else {
            ("3.0", "2.1")
        };
        self.warn(
            pos,
            format!("{what} belongs to SDF {owner}, not SDF {declared}; it is read as SDF {owner} defines it"),
        );
    }

    // ----- token helpers -------------------------------------------------

    fn next(&mut self) -> Result<Token, SdfError> {
        self.lexer.next(Mode::Structure)
    }

    fn peek(&mut self) -> Result<&Tok, SdfError> {
        Ok(&self.lexer.peek(Mode::Structure)?.tok)
    }

    fn unexpected(token: &Token, expected: impl fmt::Display) -> SdfError {
        SdfError::new(
            token.pos,
            format!("expected {expected}, found {}", token.tok.describe()),
        )
    }

    // Contexts are `Display` so that callers pass `format_args!` and only an
    // error formats them.
    fn open(&mut self, context: impl fmt::Display) -> Result<SdfPos, SdfError> {
        let token = self.next()?;
        match token.tok {
            Tok::LParen => Ok(token.pos),
            _ => Err(Self::unexpected(&token, format_args!("`(` {context}"))),
        }
    }

    fn close(&mut self, context: impl fmt::Display) -> Result<(), SdfError> {
        let token = self.next()?;
        match token.tok {
            Tok::RParen => Ok(()),
            _ => Err(Self::unexpected(&token, format_args!("`)` {context}"))),
        }
    }

    /// The keyword after an opening parenthesis, upper-cased.
    fn keyword(&mut self, context: impl fmt::Display) -> Result<(String, SdfPos), SdfError> {
        let token = self.next()?;
        match token.tok {
            Tok::Word(mut word) => {
                word.make_ascii_uppercase();
                Ok((word, token.pos))
            }
            _ => Err(Self::unexpected(
                &token,
                format_args!("a keyword {context}"),
            )),
        }
    }

    fn qstring(&mut self, what: impl fmt::Display) -> Result<String, SdfError> {
        let token = self.next()?;
        match token.tok {
            Tok::QString(text) => Ok(text),
            _ => Err(Self::unexpected(
                &token,
                format_args!("the quoted string of {what}"),
            )),
        }
    }

    fn word(&mut self, what: impl fmt::Display) -> Result<(String, SdfPos), SdfError> {
        let token = self.next()?;
        match token.tok {
            Tok::Word(word) => Ok((word, token.pos)),
            _ => Err(Self::unexpected(&token, what)),
        }
    }

    // ----- numbers and values -------------------------------------------

    fn number(&mut self, text: &str, pos: SdfPos, sign: Sign) -> Result<SdfNumber, SdfError> {
        let number = SdfNumber::parse(text).map_err(|error| {
            SdfError::new(
                pos,
                match error {
                    NumberError::Malformed => format!("`{text}` is not a number"),
                    NumberError::TooManyDigits => format!(
                        "the number `{text}` has more than {} significant digits",
                        super::number::MAX_SIGNIFICANT_DIGITS
                    ),
                    NumberError::ExponentRange => format!(
                        "the number `{text}` is out of range (exponent beyond {})",
                        super::number::MAX_DECIMAL_EXPONENT
                    ),
                },
            )
        })?;
        // value ::= NUMBER, which has no sign: a negative value counts as 0.
        if sign == Sign::NonNegative && number.is_negative() {
            self.warn(
                pos,
                format!("the value `{text}` must not be negative; 0 is used"),
            );
            return Ok(SdfNumber::ZERO);
        }
        Ok(number)
    }

    /// A NUMBER or RNUMBER, or a min:typ:max triple with both colons and at
    /// least one member.
    fn value_word(
        &mut self,
        text: &str,
        pos: SdfPos,
        sign: Sign,
        track: bool,
    ) -> Result<Option<SdfValue>, SdfError> {
        let value = if text.contains(':') {
            if text.bytes().filter(|&byte| byte == b':').count() != 2 {
                return Err(SdfError::new(
                    pos,
                    format!("the triple `{text}` must have exactly two colons"),
                ));
            }
            let mut members = [None; 3];
            for (member, part) in members.iter_mut().zip(text.split(':')) {
                if !part.is_empty() {
                    *member = Some(self.number(part, pos, sign)?);
                }
            }
            if members.iter().all(Option::is_none) {
                // `(::)` gives no data set a value, like `()`.
                self.warn(
                    pos,
                    format!("the triple `{text}` has no number; it is read as an empty value"),
                );
                return Ok(None);
            }
            SdfValue::Triple(members)
        } else {
            SdfValue::Single(self.number(text, pos, sign)?)
        };
        if track {
            self.track_form(&value, pos);
        }
        Ok(Some(value))
    }

    /// "The use of single RNUMBERs and rtriples should not be mixed in the
    /// same SDF file" (OVI SDF 3.0 3-18): warn once.
    fn track_form(&mut self, value: &SdfValue, pos: SdfPos) {
        match value {
            SdfValue::Single(_) => self.forms.single = true,
            SdfValue::Triple(_) => self.forms.triple = true,
        }
        if self.forms.single && self.forms.triple && !self.forms.warned {
            self.forms.warned = true;
            self.warnings.push(SdfWarning::new(
                pos,
                "single values and min:typ:max triples are mixed in one file; a single value \
                 applies to every data set",
            ));
        }
    }

    /// A value or rvalue in parentheses: `(1)`, `(1:2:3)` or, in 3.0, `()`.
    fn value(&mut self, sign: Sign, what: &str) -> Result<Option<SdfValue>, SdfError> {
        let open = self.open(format_args!("of {what}"))?;
        self.value_after_open(open, sign, what)
    }

    fn value_after_open(
        &mut self,
        open: SdfPos,
        sign: Sign,
        what: &str,
    ) -> Result<Option<SdfValue>, SdfError> {
        let token = self.next()?;
        match token.tok {
            Tok::RParen => {
                if self.v21() {
                    self.other_version(&format!("an empty value for {what}"), open);
                }
                Ok(None)
            }
            Tok::Word(word) => {
                let value = self.value_word(&word, token.pos, sign, true)?;
                self.close(format_args!("after {what}"))?;
                Ok(value)
            }
            _ => Err(Self::unexpected(
                &token,
                format_args!("a number or triple of {what}"),
            )),
        }
    }

    /// An rvalue inside a delay list, where `()` is allowed in both
    /// versions ("Any rvalue can be null", OVI SDF 2.1 1-4).
    fn delay_rvalue_after_open(&mut self) -> Result<Option<SdfValue>, SdfError> {
        let token = self.next()?;
        match token.tok {
            Tok::RParen => Ok(None),
            Tok::Word(word) => {
                let value = self.value_word(&word, token.pos, Sign::Any, true)?;
                self.close("after a delay value")?;
                Ok(value)
            }
            _ => Err(Self::unexpected(&token, "a delay value")),
        }
    }

    /// One delval after its opening parenthesis: an rvalue, or (3.0) two or
    /// three rvalues in parentheses.
    fn delval_after_open(&mut self) -> Result<SdfDelval, SdfError> {
        if *self.peek()? != Tok::LParen {
            return Ok(SdfDelval {
                delay: self.delay_rvalue_after_open()?,
                pulse: SdfPulse::Unspecified,
            });
        }
        let token = self.next()?;
        if self.v21() {
            self.other_version("a delay value with pulse limits `((d) (r) (e))`", token.pos);
        }
        let delay = self.delay_rvalue_after_open()?;
        let mut limits = Vec::new();
        loop {
            let token = self.next()?;
            match token.tok {
                Tok::RParen => break,
                Tok::LParen if limits.len() < 2 => limits.push(self.delay_rvalue_after_open()?),
                Tok::LParen => {
                    return Err(SdfError::new(
                        token.pos,
                        "a delay value has at most a delay, a reject limit and an error limit",
                    ))
                }
                _ => return Err(Self::unexpected(&token, "`(` or `)` in a delay value")),
            }
        }
        let pulse = match limits.as_slice() {
            [] => {
                return Err(SdfError::new(
                    token.pos,
                    "a parenthesized delay value needs a pulse limit after the delay",
                ))
            }
            // "The second specifies both the pulse rejection limit ... and
            // the X-limit" (OVI SDF 3.0 3-18).
            [limit] => SdfPulse::Limits(Box::new(SdfPulseLimits {
                reject: *limit,
                error: *limit,
            })),
            [reject, error] => SdfPulse::Limits(Box::new(SdfPulseLimits {
                reject: *reject,
                error: *error,
            })),
            _ => unreachable!("at most two limits are read"),
        };
        Ok(SdfDelval { delay, pulse })
    }

    /// A delval_list of 1 to 12 delvals, up to the closing `)` of its entry
    /// (not consumed).
    fn delval_list(&mut self, what: &str) -> Result<SdfDelvalList, SdfError> {
        let mut values = Vec::new();
        let mut thirteenth = None;
        while *self.peek()? == Tok::LParen {
            let token = self.next()?;
            if values.len() == 12 {
                thirteenth.get_or_insert(token.pos);
            }
            values.push(self.delval_after_open()?);
        }
        self.too_many_values(thirteenth, what);
        self.finish_delval_list(values, what)
    }

    /// More than 12 delay values: the entry is read but not annotated (the
    /// resolver skips lists longer than 12).
    fn too_many_values(&mut self, thirteenth: Option<SdfPos>, what: &str) {
        if let Some(pos) = thirteenth {
            self.warn(
                pos,
                format!("{what} has more than 12 delay values; it is not annotated"),
            );
        }
    }

    fn finish_delval_list(
        &mut self,
        values: Vec<SdfDelval>,
        what: &str,
    ) -> Result<SdfDelvalList, SdfError> {
        if values.is_empty() {
            let token = self.next()?;
            return Err(Self::unexpected(
                &token,
                format_args!("the delay values of {what}"),
            ));
        }
        Ok(SdfDelvalList { values })
    }

    // ----- names ---------------------------------------------------------

    /// Split a word at unescaped hierarchy dividers into identifiers. An
    /// empty level or an unescaped divider of the other kind is a warning
    /// (the path then names nothing, or the character is part of the name).
    fn path_components(
        &mut self,
        word: &str,
        pos: SdfPos,
        what: &str,
    ) -> Result<Vec<SdfIdent>, SdfError> {
        let divider = self.header.divider as u8;
        let other = if divider == b'.' { b'/' } else { b'.' };
        let bytes = word.as_bytes();
        let mut parts = Vec::new();
        let mut start = 0;
        let mut index = 0;
        let mut other_found = false;
        while index < bytes.len() {
            match bytes[index] {
                b'\\' => index += 2,
                byte if byte == divider => {
                    parts.push(&word[start..index]);
                    start = index + 1;
                    index += 1;
                }
                byte => {
                    other_found |= byte == other;
                    index += 1;
                }
            }
        }
        parts.push(&word[start..]);
        if parts.len() > self.limits.max_path_components {
            return Err(SdfError::new(
                pos,
                format!(
                    "{what} has more than {} hierarchy levels",
                    self.limits.max_path_components
                ),
            ));
        }
        if other_found {
            self.warn(
                pos,
                format!(
                    "{what} `{word}` has an unescaped `{}`, which is read as part of a name \
                     (the hierarchy divider is `{}`)",
                    other as char, self.header.divider
                ),
            );
        }
        let mut idents = Vec::with_capacity(parts.len());
        let mut empty_warned = false;
        for part in parts {
            if part.is_empty() {
                // An empty name matches no scope or port.
                if !empty_warned {
                    empty_warned = true;
                    self.warn(
                        pos,
                        format!(
                            "{what} `{word}` has an empty hierarchy level (divider `{}`)",
                            self.header.divider
                        ),
                    );
                }
                idents.push(SdfIdent {
                    name: String::new(),
                    bits: None,
                });
                continue;
            }
            if part == "*" {
                return Err(SdfError::new(
                    pos,
                    format!(
                        "{what} `{word}`: the wildcard `*` cannot be part of a hierarchical path"
                    ),
                ));
            }
            let escaped;
            let part = if other_found {
                escaped = escape_unescaped(part, other);
                escaped.as_str()
            } else {
                part
            };
            let (name, bits) =
                split_identifier(part, what).map_err(|message| SdfError::new(pos, message))?;
            idents.push(SdfIdent { name, bits });
        }
        Ok(idents)
    }

    /// Instance components select one instance each: a range selects none
    /// (the resolver finds no such instance).
    fn check_instance_components(&mut self, components: &[SdfIdent], pos: SdfPos, word: &str) {
        if components
            .iter()
            .any(|component| matches!(component.bits, Some(SdfBits::Range(..))))
        {
            self.warn(
                pos,
                format!(
                    "the instance path `{word}` selects a range of instances, not one instance"
                ),
            );
        }
    }

    /// A port_instance from its word: `port` or `PATH HCHAR port`.
    fn port_from_word(&mut self, word: &str, pos: SdfPos) -> Result<SdfPortRef, SdfError> {
        let mut components = self.path_components(word, pos, "the port")?;
        let port = components.pop().expect("a split yields one part");
        self.check_instance_components(&components, pos, word);
        Ok(SdfPortRef {
            pos,
            path: components,
            port,
        })
    }

    /// A port_instance. In 2.1 it may also be `(INSTANCE path) port`.
    fn port_instance(&mut self, what: &str) -> Result<SdfPortRef, SdfError> {
        if *self.peek()? == Tok::LParen {
            let open = self.next()?;
            if !self.v21() {
                self.other_version("a port path `(INSTANCE path) port`", open.pos);
            }
            let (keyword, pos) = self.keyword("of a port path")?;
            if keyword != "INSTANCE" {
                return Err(SdfError::new(
                    pos,
                    format!("expected `INSTANCE` before a port, found `{keyword}`"),
                ));
            }
            let mut path = self.optional_instance_path()?;
            self.close("after an INSTANCE path")?;
            let (word, word_pos) = self.word(format_args!("the port of {what}"))?;
            let mut port = self.port_from_word(&word, word_pos)?;
            path.append(&mut port.path);
            port.path = path;
            port.pos = open.pos;
            return Ok(port);
        }
        let (word, pos) = self.word(format_args!("the port of {what}"))?;
        self.port_from_word(&word, pos)
    }

    /// The path of an INSTANCE entry up to its `)` (not consumed); empty for
    /// `(INSTANCE)`.
    fn optional_instance_path(&mut self) -> Result<Vec<SdfIdent>, SdfError> {
        if let Tok::Word(_) = self.peek()? {
            let (word, pos) = self.word("an instance path")?;
            if word == "*" {
                return Err(SdfError::new(
                    pos,
                    "the wildcard `*` cannot be part of a hierarchical path",
                ));
            }
            let components = self.path_components(&word, pos, "the instance path")?;
            self.check_instance_components(&components, pos, &word);
            return Ok(components);
        }
        Ok(Vec::new())
    }

    /// A port_spec: a port_instance or `(EDGE_IDENTIFIER port_instance)`.
    fn port_spec(&mut self, what: &str) -> Result<SdfPortSpec, SdfError> {
        if *self.peek()? != Tok::LParen {
            return Ok(SdfPortSpec {
                edge: None,
                port: self.port_instance(what)?,
            });
        }
        self.next()?;
        self.edge_spec_after_open(what)
    }

    fn edge_spec_after_open(&mut self, what: &str) -> Result<SdfPortSpec, SdfError> {
        let (word, pos) = self.word(format_args!("an edge identifier of {what}"))?;
        let Some(edge) = SdfEdge::from_word(&word) else {
            return Err(SdfError::new(
                pos,
                format!(
                    "`{word}` is not an edge identifier (posedge, negedge, 01, 10, 0z, z1, 1z, z0)"
                ),
            ));
        };
        let (word, word_pos) = self.word(format_args!("the port of {what}"))?;
        let port = self.port_from_word(&word, word_pos)?;
        self.close("after an edge specification")?;
        Ok(SdfPortSpec {
            edge: Some(edge),
            port,
        })
    }

    // ----- header --------------------------------------------------------

    fn read_header(&mut self) -> Result<(), SdfError> {
        self.open("starting the DELAYFILE")?;
        let (keyword, pos) = self.keyword("starting the file")?;
        if keyword != "DELAYFILE" {
            return Err(SdfError::new(
                pos,
                format!("expected `DELAYFILE`, found `{keyword}`"),
            ));
        }
        let open = self.open("of the SDFVERSION entry")?;
        let (keyword, pos) = self.keyword("of the header")?;
        if keyword != "SDFVERSION" {
            return Err(SdfError::new(
                pos,
                format!("the header must start with SDFVERSION, found `{keyword}`"),
            ));
        }
        let text = self.qstring("the SDFVERSION entry")?;
        let (version, warning) = detect_version(&text);
        if let Some(message) = warning {
            self.warn(open, message);
        }
        self.header.version = version;
        self.header.version_text = text;
        self.close("after the SDF version")?;
        let mut last: Option<usize> = None;
        loop {
            // `delay_file ::= ( DELAYFILE sdf_header cell+ )`.
            if *self.peek()? == Tok::RParen {
                let token = self.next()?;
                return Err(SdfError::new(
                    token.pos,
                    "the DELAYFILE has no CELL entry (it needs at least one)",
                ));
            }
            let open = self.open("of a header entry or CELL")?;
            let (keyword, pos) = self.keyword("of a header entry or CELL")?;
            if keyword == "CELL" {
                self.cell_open = Some(open);
                return Ok(());
            }
            let Some(order) = HEADER_ORDER.iter().position(|entry| *entry == keyword) else {
                return Err(SdfError::new(
                    pos,
                    format!("`{keyword}` is not a header entry"),
                ));
            };
            if let Some(last) = last {
                if order <= last {
                    return Err(SdfError::new(
                        pos,
                        format!(
                            "the header entry {keyword} {} {} (the header order is fixed)",
                            if order == last {
                                "repeats"
                            } else {
                                "comes after"
                            },
                            HEADER_ORDER[last]
                        ),
                    ));
                }
            }
            last = Some(order);
            self.header_entry(&keyword)?;
            self.close(format_args!("after the {keyword} entry"))?;
        }
    }

    fn header_entry(&mut self, keyword: &str) -> Result<(), SdfError> {
        let what = format!("the {keyword} entry");
        match keyword {
            "DESIGN" => self.header.design = Some(self.qstring(&what)?),
            "DATE" => self.header.date = Some(self.qstring(&what)?),
            "VENDOR" => self.header.vendor = Some(self.qstring(&what)?),
            "PROGRAM" => self.header.program = Some(self.qstring(&what)?),
            "VERSION" => self.header.program_version = Some(self.qstring(&what)?),
            "PROCESS" => self.header.process = Some(self.qstring(&what)?),
            "DIVIDER" => {
                let (word, word_pos) = self.word("the hierarchy divider `.` or `/`")?;
                self.header.divider = match word.as_str() {
                    "." => '.',
                    "/" => '/',
                    _ => {
                        return Err(SdfError::new(
                            word_pos,
                            format!("the hierarchy divider must be `.` or `/`, not `{word}`"),
                        ))
                    }
                };
            }
            "VOLTAGE" | "TEMPERATURE" => {
                let (word, word_pos) = self.word(format_args!("the value of {what}"))?;
                let value = self.value_word(&word, word_pos, Sign::Any, false)?;
                if keyword == "VOLTAGE" {
                    self.header.voltage = value;
                } else {
                    self.header.temperature = value;
                }
            }
            "TIMESCALE" => self.header.timescale = self.timescale()?,
            _ => unreachable!("listed in HEADER_ORDER"),
        }
        Ok(())
    }

    /// TSVALUE: 1, 10, 100, 1.0, 10.0 or 100.0, optionally a space, then
    /// us, ns or ps.
    fn timescale(&mut self) -> Result<SdfTimescale, SdfError> {
        let (word, word_pos) = self.word("the timescale value")?;
        let split = word
            .find(|character: char| character.is_ascii_alphabetic())
            .unwrap_or(word.len());
        let (number, unit) = word.split_at(split);
        let (unit, unit_pos) = if unit.is_empty() {
            self.word("the timescale unit us, ns or ps")?
        } else {
            let column = word_pos.col + u32::try_from(split).unwrap_or(0);
            (unit.to_owned(), SdfPos::new(word_pos.line, column))
        };
        let multiplier = match number {
            "1" | "1.0" => 1,
            "10" | "10.0" => 10,
            "100" | "100.0" => 100,
            _ => {
                return Err(SdfError::new(
                    word_pos,
                    format!("the timescale number must be 1, 10 or 100 (or 1.0, 10.0, 100.0), not `{number}`"),
                ))
            }
        };
        let unit = match unit.to_ascii_lowercase().as_str() {
            "us" => SdfTimeUnit::Us,
            "ns" => SdfTimeUnit::Ns,
            "ps" => SdfTimeUnit::Ps,
            _ => {
                return Err(SdfError::new(
                    unit_pos,
                    format!("the timescale unit must be us, ns or ps, not `{unit}`"),
                ))
            }
        };
        Ok(SdfTimescale { multiplier, unit })
    }

    // ----- cells ---------------------------------------------------------

    /// The next CELL entry, or `None` after the closing `)` of the
    /// DELAYFILE and the end of the input.
    pub fn next_cell(&mut self) -> Result<Option<SdfCell>, SdfError> {
        if self.finished {
            return Ok(None);
        }
        let open = match self.cell_open.take() {
            Some(open) => open,
            None => {
                let token = self.next()?;
                match token.tok {
                    Tok::RParen => {
                        self.lexer.expect_end()?;
                        self.finished = true;
                        return Ok(None);
                    }
                    Tok::LParen => {
                        let (keyword, pos) = self.keyword("of a CELL entry")?;
                        if keyword != "CELL" {
                            return Err(SdfError::new(
                                pos,
                                format!("expected `CELL`, found `{keyword}`"),
                            ));
                        }
                        token.pos
                    }
                    Tok::Eof => {
                        return Err(SdfError::new(
                            token.pos,
                            "the file ends before the DELAYFILE is closed",
                        ))
                    }
                    _ => {
                        return Err(Self::unexpected(
                            &token,
                            "`(CELL` or the `)` closing the DELAYFILE",
                        ))
                    }
                }
            }
        };
        self.cell(open).map(Some)
    }

    fn cell(&mut self, pos: SdfPos) -> Result<SdfCell, SdfError> {
        self.open("of the CELLTYPE entry")?;
        let (keyword, keyword_pos) = self.keyword("of the CELLTYPE entry")?;
        if keyword != "CELLTYPE" {
            return Err(SdfError::new(
                keyword_pos,
                format!("a CELL must start with CELLTYPE, found `{keyword}`"),
            ));
        }
        let cell_type = self.qstring("the CELLTYPE entry")?;
        self.close("after the cell type")?;
        self.open("of the INSTANCE entry")?;
        let (keyword, keyword_pos) = self.keyword("of the INSTANCE entry")?;
        if keyword != "INSTANCE" {
            return Err(SdfError::new(
                keyword_pos,
                format!("CELLTYPE must be followed by INSTANCE, found `{keyword}`"),
            ));
        }
        let instance = if let Tok::Word(word) = self.peek()? {
            if word == "*" {
                self.next()?;
                SdfCellInstance::Wildcard
            } else {
                SdfCellInstance::Path(self.optional_instance_path()?)
            }
        } else {
            SdfCellInstance::Path(Vec::new())
        };
        self.close("after the instance path")?;
        let mut cell = SdfCell {
            pos,
            cell_type,
            instance,
            specs: Vec::new(),
        };
        let mut correlation = false;
        let mut specs_started = false;
        loop {
            let token = self.next()?;
            match token.tok {
                Tok::RParen => return Ok(cell),
                Tok::LParen => {}
                _ => {
                    return Err(Self::unexpected(
                        &token,
                        "a timing specification or `)` closing the CELL",
                    ))
                }
            }
            let (keyword, keyword_pos) = self.keyword("of a timing specification")?;
            match keyword.as_str() {
                "DELAY" => {
                    specs_started = true;
                    let spec = self.delay_spec()?;
                    cell.specs.push(spec);
                }
                "TIMINGCHECK" => {
                    specs_started = true;
                    let spec = self.timing_check_spec()?;
                    cell.specs.push(spec);
                }
                "TIMINGENV" => {
                    if self.v21() {
                        self.other_version("TIMINGENV", keyword_pos);
                    }
                    specs_started = true;
                    self.timing_env_spec()?;
                    cell.specs.push(SdfTimingSpec::TimingEnv);
                }
                // `instance+` (2.1): the path continues.
                "INSTANCE" if !specs_started && !correlation => {
                    if !self.v21() {
                        self.other_version("a repeated INSTANCE entry", keyword_pos);
                    }
                    let SdfCellInstance::Path(path) = &mut cell.instance else {
                        return Err(SdfError::new(
                            keyword_pos,
                            "a wildcard INSTANCE cannot be followed by another INSTANCE",
                        ));
                    };
                    if let Tok::Word(word) = self.peek()? {
                        if word == "*" {
                            return Err(SdfError::new(
                                keyword_pos,
                                "the wildcard `*` cannot follow another INSTANCE",
                            ));
                        }
                    }
                    let mut more = self.optional_instance_path()?;
                    path.append(&mut more);
                    self.close("after the instance path")?;
                }
                // CORRELATION (2.1): documentation for simultaneous min/max
                // analysis, which a simulator ignores (OVI SDF 2.1 3-12).
                "CORRELATION" if !specs_started && !correlation => {
                    if !self.v21() {
                        self.other_version("CORRELATION", keyword_pos);
                    }
                    correlation = true;
                    self.qstring("the CORRELATION entry")?;
                    let mut numbers = 0;
                    while let Tok::Word(_) = self.peek()? {
                        let (word, word_pos) = self.word("a correlation factor")?;
                        self.number(&word, word_pos, Sign::NonNegative)?;
                        numbers += 1;
                    }
                    if numbers != 0 && numbers != 1 && numbers != 3 {
                        return Err(SdfError::new(
                            keyword_pos,
                            "a correlation factor is one number or three numbers",
                        ));
                    }
                    self.close("after the CORRELATION entry")?;
                }
                _ => {
                    return Err(Self::unknown_keyword_error(
                        &keyword,
                        keyword_pos,
                        "a timing specification (DELAY, TIMINGCHECK or TIMINGENV)",
                    ))
                }
            }
        }
    }

    fn unknown_keyword_error(keyword: &str, pos: SdfPos, expected: &str) -> SdfError {
        SdfError::new(pos, format!("expected {expected}, found `{keyword}`"))
    }

    // ----- DELAY ---------------------------------------------------------

    fn delay_spec(&mut self) -> Result<SdfTimingSpec, SdfError> {
        let mut deltypes = Vec::new();
        loop {
            let token = self.next()?;
            match token.tok {
                Tok::RParen if !deltypes.is_empty() => return Ok(SdfTimingSpec::Delay(deltypes)),
                Tok::LParen => {}
                _ => {
                    return Err(Self::unexpected(
                        &token,
                        if deltypes.is_empty() {
                            "`(` of a delay type in DELAY"
                        } else {
                            "a delay type or `)` closing DELAY"
                        },
                    ))
                }
            }
            let (keyword, pos) = self.keyword("of a delay type")?;
            let deltype = match keyword.as_str() {
                "ABSOLUTE" => SdfDelType::Absolute(self.del_defs("ABSOLUTE")?),
                "INCREMENT" => SdfDelType::Increment(self.del_defs("INCREMENT")?),
                "PATHPULSE" => self.path_pulse(pos, false)?,
                "PATHPULSEPERCENT" | "GLOBALPATHPULSE" => {
                    if self.v21() == (keyword == "PATHPULSEPERCENT") {
                        self.other_version(&keyword, pos);
                    }
                    self.path_pulse(pos, true)?
                }
                _ => {
                    return Err(Self::unknown_keyword_error(
                        &keyword,
                        pos,
                        "ABSOLUTE, INCREMENT, PATHPULSE or PATHPULSEPERCENT",
                    ))
                }
            };
            deltypes.push(deltype);
        }
    }

    fn path_pulse(&mut self, pos: SdfPos, percent: bool) -> Result<SdfDelType, SdfError> {
        let what = if percent {
            "a PATHPULSEPERCENT limit"
        } else {
            "a PATHPULSE limit"
        };
        let path = if let Tok::Word(_) = self.peek()? {
            let input = self.port_instance("the pulse path input")?;
            let output = self.port_instance("the pulse path output")?;
            Some(Box::new((input, output)))
        } else {
            None
        };
        let reject = self.value(Sign::NonNegative, what)?;
        let error = if *self.peek()? == Tok::LParen {
            self.value(Sign::NonNegative, what)?
        } else {
            // "If you specify only one value, both limits are set to that
            // value" (OVI SDF 3.0 3-13).
            reject
        };
        self.close("after the pulse limits")?;
        Ok(SdfDelType::PathPulse {
            pos,
            percent,
            path,
            reject,
            error,
        })
    }

    fn del_defs(&mut self, mode: &str) -> Result<Vec<SdfDelDef>, SdfError> {
        let mut defs = Vec::new();
        loop {
            let token = self.next()?;
            match token.tok {
                Tok::RParen if !defs.is_empty() => return Ok(defs),
                Tok::LParen => {}
                _ => {
                    return Err(Self::unexpected(
                        &token,
                        &if defs.is_empty() {
                            format!("`(` of a delay definition in {mode}")
                        } else {
                            format!("a delay definition or `)` closing {mode}")
                        },
                    ))
                }
            }
            let (keyword, pos) = self.keyword("of a delay definition")?;
            let def = match keyword.as_str() {
                "IOPATH" => self.io_path(pos, SdfPathCondition::None)?,
                "COND" => {
                    let label = self.optional_label()?;
                    let expr = ExprParser::new(&mut self.lexer, self.limits).expression()?;
                    let def = self.nested_io_path("COND")?;
                    let def = with_condition(
                        def,
                        SdfPathCondition::Cond(SdfCondition { label, expr }),
                        pos,
                    );
                    self.close("after the conditional IOPATH")?;
                    def
                }
                "CONDELSE" => {
                    if self.v21() {
                        self.other_version("CONDELSE", pos);
                    }
                    let def = self.nested_io_path("CONDELSE")?;
                    let def = with_condition(def, SdfPathCondition::CondElse, pos);
                    self.close("after the CONDELSE IOPATH")?;
                    def
                }
                "PORT" => {
                    let port = self.port_instance("PORT")?;
                    let delays = self.delval_list("PORT")?;
                    self.close("after the PORT delays")?;
                    SdfDelDef::Port { pos, port, delays }
                }
                "INTERCONNECT" => {
                    let source = self.port_instance("the INTERCONNECT source")?;
                    let load = self.port_instance("the INTERCONNECT load")?;
                    let delays = self.delval_list("INTERCONNECT")?;
                    self.close("after the INTERCONNECT delays")?;
                    SdfDelDef::Interconnect {
                        pos,
                        source,
                        load,
                        delays,
                    }
                }
                "NETDELAY" => {
                    if !self.v21() {
                        self.other_version("NETDELAY", pos);
                    }
                    let net = self.port_instance("NETDELAY")?;
                    let delays = self.delval_list("NETDELAY")?;
                    self.close("after the NETDELAY delays")?;
                    SdfDelDef::NetDelay { pos, net, delays }
                }
                "DEVICE" => {
                    let port = match self.peek()? {
                        Tok::Word(_) => Some(self.port_instance("DEVICE")?),
                        _ => None,
                    };
                    let delays = self.delval_list("DEVICE")?;
                    self.close("after the DEVICE delays")?;
                    SdfDelDef::Device { pos, port, delays }
                }
                _ => {
                    return Err(Self::unknown_keyword_error(
                        &keyword,
                        pos,
                        "IOPATH, COND, CONDELSE, PORT, INTERCONNECT or DEVICE",
                    ))
                }
            };
            defs.push(def);
        }
    }

    /// An optional QSTRING condition label (3.0).
    fn optional_label(&mut self) -> Result<Option<String>, SdfError> {
        if let Tok::QString(_) = self.peek()? {
            let token = self.next()?;
            if self.v21() {
                self.other_version("a condition label", token.pos);
            }
            let Tok::QString(text) = token.tok else {
                unreachable!()
            };
            return Ok(Some(text));
        }
        Ok(None)
    }

    /// The `(IOPATH ...)` inside COND or CONDELSE.
    fn nested_io_path(&mut self, owner: &str) -> Result<SdfDelDef, SdfError> {
        self.open(format_args!("of the IOPATH in {owner}"))?;
        let (keyword, pos) = self.keyword(format_args!("of the IOPATH in {owner}"))?;
        if keyword != "IOPATH" {
            return Err(SdfError::new(
                pos,
                format!("{owner} must contain an IOPATH, found `{keyword}`"),
            ));
        }
        self.io_path(pos, SdfPathCondition::None)
    }

    fn io_path(&mut self, pos: SdfPos, condition: SdfPathCondition) -> Result<SdfDelDef, SdfError> {
        let input = self.port_spec("the IOPATH input")?;
        let output = self.port_instance("the IOPATH output")?;
        let mut retain = Vec::new();
        let mut retain_positions = Vec::new();
        let mut first = Vec::new();
        let mut thirteenth = None;
        // `( RETAIN delval_list )*` then the delays; both start with `(`.
        while *self.peek()? == Tok::LParen {
            let open = self.next()?;
            if let Tok::Word(word) = self.peek()? {
                if keyword_is(word, "RETAIN") {
                    let (_, retain_pos) = self.word("RETAIN")?;
                    if self.v21() {
                        self.other_version("RETAIN", retain_pos);
                    }
                    if !first.is_empty() {
                        return Err(SdfError::new(
                            retain_pos,
                            "RETAIN entries must come before the IOPATH delays",
                        ));
                    }
                    retain_positions.push(open.pos);
                    retain.push(self.delval_list("RETAIN")?);
                    self.close("after the RETAIN delays")?;
                    continue;
                }
            }
            if first.len() == 12 {
                thirteenth.get_or_insert(open.pos);
            }
            first.push(self.delval_after_open()?);
        }
        self.too_many_values(thirteenth, "IOPATH");
        let delays = self.finish_delval_list(first, "IOPATH")?;
        self.close("after the IOPATH delays")?;
        self.check_retain_order(&retain, &retain_positions);
        Ok(SdfDelDef::IoPath {
            pos,
            condition,
            input,
            output,
            retain,
            delays,
        })
    }

    /// "The delays in delval_list for consecutive RETAIN statements must be
    /// strictly monotonically increasing" (OVI SDF 3.0 3-21): compared per
    /// transition and data set where both are given.
    fn check_retain_order(&mut self, retain: &[SdfDelvalList], positions: &[SdfPos]) {
        for (index, pair) in retain.windows(2).enumerate() {
            for (earlier, later) in pair[0].values.iter().zip(&pair[1].values) {
                let (Some(earlier), Some(later)) = (earlier.delay, later.delay) else {
                    continue;
                };
                for set in 0..3 {
                    if let (Some(a), Some(b)) = (earlier.member(set), later.member(set)) {
                        if b.cmp_exact(&a) != std::cmp::Ordering::Greater {
                            // RETAIN is not simulated (IEEE 1800-2009 Table
                            // 32-1), so the order only earns a warning.
                            self.warn(
                                positions[index + 1],
                                format!(
                                    "consecutive RETAIN delays must increase strictly ({a} then {b})"
                                ),
                            );
                            return;
                        }
                    }
                }
            }
        }
    }

    // ----- TIMINGCHECK ---------------------------------------------------

    fn timing_check_spec(&mut self) -> Result<SdfTimingSpec, SdfError> {
        let mut checks = Vec::new();
        let mut entries = 0usize;
        loop {
            let token = self.next()?;
            match token.tok {
                Tok::RParen if entries > 0 => return Ok(SdfTimingSpec::TimingCheck(checks)),
                Tok::LParen => {}
                _ => {
                    return Err(Self::unexpected(
                        &token,
                        if entries == 0 {
                            "`(` of a timing check in TIMINGCHECK"
                        } else {
                            "a timing check or `)` closing TIMINGCHECK"
                        },
                    ))
                }
            }
            let (keyword, pos) = self.keyword("of a timing check")?;
            entries += 1;
            let kind =
                match keyword.as_str() {
                    "SETUP" => SdfCheckKind::Setup,
                    "HOLD" => SdfCheckKind::Hold,
                    "SETUPHOLD" => SdfCheckKind::SetupHold,
                    "RECOVERY" => SdfCheckKind::Recovery,
                    "REMOVAL" | "RECREM" => {
                        if self.v21() {
                            self.other_version(&keyword, pos);
                        }
                        if keyword == "REMOVAL" {
                            SdfCheckKind::Removal
                        } else {
                            SdfCheckKind::RecRem
                        }
                    }
                    "SKEW" => SdfCheckKind::Skew,
                    "WIDTH" => SdfCheckKind::Width,
                    "PERIOD" => SdfCheckKind::Period,
                    "NOCHANGE" => SdfCheckKind::NoChange,
                    // 2.1 kept constraints among the timing checks.
                    "PATHCONSTRAINT" | "SUM" | "DIFF" | "SKEWCONSTRAINT" => {
                        if !self.v21() {
                            self.other_version(&format!("{keyword} in TIMINGCHECK"), pos);
                        }
                        self.constraint(&keyword, pos)?;
                        continue;
                    }
                    _ => return Err(Self::unknown_keyword_error(
                        &keyword,
                        pos,
                        "a timing check (SETUP, HOLD, SETUPHOLD, RECOVERY, REMOVAL, RECREM, SKEW, \
                         WIDTH, PERIOD or NOCHANGE)",
                    )),
                };
            if let Some(check) = self.timing_check(kind, pos)? {
                checks.push(check);
            }
        }
    }

    /// A timing check entry, `None` when it is read but not annotated.
    fn timing_check(
        &mut self,
        kind: SdfCheckKind,
        pos: SdfPos,
    ) -> Result<Option<SdfTimingCheck>, SdfError> {
        let name = kind.keyword();
        let mut ports = Vec::with_capacity(2);
        for index in 0..kind.port_count() {
            ports.push(self.port_tchk(&format!("{name} port {}", index + 1))?);
        }
        // value (NUMBER) where the grammar says value, else rvalue. 2.1 used
        // rvalue for SETUP, HOLD and RECOVERY.
        let sign = match kind {
            SdfCheckKind::Setup | SdfCheckKind::Hold | SdfCheckKind::Recovery if self.v21() => {
                Sign::Any
            }
            SdfCheckKind::Setup
            | SdfCheckKind::Hold
            | SdfCheckKind::Recovery
            | SdfCheckKind::Removal
            | SdfCheckKind::Width
            | SdfCheckKind::Period => Sign::NonNegative,
            _ => Sign::Any,
        };
        let mut values = Vec::with_capacity(2);
        for _ in 0..kind.value_count() {
            values.push(self.value(sign, &format!("the {name} limit"))?);
        }
        let mut scond = None;
        let mut ccond = None;
        if matches!(kind, SdfCheckKind::SetupHold | SdfCheckKind::RecRem) {
            while *self.peek()? == Tok::LParen {
                self.next()?;
                let (keyword, keyword_pos) = self.keyword("of SCOND or CCOND")?;
                if self.v21() && matches!(keyword.as_str(), "SCOND" | "CCOND") {
                    self.other_version(&keyword, keyword_pos);
                }
                let slot = match keyword.as_str() {
                    "SCOND" if scond.is_none() && ccond.is_none() => &mut scond,
                    "CCOND" if ccond.is_none() => &mut ccond,
                    "SCOND" | "CCOND" => {
                        return Err(SdfError::new(
                            keyword_pos,
                            format!("{keyword} is out of order or repeated (SCOND comes before CCOND, once each)"),
                        ))
                    }
                    _ => return Err(Self::unknown_keyword_error(&keyword, keyword_pos, "SCOND, CCOND or `)`")),
                };
                let label = self.optional_label()?;
                let expr =
                    ExprParser::new(&mut self.lexer, self.limits).timing_check_condition()?;
                *slot = Some(SdfCondition { label, expr });
                self.close(format_args!("after the {keyword} condition"))?;
            }
        }
        self.close(format_args!("after the {name} limits"))?;
        // "An edge specification must be included for the control port"
        // (OVI SDF 3.0 3-35).
        if kind == SdfCheckKind::NoChange && ports[0].spec.edge.is_none() {
            self.warn(
                ports[0].spec.port.pos,
                "the NOCHANGE control port needs an edge specification; the entry is not annotated",
            );
            return Ok(None);
        }
        self.check_limit_sum(kind, &values, pos);
        Ok(Some(SdfTimingCheck {
            pos,
            kind,
            ports,
            values,
            scond,
            ccond,
        }))
    }

    /// SETUPHOLD and RECREM: "Either can be negative, however their sum
    /// must be greater than zero" (OVI SDF 3.0 3-30, 3-33), checked per
    /// data set where both limits are given.
    fn check_limit_sum(&mut self, kind: SdfCheckKind, values: &[Option<SdfValue>], pos: SdfPos) {
        if !matches!(kind, SdfCheckKind::SetupHold | SdfCheckKind::RecRem) {
            return;
        }
        let (Some(first), Some(second)) = (values[0], values[1]) else {
            return;
        };
        for set in 0..3 {
            if let (Some(a), Some(b)) = (first.member(set), second.member(set)) {
                if !a.is_negative() && !b.is_negative() {
                    continue;
                }
                // a + b > 0 exactly when a > -b.
                if a.cmp_exact(&b.negated()) != std::cmp::Ordering::Greater {
                    // Applying the limits settles them as declared ones are
                    // (IEEE 1800-2009 31.9.1), so this only earns a warning.
                    self.warn(
                        pos,
                        format!(
                            "the {} limits {a} and {b} have a sum that is not greater than zero",
                            kind.keyword()
                        ),
                    );
                    return;
                }
            }
        }
    }

    /// A port_tchk: a port_spec, or `(COND label? condition port_spec)`.
    fn port_tchk(&mut self, what: &str) -> Result<SdfPortTchk, SdfError> {
        if *self.peek()? != Tok::LParen {
            return Ok(SdfPortTchk {
                condition: None,
                spec: self.port_spec(what)?,
            });
        }
        self.next()?;
        if let Tok::Word(word) = self.peek()? {
            if keyword_is(word, "COND") {
                self.next()?;
                let label = self.optional_label()?;
                let expr =
                    ExprParser::new(&mut self.lexer, self.limits).timing_check_condition()?;
                let spec = self.port_spec(what)?;
                self.close("after the conditional timing check port")?;
                return Ok(SdfPortTchk {
                    condition: Some(SdfCondition { label, expr }),
                    spec,
                });
            }
        }
        Ok(SdfPortTchk {
            condition: None,
            spec: self.edge_spec_after_open(what)?,
        })
    }

    // ----- TIMINGENV and constraints ------------------------------------

    fn timing_env_spec(&mut self) -> Result<(), SdfError> {
        let mut entries = 0usize;
        loop {
            let token = self.next()?;
            match token.tok {
                Tok::RParen if entries > 0 => return Ok(()),
                Tok::LParen => {}
                _ => {
                    return Err(Self::unexpected(
                        &token,
                        if entries == 0 {
                            "`(` of an entry in TIMINGENV"
                        } else {
                            "a TIMINGENV entry or `)` closing TIMINGENV"
                        },
                    ))
                }
            }
            entries += 1;
            let (keyword, pos) = self.keyword("of a TIMINGENV entry")?;
            match keyword.as_str() {
                "PATHCONSTRAINT" | "PERIODCONSTRAINT" | "SUM" | "DIFF" | "SKEWCONSTRAINT" => {
                    self.constraint(&keyword, pos)?
                }
                "ARRIVAL" | "DEPARTURE" => {
                    if *self.peek()? == Tok::LParen {
                        self.next()?;
                        self.edge_spec_after_open(&keyword)?;
                    }
                    self.port_instance(&keyword)?;
                    for _ in 0..4 {
                        self.value(Sign::Any, &format!("a {keyword} value"))?;
                    }
                    self.close(format_args!("after the {keyword} entry"))?;
                }
                "SLACK" => {
                    self.port_instance("SLACK")?;
                    for _ in 0..4 {
                        self.value(Sign::Any, "a SLACK value")?;
                    }
                    if let Tok::Word(_) = self.peek()? {
                        let (word, word_pos) = self.word("the SLACK clock period")?;
                        self.number(&word, word_pos, Sign::NonNegative)?;
                    }
                    self.close("after the SLACK entry")?;
                }
                "WAVEFORM" => self.waveform()?,
                _ => {
                    return Err(SdfError::new(
                        pos,
                        format!(
                            "expected a TIMINGENV entry (PATHCONSTRAINT, PERIODCONSTRAINT, SUM, \
                             DIFF, SKEWCONSTRAINT, ARRIVAL, DEPARTURE, SLACK or WAVEFORM), found \
                             `{keyword}`"
                        ),
                    ))
                }
            }
        }
    }

    /// A constraint (cns_def), read and dropped.
    fn constraint(&mut self, keyword: &str, pos: SdfPos) -> Result<(), SdfError> {
        match keyword {
            "PATHCONSTRAINT" => {
                if !self.v21() && *self.peek()? == Tok::LParen {
                    self.next()?;
                    let (name, name_pos) = self.keyword("of the PATHCONSTRAINT name")?;
                    if name != "NAME" {
                        return Err(SdfError::new(
                            name_pos,
                            format!("expected `NAME`, found `{name}`"),
                        ));
                    }
                    if let Tok::QString(_) = self.peek()? {
                        self.next()?;
                    }
                    self.close("after the NAME entry")?;
                }
                let mut ports = 0;
                while let Tok::Word(_) = self.peek()? {
                    self.port_instance("PATHCONSTRAINT")?;
                    ports += 1;
                }
                if ports < 2 {
                    return Err(SdfError::new(
                        pos,
                        "PATHCONSTRAINT needs at least two ports",
                    ));
                }
                self.value(Sign::Any, "a PATHCONSTRAINT value")?;
                self.value(Sign::Any, "a PATHCONSTRAINT value")?;
            }
            "PERIODCONSTRAINT" => {
                self.port_instance("PERIODCONSTRAINT")?;
                self.value(Sign::NonNegative, "a PERIODCONSTRAINT value")?;
                if *self.peek()? == Tok::LParen {
                    self.next()?;
                    let (name, name_pos) = self.keyword("of the EXCEPTION entry")?;
                    if name != "EXCEPTION" {
                        return Err(SdfError::new(
                            name_pos,
                            format!("expected `EXCEPTION`, found `{name}`"),
                        ));
                    }
                    let mut instances = 0;
                    while *self.peek()? == Tok::LParen {
                        self.next()?;
                        let (word, word_pos) = self.keyword("of an EXCEPTION instance")?;
                        if word != "INSTANCE" {
                            return Err(SdfError::new(
                                word_pos,
                                format!("expected `INSTANCE`, found `{word}`"),
                            ));
                        }
                        if let Tok::Word(word) = self.peek()? {
                            if word == "*" {
                                self.next()?;
                            } else {
                                self.optional_instance_path()?;
                            }
                        }
                        self.close("after the EXCEPTION instance")?;
                        instances += 1;
                    }
                    if instances == 0 {
                        return Err(SdfError::new(name_pos, "EXCEPTION needs an instance"));
                    }
                    self.close("after the EXCEPTION entry")?;
                }
            }
            "SUM" | "DIFF" => {
                // constraint_path+ then the value(s); a constraint path is
                // `(port port)`, a value `(number)` or `()`.
                let mut paths = 0usize;
                let mut valued = false;
                while *self.peek()? == Tok::LParen {
                    let open = self.next()?;
                    let is_path = match self.peek()? {
                        Tok::Word(word) => !word.starts_with(|c: char| {
                            c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | ':')
                        }),
                        _ => false,
                    };
                    if is_path {
                        self.port_instance(keyword)?;
                        self.port_instance(keyword)?;
                        self.close("after a constraint path")?;
                        paths += 1;
                        continue;
                    }
                    let sign = if keyword == "DIFF" {
                        Sign::NonNegative
                    } else {
                        Sign::Any
                    };
                    self.value_after_open(open.pos, sign, &format!("a {keyword} value"))?;
                    if *self.peek()? == Tok::LParen {
                        self.value(sign, &format!("a {keyword} value"))?;
                    }
                    valued = true;
                    break;
                }
                let paths_ok = if keyword == "SUM" {
                    paths >= 2
                } else {
                    paths == 2
                };
                if !paths_ok || !valued {
                    return Err(SdfError::new(
                        pos,
                        format!(
                            "{keyword} needs {} constraint paths `(port port)` and a value",
                            if keyword == "SUM" {
                                "two or more"
                            } else {
                                "two"
                            }
                        ),
                    ));
                }
            }
            "SKEWCONSTRAINT" => {
                self.port_spec("SKEWCONSTRAINT")?;
                self.value(Sign::NonNegative, "a SKEWCONSTRAINT value")?;
            }
            _ => unreachable!("callers pass constraint keywords"),
        }
        self.close(format_args!("after the {keyword} entry"))
    }

    /// `(WAVEFORM port NUMBER edge_list)`: alternating posedge/negedge
    /// pairs.
    fn waveform(&mut self) -> Result<(), SdfError> {
        self.port_instance("WAVEFORM")?;
        let (word, word_pos) = self.word("the WAVEFORM period")?;
        self.number(&word, word_pos, Sign::NonNegative)?;
        let mut edges: Vec<SdfEdge> = Vec::new();
        while *self.peek()? == Tok::LParen {
            self.next()?;
            let (keyword, pos) = self.word("posedge or negedge")?;
            let edge = match SdfEdge::from_word(&keyword) {
                Some(edge @ (SdfEdge::Posedge | SdfEdge::Negedge)) => edge,
                _ => {
                    return Err(SdfError::new(
                        pos,
                        format!("a WAVEFORM edge is posedge or negedge, not `{keyword}`"),
                    ))
                }
            };
            if edges.last() == Some(&edge) {
                return Err(SdfError::new(
                    pos,
                    "WAVEFORM edges must alternate between posedge and negedge",
                ));
            }
            edges.push(edge);
            let (word, word_pos) = self.word("a WAVEFORM edge time")?;
            self.number(&word, word_pos, Sign::Any)?;
            if let Tok::Word(_) = self.peek()? {
                let (word, word_pos) = self.word("a WAVEFORM edge time")?;
                self.number(&word, word_pos, Sign::Any)?;
            }
            self.close("after a WAVEFORM edge")?;
        }
        if edges.is_empty() || !edges.len().is_multiple_of(2) {
            return Err(SdfError::new(
                word_pos,
                "a WAVEFORM needs pairs of posedge and negedge entries",
            ));
        }
        self.close("after the WAVEFORM entry")
    }
}

/// Put a condition on a parsed IOPATH and give it the position of its
/// COND or CONDELSE.
fn with_condition(def: SdfDelDef, condition: SdfPathCondition, pos: SdfPos) -> SdfDelDef {
    match def {
        SdfDelDef::IoPath {
            input,
            output,
            retain,
            delays,
            ..
        } => SdfDelDef::IoPath {
            pos,
            condition,
            input,
            output,
            retain,
            delays,
        },
        other => other,
    }
}

/// `part` with every unescaped `character` escaped.
fn escape_unescaped(part: &str, character: u8) -> String {
    let mut out = String::with_capacity(part.len() + 4);
    let mut bytes = part.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'\\' {
            out.push('\\');
            if let Some(next) = bytes.next() {
                out.push(next as char);
            }
        } else {
            if byte == character {
                out.push('\\');
            }
            out.push(byte as char);
        }
    }
    out
}

/// The grammar for an SDFVERSION string: the first of the known version
/// numbers it contains (OVI SDF 3.0 3-3). Versions without a published
/// grammar here are read with the nearest one, with a warning: 1.0 and 2.0
/// with 2.1 (their successor), 4.0 and a string without a version number
/// with 3.0.
fn detect_version(text: &str) -> (SdfVersion, Option<String>) {
    const KNOWN: &[&str] = &["1.0", "2.0", "2.1", "3.0", "4.0"];
    let found = KNOWN
        .iter()
        .filter_map(|version| text.find(version).map(|at| (at, *version)))
        .min();
    let fallback = |version: SdfVersion, reason: String| {
        (
            version,
            Some(format!(
                "{reason}; the file is read with the OVI SDF {version} grammar"
            )),
        )
    };
    match found {
        Some((_, "2.1")) => (SdfVersion::V2_1, None),
        Some((_, "3.0")) => (SdfVersion::V3_0, None),
        Some((_, version @ ("1.0" | "2.0"))) => fallback(
            SdfVersion::V2_1,
            format!("SDF version {version} has no grammar in llg"),
        ),
        Some((_, version)) => fallback(
            SdfVersion::V3_0,
            format!("SDF version {version} has no grammar in llg"),
        ),
        None => fallback(
            SdfVersion::V3_0,
            format!(
                "the SDFVERSION string \"{text}\" names no SDF version (1.0, 2.0, 2.1, 3.0 or 4.0)"
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> SdfFile {
        parse(text.as_bytes(), SdfLimits::default()).expect("parses")
    }

    fn error(text: &str) -> String {
        parse(text.as_bytes(), SdfLimits::default())
            .expect_err("is rejected")
            .to_string()
    }

    /// A file of `version` with one CELL whose body is `body`.
    fn cell(version: &str, body: &str) -> String {
        format!(
            "(DELAYFILE (SDFVERSION \"{version}\") (CELL (CELLTYPE \"c\") (INSTANCE i)\n{body}))"
        )
    }

    fn num(text: &str) -> SdfNumber {
        SdfNumber::parse(text).expect("a number")
    }

    #[test]
    fn header_entries_are_read_in_order() {
        let parsed = file(
            "(DELAYFILE (sdfversion \"OVI 3.0\") (DESIGN \"d\") (DATE \"x\") (VENDOR \"v\") \
             (PROGRAM \"p\") (VERSION \"1\") (DIVIDER /) (VOLTAGE 1.0) (PROCESS \"best\") \
             (TEMPERATURE ::85) (TIMESCALE 10.0 us) (CELL (CELLTYPE \"c\") (INSTANCE)))",
        );
        let header = parsed.header;
        assert_eq!(header.version, SdfVersion::V3_0);
        assert_eq!(header.version_text, "OVI 3.0");
        assert_eq!(header.design.as_deref(), Some("d"));
        assert_eq!(header.program_version.as_deref(), Some("1"));
        assert_eq!(header.divider, '/');
        assert_eq!(header.voltage, Some(SdfValue::Single(num("1"))));
        assert_eq!(
            header.temperature,
            Some(SdfValue::Triple([None, None, Some(num("85"))]))
        );
        assert_eq!(
            header.timescale,
            SdfTimescale {
                multiplier: 10,
                unit: SdfTimeUnit::Us
            }
        );
        assert_eq!(parsed.cells[0].instance, SdfCellInstance::Path(Vec::new()));
        // Defaults: `.` and 1ns.
        let header =
            file("(DELAYFILE (SDFVERSION \"2.1\") (CELL (CELLTYPE \"c\") (INSTANCE)))").header;
        assert_eq!(header.version, SdfVersion::V2_1);
        assert_eq!(header.divider, '.');
        assert_eq!(header.timescale.to_string(), "1ns");
    }

    #[test]
    fn the_first_version_number_selects_the_grammar() {
        let read = |text: &str| {
            let parsed = file(&format!(
                "(DELAYFILE (SDFVERSION \"{text}\") (CELL (CELLTYPE \"c\") (INSTANCE)))"
            ));
            (
                parsed.header.version,
                parsed
                    .warnings
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(read("3.0 (2.1 compatible)"), (SdfVersion::V3_0, vec![]));
        assert_eq!(read("v2.1, not 3.0"), (SdfVersion::V2_1, vec![]));
        // Without a grammar of their own, versions read with the nearest one.
        assert_eq!(
            read("1.0"),
            (
                SdfVersion::V2_1,
                vec!["1:12: SDF version 1.0 has no grammar in llg; the file is read with the OVI SDF 2.1 grammar".to_owned()]
            )
        );
        assert_eq!(read("OVI 2.0").0, SdfVersion::V2_1);
        assert_eq!(
            read("IEEE 1497 4.0"),
            (
                SdfVersion::V3_0,
                vec!["1:12: SDF version 4.0 has no grammar in llg; the file is read with the OVI SDF 3.0 grammar".to_owned()]
            )
        );
        assert_eq!(
            read(""),
            (
                SdfVersion::V3_0,
                vec!["1:12: the SDFVERSION string \"\" names no SDF version (1.0, 2.0, 2.1, 3.0 or 4.0); the file is read with the OVI SDF 3.0 grammar".to_owned()]
            )
        );
        assert_eq!(
            error("(DELAYFILE (SDFVERSION \"3.0\") (DESIGN \"d\"))"),
            "1:43: the DELAYFILE has no CELL entry (it needs at least one)"
        );
    }

    #[test]
    fn delay_entries_keep_their_values() {
        let parsed = file(&cell(
            "3.0",
            "(DELAY (ABSOLUTE (IOPATH (posedge a) y[3] () (1:2:3) ((4) () (5)))) \
             (INCREMENT (PORT u.p[7:0] (-1)) (INTERCONNECT u.o v.i (1)) (DEVICE (2)) (DEVICE o (3))) \
             (PATHPULSE a y (1)) (PATHPULSEPERCENT (10) (20)))",
        ));
        let [SdfTimingSpec::Delay(deltypes)] = parsed.cells[0].specs.as_slice() else {
            panic!("one DELAY");
        };
        let SdfDelType::Absolute(defs) = &deltypes[0] else {
            panic!("ABSOLUTE");
        };
        let SdfDelDef::IoPath {
            input,
            output,
            delays,
            ..
        } = &defs[0]
        else {
            panic!("IOPATH");
        };
        assert_eq!(input.to_string(), "(posedge a)");
        assert_eq!(output.to_string(), "y[3]");
        assert_eq!(delays.to_string(), "() (1:2:3) ((4) () (5))");
        let SdfDelType::Increment(defs) = &deltypes[1] else {
            panic!("INCREMENT");
        };
        assert!(matches!(&defs[0], SdfDelDef::Port { port, .. } if port.to_string() == "u.p[7:0]"));
        assert!(matches!(&defs[2], SdfDelDef::Device { port: None, .. }));
        assert!(matches!(&defs[3], SdfDelDef::Device { port: Some(_), .. }));
        // One PATHPULSE value sets both limits.
        assert!(matches!(
            &deltypes[2],
            SdfDelType::PathPulse { percent: false, path: Some(_), reject, error, .. } if reject == error
        ));
        assert!(matches!(
            &deltypes[3],
            SdfDelType::PathPulse {
                percent: true,
                path: None,
                ..
            }
        ));
    }

    #[test]
    fn conditions_and_labels() {
        let parsed = file(&cell(
            "3.0",
            "(DELAY (ABSOLUTE (COND \"l\" a&&!b (IOPATH x y (1))) (CONDELSE (IOPATH x y (2))))) \
             (TIMINGCHECK (SETUPHOLD (COND en d) (posedge clk) (1) (1) (SCOND \"s\" en) (CCOND ~en)))",
        ));
        let SdfTimingSpec::Delay(deltypes) = &parsed.cells[0].specs[0] else {
            panic!("DELAY");
        };
        let SdfDelType::Absolute(defs) = &deltypes[0] else {
            panic!("ABSOLUTE");
        };
        let SdfDelDef::IoPath {
            condition: SdfPathCondition::Cond(condition),
            ..
        } = &defs[0]
        else {
            panic!("COND");
        };
        assert_eq!(condition.label.as_deref(), Some("l"));
        assert_eq!(condition.expr.to_string(), "(a && !(b))");
        assert!(matches!(
            &defs[1],
            SdfDelDef::IoPath {
                condition: SdfPathCondition::CondElse,
                ..
            }
        ));
        let SdfTimingSpec::TimingCheck(checks) = &parsed.cells[0].specs[1] else {
            panic!("TIMINGCHECK");
        };
        let check = &checks[0];
        assert_eq!(check.kind, SdfCheckKind::SetupHold);
        assert_eq!(
            check.ports[0]
                .condition
                .as_ref()
                .map(|c| c.expr.to_string()),
            Some("en".to_owned())
        );
        assert_eq!(
            check.scond.as_ref().and_then(|c| c.label.as_deref()),
            Some("s")
        );
        assert_eq!(
            check.ccond.as_ref().map(|c| c.expr.to_string()),
            Some("~(en)".to_owned())
        );
    }

    #[test]
    fn sdf_2_1_forms() {
        let parsed = file(
            "(DELAYFILE (SDFVERSION \"2.1\") (DIVIDER /) (CELL (CELLTYPE \"c\") (INSTANCE a/b) \
             (INSTANCE c) (CORRELATION \"k\" 1 2 3) (DELAY (ABSOLUTE (NETDELAY (INSTANCE d) n (1)) \
             (IOPATH x y (1))) (GLOBALPATHPULSE (30) (40))) (TIMINGCHECK (SETUP d clk (-1)) \
             (SUM (a b) (c d) (1)) (DIFF (a b) (c d) (1)) (SKEWCONSTRAINT (posedge c) (1)) \
             (PATHCONSTRAINT a b c (1) (2)))))",
        );
        let cell = &parsed.cells[0];
        assert_eq!(
            cell.instance,
            SdfCellInstance::Path(
                ["a", "b", "c"]
                    .iter()
                    .map(|name| SdfIdent {
                        name: (*name).to_owned(),
                        bits: None
                    })
                    .collect()
            )
        );
        let SdfTimingSpec::Delay(deltypes) = &cell.specs[0] else {
            panic!("DELAY");
        };
        let SdfDelType::Absolute(defs) = &deltypes[0] else {
            panic!("ABSOLUTE");
        };
        assert!(matches!(&defs[0], SdfDelDef::NetDelay { net, .. } if net.to_string() == "d.n"));
        assert!(matches!(
            &deltypes[1],
            SdfDelType::PathPulse { percent: true, .. }
        ));
        let SdfTimingSpec::TimingCheck(checks) = &cell.specs[1] else {
            panic!("TIMINGCHECK");
        };
        // The constraints are read and dropped.
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].values[0], Some(SdfValue::Single(num("-1"))));
    }

    #[test]
    fn constructs_of_the_other_version_warn() {
        for (version, body, message) in [
            ("3.0", "(INSTANCE x)", "2:2: a repeated INSTANCE entry belongs to SDF 2.1, not SDF 3.0; it is read as SDF 2.1 defines it"),
            ("3.0", "(CORRELATION \"k\")", "2:2: CORRELATION belongs to SDF 2.1, not SDF 3.0; it is read as SDF 2.1 defines it"),
            ("3.0", "(DELAY (GLOBALPATHPULSE (1)))", "2:9: GLOBALPATHPULSE belongs to SDF 2.1, not SDF 3.0; it is read as SDF 2.1 defines it"),
            ("3.0", "(DELAY (ABSOLUTE (NETDELAY n (1))))", "2:19: NETDELAY belongs to SDF 2.1, not SDF 3.0; it is read as SDF 2.1 defines it"),
            ("3.0", "(DELAY (ABSOLUTE (PORT (INSTANCE u) p (1))))", "2:24: a port path `(INSTANCE path) port` belongs to SDF 2.1, not SDF 3.0; it is read as SDF 2.1 defines it"),
            ("3.0", "(TIMINGCHECK (SUM (a b) (c d) (1)))", "2:15: SUM in TIMINGCHECK belongs to SDF 2.1, not SDF 3.0; it is read as SDF 2.1 defines it"),
            ("2.1", "(TIMINGENV (SLACK p (1) (1) (1) (1)))", "2:2: TIMINGENV belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(DELAY (PATHPULSEPERCENT (1)))", "2:9: PATHPULSEPERCENT belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(DELAY (ABSOLUTE (CONDELSE (IOPATH a y (1)))))", "2:19: CONDELSE belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(TIMINGCHECK (REMOVAL r c (1)))", "2:15: REMOVAL belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(TIMINGCHECK (RECREM r c (1) (1)))", "2:15: RECREM belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(TIMINGCHECK (SETUPHOLD d c (1) (1) (SCOND e)))", "2:38: SCOND belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(TIMINGCHECK (WIDTH c ()))", "2:23: an empty value for the WIDTH limit belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(DELAY (ABSOLUTE (IOPATH a y ((1) (2)))))", "2:31: a delay value with pulse limits `((d) (r) (e))` belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(DELAY (ABSOLUTE (IOPATH a y (RETAIN (1)) (2))))", "2:31: RETAIN belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("2.1", "(DELAY (ABSOLUTE (COND \"l\" a (IOPATH a y (1)))))", "2:24: a condition label belongs to SDF 3.0, not SDF 2.1; it is read as SDF 3.0 defines it"),
            ("3.0", "(TIMINGCHECK (SETUP d c (-1)))", "2:26: the value `-1` must not be negative; 0 is used"),
        ] {
            let parsed = file(&cell(version, body));
            let warnings = parsed.warnings.iter().map(ToString::to_string).collect::<Vec<_>>();
            assert_eq!(warnings, [message], "{version}: {body}");
        }
        // A negative value counts as 0.
        let parsed = file(&cell("3.0", "(TIMINGCHECK (WIDTH c (-1:2:-3)))"));
        let SdfTimingSpec::TimingCheck(checks) = &parsed.cells[0].specs[0] else {
            panic!("TIMINGCHECK");
        };
        assert_eq!(
            checks[0].values[0].map(|value| value.to_string()),
            Some("0:2:0".to_owned())
        );
    }

    #[test]
    fn reader_streams_cells() {
        let text = format!(
            "(DELAYFILE (SDFVERSION \"3.0\") {})",
            "(CELL (CELLTYPE \"c\") (INSTANCE *) (DELAY (ABSOLUTE (DEVICE (1))))) ".repeat(3)
        );
        let mut reader = SdfReader::new(text.as_bytes(), SdfLimits::default()).expect("header");
        for _ in 0..3 {
            let cell = reader.next_cell().expect("a cell").expect("not the end");
            assert_eq!(cell.instance, SdfCellInstance::Wildcard);
        }
        assert_eq!(reader.next_cell().expect("the end"), None);
        assert_eq!(reader.next_cell().expect("still the end"), None);
    }

    /// Buffer boundaries anywhere (inside words, comments, strings and
    /// escapes) give the same result as one buffer.
    #[test]
    fn small_read_buffers_give_the_same_file() {
        let text = cell(
            "3.0",
            "// comment\n(DELAY (ABSOLUTE (COND \"l\" a\\.b==1'b1 (IOPATH (posedge u\\/v.a[3]) y \
             (1.5:2.25:3) ((4) (5)))) /* block */ (INTERCONNECT p.q[1:0] r.s ()))) \
             (TIMINGCHECK (SETUPHOLD (COND ~e d) (negedge clk) (1) (-0.5)))",
        );
        let whole = file(&text);
        for capacity in [1, 2, 3, 7, 64] {
            let reader = std::io::BufReader::with_capacity(capacity, text.as_bytes());
            assert_eq!(
                parse(reader, SdfLimits::default()).expect("parses"),
                whole,
                "{capacity}"
            );
        }
    }

    #[test]
    fn mixed_value_forms_warn_once() {
        let parsed = file(&cell(
            "3.0",
            "(DELAY (ABSOLUTE (IOPATH a y (1:2:3)) (IOPATH b y (1)) (IOPATH c y (2))))",
        ));
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(parsed.warnings[0].pos, SdfPos::new(2, 52));
    }

    #[test]
    fn limits_bound_paths_and_conditions() {
        let limits = SdfLimits {
            max_path_components: 2,
            ..SdfLimits::default()
        };
        let text = cell("3.0", "(DELAY (ABSOLUTE (PORT a.b.c (1))))");
        assert_eq!(
            parse(text.as_bytes(), limits)
                .expect_err("too deep")
                .to_string(),
            "2:24: the port has more than 2 hierarchy levels"
        );
        let limits = SdfLimits {
            max_condition_depth: 2,
            ..SdfLimits::default()
        };
        let text = cell("3.0", "(DELAY (ABSOLUTE (COND (((a))) (IOPATH a y (1)))))");
        assert_eq!(
            parse(text.as_bytes(), limits)
                .expect_err("too deep")
                .to_string(),
            "2:26: the condition nests deeper than 2 levels"
        );
    }
}
