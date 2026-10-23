//! Streaming SDF lexer (OVI SDF 3.0 "SDF File Characters").
//!
//! The lexer reads bytes from a [`BufRead`] without holding the file: memory
//! is bounded by the longest token, which [`SdfLimits`] caps. It has two
//! modes chosen by the parser for each token:
//!
//! * [`Mode::Structure`] splits the input at white space, parentheses and
//!   double quotes. Everything else (keywords, numbers, triples, paths with
//!   escapes and bit specs) is one [`Tok::Word`] that the parser interprets
//!   in context, because `.` and `/` are hierarchy dividers in a path but
//!   part of a number elsewhere.
//! * [`Mode::Expression`] splits conditions into identifiers, constants and
//!   operators (`a&&b` is three tokens).
//!
//! A peeked token can be returned to the input ([`Lexer::unread`]) so the
//! parser can switch modes at a token boundary. Comments are C (`/* */`, not
//! nested) and C++ (`//` to the end of the line) style; the specification
//! leaves the legal places of C comments open, so both are accepted between
//! any two tokens.

use super::{SdfError, SdfLimits, SdfPos};
use std::io::BufRead;

/// How the next token is split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Structure,
    Expression,
}

/// Operators of SDF conditions ("Operators for Expressions"), longest first
/// so that lexing is greedy.
const OPERATORS: &[&str] = &[
    "==", "!=", "&&", "||", "<=", ">=", "<<", ">>", "~&", "~|", "~^", "^~", "+", "-", "!", "~",
    "&", "|", "^", "*", "/", "%", "<", ">",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Tok {
    LParen,
    RParen,
    /// A QSTRING without its quotes.
    QString(String),
    /// Structure mode: any other run of characters, escapes kept verbatim.
    Word(String),
    /// Expression mode: an identifier, escapes kept verbatim, with any bit
    /// spec that directly follows it.
    Ident(String),
    /// Expression mode: a number or a based constant such as `1'b0`.
    Const(String),
    Op(&'static str),
    LBrace,
    RBrace,
    Comma,
    Question,
    Colon,
    Eof,
}

impl Tok {
    /// How a diagnostic names the token.
    pub(super) fn describe(&self) -> String {
        match self {
            Self::LParen => "`(`".to_owned(),
            Self::RParen => "`)`".to_owned(),
            Self::QString(text) => format!("string \"{text}\""),
            Self::Word(text) | Self::Ident(text) | Self::Const(text) => format!("`{text}`"),
            Self::Op(op) => format!("`{op}`"),
            Self::LBrace => "`{`".to_owned(),
            Self::RBrace => "`}`".to_owned(),
            Self::Comma => "`,`".to_owned(),
            Self::Question => "`?`".to_owned(),
            Self::Colon => "`:`".to_owned(),
            Self::Eof => "the end of the file".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Token {
    pub(super) tok: Tok,
    pub(super) pos: SdfPos,
    mode: Mode,
}

impl Tok {
    /// Push the token's source bytes onto a pushback stack, last byte
    /// first. Every token is its source text: words, identifiers and
    /// constants keep their escapes and a string has no escapes.
    fn push_source(&self, stack: &mut Vec<u8>) {
        let text: &[u8] = match self {
            Self::LParen => b"(",
            Self::RParen => b")",
            Self::LBrace => b"{",
            Self::RBrace => b"}",
            Self::Comma => b",",
            Self::Question => b"?",
            Self::Colon => b":",
            Self::Eof => b"",
            Self::Op(op) => op.as_bytes(),
            Self::Word(text) | Self::Ident(text) | Self::Const(text) => text.as_bytes(),
            Self::QString(text) => {
                stack.push(b'"');
                stack.extend(text.bytes().rev());
                stack.push(b'"');
                return;
            }
        };
        stack.extend(text.iter().rev());
    }
}

pub(super) struct Lexer<R> {
    input: R,
    /// Bytes returned by [`Lexer::unread`], last byte first.
    pushback: Vec<u8>,
    line: u32,
    col: u32,
    /// Bytes taken from `input` so far (pushback does not count twice).
    consumed: u64,
    peeked: Option<Token>,
    limits: SdfLimits,
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// Identifier characters that need no escape: letters, digits and `_`.
pub(super) fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

impl<R: BufRead> Lexer<R> {
    pub(super) fn new(input: R, limits: SdfLimits) -> Self {
        Self {
            input,
            pushback: Vec::new(),
            line: 1,
            col: 1,
            consumed: 0,
            peeked: None,
            limits,
        }
    }

    pub(super) fn pos(&self) -> SdfPos {
        SdfPos::new(self.line, self.col)
    }

    fn error(&self, pos: SdfPos, message: impl Into<String>) -> SdfError {
        SdfError::new(pos, message)
    }

    fn peek_byte(&mut self) -> Result<Option<u8>, SdfError> {
        if let Some(byte) = self.pushback.last() {
            return Ok(Some(*byte));
        }
        let pos = self.pos();
        let buffer = self
            .input
            .fill_buf()
            .map_err(|error| SdfError::new(pos, format!("cannot read the SDF file: {error}")))?;
        Ok(buffer.first().copied())
    }

    /// The byte after the next one, without consuming either.
    fn peek_second(&mut self) -> Result<Option<u8>, SdfError> {
        match self.pushback.len() {
            0 => {}
            1 => {
                let pos = self.pos();
                let buffer = self.input.fill_buf().map_err(|error| {
                    SdfError::new(pos, format!("cannot read the SDF file: {error}"))
                })?;
                return Ok(buffer.first().copied());
            }
            count => return Ok(Some(self.pushback[count - 2])),
        }
        let pos = self.pos();
        let buffer = self
            .input
            .fill_buf()
            .map_err(|error| SdfError::new(pos, format!("cannot read the SDF file: {error}")))?;
        if buffer.len() >= 2 {
            return Ok(Some(buffer[1]));
        }
        if buffer.is_empty() {
            return Ok(None);
        }
        // The second byte is beyond this buffer: take the first one into the
        // pushback so the next fill shows the second.
        let first = buffer[0];
        self.input.consume(1);
        self.count_consumed(pos)?;
        self.pushback.push(first);
        let buffer = self
            .input
            .fill_buf()
            .map_err(|error| SdfError::new(pos, format!("cannot read the SDF file: {error}")))?;
        Ok(buffer.first().copied())
    }

    fn count_consumed(&mut self, pos: SdfPos) -> Result<(), SdfError> {
        self.consumed += 1;
        if self.consumed > self.limits.max_input_bytes {
            return Err(self.error(
                pos,
                format!(
                    "the SDF file is larger than the limit of {} bytes",
                    self.limits.max_input_bytes
                ),
            ));
        }
        Ok(())
    }

    /// Consume one byte and advance the position.
    fn bump(&mut self) -> Result<Option<u8>, SdfError> {
        let byte = match self.pushback.pop() {
            Some(byte) => byte,
            None => {
                let Some(byte) = self.peek_byte()? else {
                    return Ok(None);
                };
                self.input.consume(1);
                let pos = self.pos();
                self.count_consumed(pos)?;
                byte
            }
        };
        if byte == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Ok(Some(byte))
    }

    /// Consume `count` bytes of the current input buffer that contain no
    /// newline (the pushback must be empty).
    fn consume_run(&mut self, count: usize, pos: SdfPos) -> Result<(), SdfError> {
        self.input.consume(count);
        self.consumed += count as u64;
        if self.consumed > self.limits.max_input_bytes {
            return Err(self.error(
                pos,
                format!(
                    "the SDF file is larger than the limit of {} bytes",
                    self.limits.max_input_bytes
                ),
            ));
        }
        self.col += count as u32;
        Ok(())
    }

    /// Append the longest run of word bytes at the front of the input
    /// buffer that need no special handling (printable ASCII other than
    /// white space, `(`, `)`, `"`, `\` and `/`). This is the fast path of
    /// [`Lexer::lex_word`]; the bytes it stops at are handled one at a time.
    fn take_plain_run(&mut self, text: &mut Vec<u8>, pos: SdfPos) -> Result<(), SdfError> {
        if !self.pushback.is_empty() {
            return Ok(());
        }
        let here = self.pos();
        let buffer = self
            .input
            .fill_buf()
            .map_err(|error| SdfError::new(here, format!("cannot read the SDF file: {error}")))?;
        let count = buffer
            .iter()
            .position(|&byte| {
                !(0x21..0x7f).contains(&byte) || matches!(byte, b'(' | b')' | b'"' | b'\\' | b'/')
            })
            .unwrap_or(buffer.len());
        if count == 0 {
            return Ok(());
        }
        if text.len() + count > self.limits.max_token_bytes {
            return Err(self.error(
                pos,
                format!(
                    "a token is longer than the limit of {} bytes",
                    self.limits.max_token_bytes
                ),
            ));
        }
        text.extend_from_slice(&buffer[..count]);
        self.consume_run(count, pos)
    }

    /// Skip a run of spaces and tabs at the front of the input buffer.
    fn skip_blank_run(&mut self) -> Result<(), SdfError> {
        if !self.pushback.is_empty() {
            return Ok(());
        }
        let here = self.pos();
        let buffer = self
            .input
            .fill_buf()
            .map_err(|error| SdfError::new(here, format!("cannot read the SDF file: {error}")))?;
        let count = buffer
            .iter()
            .position(|&byte| !matches!(byte, b' ' | b'\t'))
            .unwrap_or(buffer.len());
        if count > 0 {
            self.consume_run(count, here)?;
        }
        Ok(())
    }

    /// Skip white space and comments.
    fn skip_trivia(&mut self) -> Result<(), SdfError> {
        loop {
            self.skip_blank_run()?;
            match self.peek_byte()? {
                Some(byte) if is_space(byte) => {
                    self.bump()?;
                }
                Some(b'/') => match self.peek_second()? {
                    Some(b'/') => {
                        while let Some(byte) = self.bump()? {
                            if byte == b'\n' {
                                break;
                            }
                        }
                    }
                    Some(b'*') => {
                        let start = self.pos();
                        self.bump()?;
                        self.bump()?;
                        let mut star = false;
                        loop {
                            match self.bump()? {
                                None => {
                                    return Err(self
                                        .error(start, "the comment starting here is not closed"))
                                }
                                Some(b'/') if star => break,
                                Some(byte) => star = byte == b'*',
                            }
                        }
                    }
                    _ => return Ok(()),
                },
                _ => return Ok(()),
            }
        }
    }

    /// The next token in `mode`. A token peeked in the other mode is
    /// returned to the input and lexed again.
    pub(super) fn next(&mut self, mode: Mode) -> Result<Token, SdfError> {
        if let Some(token) = self.peeked.take() {
            if token.mode == mode || matches!(token.tok, Tok::Eof) {
                return Ok(token);
            }
            self.unread(token);
        }
        self.lex(mode)
    }

    pub(super) fn peek(&mut self, mode: Mode) -> Result<&Token, SdfError> {
        if let Some(token) = self.peeked.take() {
            if token.mode == mode || matches!(token.tok, Tok::Eof) {
                self.peeked = Some(token);
            } else {
                self.unread(token);
            }
        }
        if self.peeked.is_none() {
            let token = self.lex(mode)?;
            self.peeked = Some(token);
        }
        Ok(self.peeked.as_ref().expect("a token was just peeked"))
    }

    /// Return a token to the input; its bytes are lexed again from its
    /// position.
    pub(super) fn unread(&mut self, token: Token) {
        token.tok.push_source(&mut self.pushback);
        self.line = token.pos.line;
        self.col = token.pos.col;
    }

    fn token(&self, tok: Tok, pos: SdfPos, mode: Mode) -> Token {
        Token { tok, pos, mode }
    }

    fn check_ascii(&self, byte: u8, pos: SdfPos) -> Result<(), SdfError> {
        if byte >= 0x80 {
            return Err(self.error(
                pos,
                format!("the character 0x{byte:02x} is not an SDF character (SDF files are ASCII)"),
            ));
        }
        if byte < 0x20 && !is_space(byte) {
            return Err(self.error(
                pos,
                format!("the control character 0x{byte:02x} is not an SDF character"),
            ));
        }
        Ok(())
    }

    fn lex(&mut self, mode: Mode) -> Result<Token, SdfError> {
        self.skip_trivia()?;
        let pos = self.pos();
        let Some(byte) = self.peek_byte()? else {
            return Ok(self.token(Tok::Eof, pos, mode));
        };
        self.check_ascii(byte, pos)?;
        match byte {
            b'(' => {
                self.bump()?;
                return Ok(self.token(Tok::LParen, pos, mode));
            }
            b')' => {
                self.bump()?;
                return Ok(self.token(Tok::RParen, pos, mode));
            }
            b'"' => return self.lex_qstring(pos, mode),
            _ => {}
        }
        match mode {
            Mode::Structure => self.lex_word(pos),
            Mode::Expression => self.lex_expression(pos, byte),
        }
    }

    fn lex_qstring(&mut self, pos: SdfPos, mode: Mode) -> Result<Token, SdfError> {
        self.bump()?;
        let mut text = Vec::new();
        loop {
            let here = self.pos();
            match self.bump()? {
                None => return Err(self.error(pos, "the string starting here is not closed")),
                Some(b'"') => break,
                Some(b'\n') => {
                    return Err(self.error(
                        here,
                        "a QSTRING cannot contain a newline (the string is not closed)",
                    ))
                }
                Some(b'\t') => {
                    return Err(self.error(here, "a QSTRING cannot contain a tab"));
                }
                Some(byte) => {
                    self.check_ascii(byte, here)?;
                    if text.len() >= self.limits.max_string_bytes {
                        return Err(self.error(
                            pos,
                            format!(
                                "the string is longer than the limit of {} bytes",
                                self.limits.max_string_bytes
                            ),
                        ));
                    }
                    text.push(byte);
                }
            }
        }
        let content = String::from_utf8(text).expect("ASCII bytes are UTF-8");
        Ok(self.token(Tok::QString(content), pos, mode))
    }

    fn push_limited(&self, text: &mut Vec<u8>, byte: u8, pos: SdfPos) -> Result<(), SdfError> {
        if text.len() >= self.limits.max_token_bytes {
            return Err(self.error(
                pos,
                format!(
                    "a token is longer than the limit of {} bytes",
                    self.limits.max_token_bytes
                ),
            ));
        }
        text.push(byte);
        Ok(())
    }

    /// An escape: the backslash and the escaped character, which must be a
    /// printable non-space character.
    fn lex_escape(&mut self, text: &mut Vec<u8>, start: SdfPos) -> Result<(), SdfError> {
        let here = self.pos();
        self.bump()?;
        self.push_limited(text, b'\\', start)?;
        match self.peek_byte()? {
            Some(byte) if !is_space(byte) => {
                self.check_ascii(byte, self.pos())?;
                self.bump()?;
                self.push_limited(text, byte, start)
            }
            _ => Err(self.error(
                here,
                "the escape character `\\` must be followed by the character it escapes",
            )),
        }
    }

    fn lex_word(&mut self, pos: SdfPos) -> Result<Token, SdfError> {
        let mut text = Vec::new();
        loop {
            self.take_plain_run(&mut text, pos)?;
            let Some(byte) = self.peek_byte()? else {
                break;
            };
            if is_space(byte) || matches!(byte, b'(' | b')' | b'"') {
                break;
            }
            self.check_ascii(byte, self.pos())?;
            if byte == b'\\' {
                self.lex_escape(&mut text, pos)?;
                continue;
            }
            if byte == b'/' && matches!(self.peek_second()?, Some(b'/' | b'*')) {
                // A comment ends the word.
                break;
            }
            self.bump()?;
            self.push_limited(&mut text, byte, pos)?;
        }
        let word = String::from_utf8(text).expect("ASCII bytes are UTF-8");
        Ok(self.token(Tok::Word(word), pos, Mode::Structure))
    }

    fn lex_expression(&mut self, pos: SdfPos, first: u8) -> Result<Token, SdfError> {
        let mode = Mode::Expression;
        let single = match first {
            b'{' => Some(Tok::LBrace),
            b'}' => Some(Tok::RBrace),
            b',' => Some(Tok::Comma),
            b'?' => Some(Tok::Question),
            b':' => Some(Tok::Colon),
            _ => None,
        };
        if let Some(tok) = single {
            self.bump()?;
            return Ok(self.token(tok, pos, mode));
        }
        if first == b'\\' || first.is_ascii_alphabetic() || first == b'_' || first == b'$' {
            return self.lex_identifier(pos);
        }
        if first.is_ascii_digit() || first == b'\'' {
            return self.lex_constant(pos);
        }
        // Operators, longest match first. The three-byte ones (`===`,
        // `!==`) extend a two-byte one by `=`.
        let second = self.peek_second()?;
        let matched = OPERATORS.iter().find(|op| match op.as_bytes() {
            [a, b] => *a == first && Some(*b) == second,
            [a] => *a == first,
            _ => false,
        });
        if let Some(op) = matched {
            for _ in 0..op.len() {
                self.bump()?;
            }
            let op: &'static str = match *op {
                "==" if self.peek_byte()? == Some(b'=') => "===",
                "!=" if self.peek_byte()? == Some(b'=') => "!==",
                other => other,
            };
            if op.len() == 3 {
                self.bump()?;
            }
            return Ok(self.token(Tok::Op(op), pos, mode));
        }
        Err(self.error(
            pos,
            format!("unexpected character `{}` in a condition", first as char),
        ))
    }

    fn lex_identifier(&mut self, pos: SdfPos) -> Result<Token, SdfError> {
        let mut text = Vec::new();
        while let Some(byte) = self.peek_byte()? {
            if byte == b'\\' {
                self.lex_escape(&mut text, pos)?;
                continue;
            }
            if !is_identifier_char(byte) && byte != b'$' {
                break;
            }
            self.bump()?;
            self.push_limited(&mut text, byte, pos)?;
        }
        // A bit spec directly after the identifier belongs to it.
        if self.peek_byte()? == Some(b'[') {
            while let Some(byte) = self.peek_byte()? {
                self.check_ascii(byte, self.pos())?;
                if is_space(byte) || matches!(byte, b'(' | b')' | b'"') {
                    break;
                }
                self.bump()?;
                self.push_limited(&mut text, byte, pos)?;
                if byte == b']' {
                    break;
                }
            }
        }
        let word = String::from_utf8(text).expect("ASCII bytes are UTF-8");
        Ok(self.token(Tok::Ident(word), pos, Mode::Expression))
    }

    fn lex_constant(&mut self, pos: SdfPos) -> Result<Token, SdfError> {
        let mut text = Vec::new();
        while let Some(byte) = self.peek_byte()? {
            if !(is_identifier_char(byte) || byte == b'\'' || byte == b'.') {
                break;
            }
            self.bump()?;
            self.push_limited(&mut text, byte, pos)?;
        }
        let word = String::from_utf8(text).expect("ASCII bytes are UTF-8");
        Ok(self.token(Tok::Const(word), pos, Mode::Expression))
    }

    /// Read the rest of the input as trivia; anything else is an error.
    pub(super) fn expect_end(&mut self) -> Result<(), SdfError> {
        let token = self.next(Mode::Structure)?;
        match token.tok {
            Tok::Eof => Ok(()),
            other => Err(self.error(
                token.pos,
                format!(
                    "unexpected {} after the end of the DELAYFILE",
                    other.describe()
                ),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(text: &str, mode: Mode) -> Vec<Tok> {
        let mut lexer = Lexer::new(text.as_bytes(), SdfLimits::default());
        let mut out = Vec::new();
        loop {
            let token = lexer.next(mode).expect("lexes");
            if token.tok == Tok::Eof {
                return out;
            }
            out.push(token.tok);
        }
    }

    fn word(text: &str) -> Tok {
        Tok::Word(text.to_owned())
    }

    #[test]
    fn structure_words_split_at_parentheses_quotes_and_space() {
        assert_eq!(
            tokens(
                "(DELAYFILE (SDFVERSION \"3.0\") (DIVIDER /)",
                Mode::Structure
            ),
            vec![
                Tok::LParen,
                word("DELAYFILE"),
                Tok::LParen,
                word("SDFVERSION"),
                Tok::QString("3.0".to_owned()),
                Tok::RParen,
                Tok::LParen,
                word("DIVIDER"),
                word("/"),
                Tok::RParen,
            ]
        );
        assert_eq!(
            tokens("(1.5:2:) a\\(b\\)[3] top/u1/x", Mode::Structure),
            vec![
                Tok::LParen,
                word("1.5:2:"),
                Tok::RParen,
                word("a\\(b\\)[3]"),
                word("top/u1/x"),
            ]
        );
    }

    #[test]
    fn comments_are_trivia() {
        assert_eq!(
            tokens("a // line\n/* block\n*/ b/*x*/c", Mode::Structure),
            vec![word("a"), word("b"), word("c")]
        );
        assert_eq!(tokens("a/b", Mode::Structure), vec![word("a/b")]);
        let mut lexer = Lexer::new("a /* open".as_bytes(), SdfLimits::default());
        lexer.next(Mode::Structure).unwrap();
        let error = lexer.next(Mode::Structure).unwrap_err();
        assert_eq!(
            error.to_string(),
            "1:3: the comment starting here is not closed"
        );
    }

    #[test]
    fn expression_tokens_split_operators() {
        assert_eq!(
            tokens("a&&!b[2]==1'b1 ~^c {x,y} ? : !== === ~&", Mode::Expression),
            vec![
                Tok::Ident("a".to_owned()),
                Tok::Op("&&"),
                Tok::Op("!"),
                Tok::Ident("b[2]".to_owned()),
                Tok::Op("=="),
                Tok::Const("1'b1".to_owned()),
                Tok::Op("~^"),
                Tok::Ident("c".to_owned()),
                Tok::LBrace,
                Tok::Ident("x".to_owned()),
                Tok::Comma,
                Tok::Ident("y".to_owned()),
                Tok::RBrace,
                Tok::Question,
                Tok::Colon,
                Tok::Op("!=="),
                Tok::Op("==="),
                Tok::Op("~&"),
            ]
        );
    }

    #[test]
    fn unread_relexes_in_the_other_mode() {
        let mut lexer = Lexer::new("en==1 a/b (x)".as_bytes(), SdfLimits::default());
        assert_eq!(
            lexer.next(Mode::Expression).unwrap().tok,
            Tok::Ident("en".to_owned())
        );
        assert_eq!(lexer.next(Mode::Expression).unwrap().tok, Tok::Op("=="));
        assert_eq!(
            lexer.next(Mode::Expression).unwrap().tok,
            Tok::Const("1".to_owned())
        );
        // Peeked as an identifier, then taken as a structure word.
        assert_eq!(
            lexer.peek(Mode::Expression).unwrap().tok,
            Tok::Ident("a".to_owned())
        );
        let token = lexer.next(Mode::Structure).unwrap();
        assert_eq!(token.tok, word("a/b"));
        assert_eq!(token.pos, SdfPos::new(1, 7));
        assert_eq!(lexer.next(Mode::Structure).unwrap().tok, Tok::LParen);
    }

    #[test]
    fn lexical_errors_name_their_position() {
        let error = |text: &str, mode: Mode| {
            let mut lexer = Lexer::new(text.as_bytes(), SdfLimits::default());
            loop {
                match lexer.next(mode) {
                    Ok(token) if token.tok == Tok::Eof => panic!("no error in {text:?}"),
                    Ok(_) => {}
                    Err(error) => return error.to_string(),
                }
            }
        };
        assert_eq!(
            error("\"abc", Mode::Structure),
            "1:1: the string starting here is not closed"
        );
        assert_eq!(
            error("x \"a\nb\"", Mode::Structure),
            "1:5: a QSTRING cannot contain a newline (the string is not closed)"
        );
        assert_eq!(
            error("a\\ b", Mode::Structure),
            "1:2: the escape character `\\` must be followed by the character it escapes"
        );
        assert_eq!(
            error("ab\u{e9}", Mode::Structure),
            "1:3: the character 0xc3 is not an SDF character (SDF files are ASCII)"
        );
        assert_eq!(
            error("a # b", Mode::Expression),
            "1:3: unexpected character `#` in a condition"
        );
    }

    #[test]
    fn limits_bound_tokens_and_input() {
        let limits = SdfLimits {
            max_token_bytes: 4,
            ..SdfLimits::default()
        };
        let mut lexer = Lexer::new("abcde".as_bytes(), limits);
        assert_eq!(
            lexer.next(Mode::Structure).unwrap_err().to_string(),
            "1:1: a token is longer than the limit of 4 bytes"
        );
        let limits = SdfLimits {
            max_input_bytes: 3,
            ..SdfLimits::default()
        };
        let mut lexer = Lexer::new("ab cd".as_bytes(), limits);
        assert_eq!(lexer.next(Mode::Structure).unwrap().tok, word("ab"));
        assert_eq!(
            lexer.next(Mode::Structure).unwrap_err().to_string(),
            "1:4: the SDF file is larger than the limit of 3 bytes"
        );
    }
}
