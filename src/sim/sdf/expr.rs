//! SDF conditions (OVI SDF 3.0 "Conditions for Path Delays", "Conditions for
//! Timing Checks", "Constants for Expressions", "Operators for Expressions").
//!
//! A condition parses into a [`CondExpr`] tree. Matching an SDF `COND`
//! against a specify path or timing check condition compares trees, so
//! parentheses, white space and the spelling of a scalar constant (`1'b1`,
//! `'B1`, `1`) do not matter, while operators and operands must be equal.
//! The HDL side is the condition's source text, parsed by the same parser
//! ([`parse_hdl_condition`]); a condition outside this expression subset
//! (a hierarchical name, a function call) has no tree and matches no SDF
//! condition.

use super::lexer::{Lexer, Mode, Tok};
use super::{SdfError, SdfLimits};
use std::fmt;
use std::io::BufRead;

/// A bit spec `[n]` or `[m:n]` of an identifier (DNUMBERs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfBits {
    Bit(u64),
    Range(u64, u64),
}

impl SdfBits {
    /// `(msb, lsb)` as written.
    pub fn bounds(self) -> (u64, u64) {
        match self {
            Self::Bit(bit) => (bit, bit),
            Self::Range(msb, lsb) => (msb, lsb),
        }
    }
}

impl fmt::Display for SdfBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bit(bit) => write!(f, "[{bit}]"),
            Self::Range(msb, lsb) => write!(f, "[{msb}:{lsb}]"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    BitNot,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    CaseEq,
    CaseNe,
    BitAnd,
    BitXor,
    BitXnor,
    BitOr,
    LogAnd,
    LogOr,
}

impl UnaryOp {
    fn from_token(op: &str) -> Option<Self> {
        Some(match op {
            "+" => Self::Plus,
            "-" => Self::Minus,
            "!" => Self::Not,
            "~" => Self::BitNot,
            "&" => Self::And,
            "~&" => Self::Nand,
            "|" => Self::Or,
            "~|" => Self::Nor,
            "^" => Self::Xor,
            "^~" | "~^" => Self::Xnor,
            _ => return None,
        })
    }

    fn text(self) -> &'static str {
        match self {
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Not => "!",
            Self::BitNot => "~",
            Self::And => "&",
            Self::Nand => "~&",
            Self::Or => "|",
            Self::Nor => "~|",
            Self::Xor => "^",
            Self::Xnor => "~^",
        }
    }
}

impl BinaryOp {
    fn from_token(op: &str) -> Option<Self> {
        Some(match op {
            "*" => Self::Mul,
            "/" => Self::Div,
            "%" => Self::Mod,
            "+" => Self::Add,
            "-" => Self::Sub,
            "<<" => Self::Shl,
            ">>" => Self::Shr,
            "<" => Self::Lt,
            "<=" => Self::Le,
            ">" => Self::Gt,
            ">=" => Self::Ge,
            "==" => Self::Eq,
            "!=" => Self::Ne,
            "===" => Self::CaseEq,
            "!==" => Self::CaseNe,
            "&" => Self::BitAnd,
            "^" => Self::BitXor,
            "^~" | "~^" => Self::BitXnor,
            "|" => Self::BitOr,
            "&&" => Self::LogAnd,
            "||" => Self::LogOr,
            _ => return None,
        })
    }

    /// Binding strength from "Precedence Rules of SDF Operators" (higher
    /// binds tighter).
    fn precedence(self) -> u8 {
        match self {
            Self::Mul | Self::Div | Self::Mod => 10,
            Self::Add | Self::Sub => 9,
            Self::Shl | Self::Shr => 8,
            Self::Lt | Self::Le | Self::Gt | Self::Ge => 7,
            Self::Eq | Self::Ne | Self::CaseEq | Self::CaseNe => 6,
            Self::BitAnd => 5,
            Self::BitXor | Self::BitXnor => 4,
            Self::BitOr => 3,
            Self::LogAnd => 2,
            Self::LogOr => 1,
        }
    }

    pub fn is_equality(self) -> bool {
        matches!(self, Self::Eq | Self::Ne | Self::CaseEq | Self::CaseNe)
    }

    fn text(self) -> &'static str {
        match self {
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::CaseEq => "===",
            Self::CaseNe => "!==",
            Self::BitAnd => "&",
            Self::BitXor => "^",
            Self::BitXnor => "~^",
            Self::BitOr => "|",
            Self::LogAnd => "&&",
            Self::LogOr => "||",
        }
    }
}

/// A condition tree. Constants are compared by value: the scalar constant
/// forms `1'b0`, `'b0` and `0` are all `Const(0)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CondExpr {
    Ident { name: String, bits: Option<SdfBits> },
    Const(u128),
    Unary(UnaryOp, Box<CondExpr>),
    Binary(BinaryOp, Box<CondExpr>, Box<CondExpr>),
    Ternary(Box<CondExpr>, Box<CondExpr>, Box<CondExpr>),
    Concat(Vec<CondExpr>),
    Replicate(Box<CondExpr>, Vec<CondExpr>),
}

impl CondExpr {
    /// A scalar constant (0 or 1) as SCALAR_CONSTANT allows.
    pub fn is_scalar_constant(&self) -> bool {
        matches!(self, Self::Const(0 | 1))
    }
}

impl fmt::Display for CondExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ident { name, bits } => {
                write_identifier(f, name)?;
                if let Some(bits) = bits {
                    write!(f, "{bits}")?;
                }
                Ok(())
            }
            Self::Const(value) => write!(f, "{value}"),
            Self::Unary(op, operand) => write!(f, "{}({operand})", op.text()),
            Self::Binary(op, left, right) => write!(f, "({left} {} {right})", op.text()),
            Self::Ternary(condition, yes, no) => write!(f, "({condition} ? {yes} : {no})"),
            Self::Concat(items) => {
                f.write_str("{")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("}")
            }
            Self::Replicate(count, items) => {
                write!(f, "{{{count}{{")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("}}")
            }
        }
    }
}

/// Write an identifier with SDF escapes for its special characters.
pub fn write_identifier(f: &mut impl fmt::Write, name: &str) -> fmt::Result {
    for character in name.chars() {
        if !(character.is_ascii_alphanumeric() || character == '_') {
            f.write_char('\\')?;
        }
        f.write_char(character)?;
    }
    Ok(())
}

/// Split an identifier word (`name`, `name[3]`, `name[7:0]`, escapes kept)
/// into its unescaped name and bit spec. A character other than a letter,
/// digit or `_` must be escaped, so an unescaped hierarchy divider of
/// either kind is reported.
pub(super) fn split_identifier(
    word: &str,
    what: &str,
) -> Result<(String, Option<SdfBits>), String> {
    let bytes = word.as_bytes();
    let mut name = String::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                // The lexer guarantees an escaped character follows.
                let Some(escaped) = bytes.get(index + 1) else {
                    return Err(format!("{what} `{word}` ends with an escape character"));
                };
                name.push(*escaped as char);
                index += 2;
            }
            b'[' => break,
            byte if super::lexer::is_identifier_char(byte) => {
                name.push(byte as char);
                index += 1;
            }
            byte => {
                return Err(format!(
                    "the character `{}` in {what} `{word}` must be escaped with `\\`",
                    byte as char
                ))
            }
        }
    }
    if name.is_empty() {
        return Err(format!("{what} `{word}` has no name"));
    }
    if name.chars().count() > super::MAX_IDENTIFIER_CHARS {
        return Err(format!(
            "{what} is longer than {} characters",
            super::MAX_IDENTIFIER_CHARS
        ));
    }
    if index == bytes.len() {
        return Ok((name, None));
    }
    // A bit spec ends the identifier: `[DNUMBER]` or `[DNUMBER:DNUMBER]`.
    let spec = &word[index..];
    let malformed = || {
        format!("the bit spec `{spec}` of {what} `{word}` is not `[n]` or `[m:n]` with non-negative integers")
    };
    let Some(inner) = spec
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return Err(malformed());
    };
    let number = |text: &str| -> Option<u64> {
        let digits = text.strip_prefix('+').unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    };
    let bits = match inner.split_once(':') {
        None => SdfBits::Bit(number(inner).ok_or_else(malformed)?),
        Some((msb, lsb)) => SdfBits::Range(
            number(msb).ok_or_else(malformed)?,
            number(lsb).ok_or_else(malformed)?,
        ),
    };
    Ok((name, Some(bits)))
}

/// Parse a constant token: `0`, `1`, `1'b0`, `'b1`, `'B1`, or a decimal or
/// based number of another value (allowed in HDL conditions).
fn parse_constant(text: &str) -> Option<u128> {
    if let Some((size, rest)) = text.split_once('\'') {
        if !size.is_empty() && !size.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let rest = rest.strip_prefix(['s', 'S']).unwrap_or(rest);
        let mut chars = rest.chars();
        let radix = match chars.next()?.to_ascii_lowercase() {
            'b' => 2,
            'o' => 8,
            'd' => 10,
            'h' => 16,
            _ => return None,
        };
        let digits: String = chars.filter(|character| *character != '_').collect();
        if digits.is_empty() {
            return None;
        }
        return u128::from_str_radix(&digits, radix).ok();
    }
    let digits: String = text.chars().filter(|character| *character != '_').collect();
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Recursive-descent parser over the expression-mode tokens of a lexer.
pub(super) struct ExprParser<'l, R> {
    lexer: &'l mut Lexer<R>,
    depth: usize,
    nodes: usize,
    limits: SdfLimits,
}

impl<'l, R: BufRead> ExprParser<'l, R> {
    pub(super) fn new(lexer: &'l mut Lexer<R>, limits: SdfLimits) -> Self {
        Self {
            lexer,
            depth: 0,
            nodes: 0,
            limits,
        }
    }

    fn count_node(&mut self) -> Result<(), SdfError> {
        self.nodes += 1;
        if self.nodes > self.limits.max_condition_nodes {
            let pos = self.lexer.pos();
            return Err(SdfError::new(
                pos,
                format!(
                    "the condition has more than {} operands and operators",
                    self.limits.max_condition_nodes
                ),
            ));
        }
        Ok(())
    }

    fn enter(&mut self) -> Result<(), SdfError> {
        self.depth += 1;
        if self.depth > self.limits.max_condition_depth {
            let pos = self.lexer.pos();
            return Err(SdfError::new(
                pos,
                format!(
                    "the condition nests deeper than {} levels",
                    self.limits.max_condition_depth
                ),
            ));
        }
        Ok(())
    }

    /// A conditional_port_expr. It ends before a token that cannot continue
    /// it, such as the `(` of the `IOPATH` that follows.
    pub(super) fn expression(&mut self) -> Result<CondExpr, SdfError> {
        self.enter()?;
        let condition = self.binary(1)?;
        let result = if self.lexer.peek(Mode::Expression)?.tok == Tok::Question {
            self.lexer.next(Mode::Expression)?;
            self.count_node()?;
            let yes = self.expression()?;
            let colon = self.lexer.next(Mode::Expression)?;
            if colon.tok != Tok::Colon {
                return Err(SdfError::new(
                    colon.pos,
                    format!(
                        "expected `:` of a conditional operator, found {}",
                        colon.tok.describe()
                    ),
                ));
            }
            let no = self.expression()?;
            CondExpr::Ternary(Box::new(condition), Box::new(yes), Box::new(no))
        } else {
            condition
        };
        self.depth -= 1;
        Ok(result)
    }

    /// The next token as a binary operator binding at least as tightly as
    /// `minimum`.
    fn binary_operator(&mut self, minimum: u8) -> Result<Option<BinaryOp>, SdfError> {
        Ok(match &self.lexer.peek(Mode::Expression)?.tok {
            Tok::Op(op) => BinaryOp::from_token(op).filter(|op| op.precedence() >= minimum),
            _ => None,
        })
    }

    fn binary(&mut self, minimum: u8) -> Result<CondExpr, SdfError> {
        let mut left = self.unary()?;
        while let Some(op) = self.binary_operator(minimum)? {
            self.lexer.next(Mode::Expression)?;
            self.count_node()?;
            self.enter()?;
            let right = self.binary(op.precedence() + 1)?;
            self.depth -= 1;
            left = CondExpr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<CondExpr, SdfError> {
        let token = self.lexer.next(Mode::Expression)?;
        self.count_node()?;
        match token.tok {
            Tok::Op(op) => {
                let Some(op) = UnaryOp::from_token(op) else {
                    return Err(SdfError::new(
                        token.pos,
                        format!("`{op}` is not a unary operator"),
                    ));
                };
                self.enter()?;
                let operand = self.unary()?;
                self.depth -= 1;
                Ok(CondExpr::Unary(op, Box::new(operand)))
            }
            Tok::LParen => {
                let inner = self.expression()?;
                let close = self.lexer.next(Mode::Expression)?;
                if close.tok != Tok::RParen {
                    return Err(SdfError::new(
                        close.pos,
                        format!(
                            "expected `)` closing the parenthesized condition, found {}",
                            close.tok.describe()
                        ),
                    ));
                }
                Ok(inner)
            }
            Tok::LBrace => self.concatenation(),
            Tok::Ident(word) => {
                let (name, bits) = split_identifier(&word, "the identifier")
                    .map_err(|message| SdfError::new(token.pos, message))?;
                Ok(CondExpr::Ident { name, bits })
            }
            Tok::Const(text) => parse_constant(&text)
                .map(CondExpr::Const)
                .ok_or_else(|| SdfError::new(token.pos, format!("`{text}` is not a constant"))),
            other => Err(SdfError::new(
                token.pos,
                format!("expected a condition operand, found {}", other.describe()),
            )),
        }
    }

    /// `{a, b}` or `{n{a, b}}`, after the opening brace.
    fn concatenation(&mut self) -> Result<CondExpr, SdfError> {
        self.enter()?;
        let first = self.expression()?;
        let result = if self.lexer.peek(Mode::Expression)?.tok == Tok::LBrace {
            self.lexer.next(Mode::Expression)?;
            let items = self.items(Tok::RBrace)?;
            CondExpr::Replicate(Box::new(first), items)
        } else {
            let mut items = vec![first];
            if self.lexer.peek(Mode::Expression)?.tok == Tok::Comma {
                self.lexer.next(Mode::Expression)?;
                items.extend(self.items(Tok::RBrace)?);
                CondExpr::Concat(items)
            } else {
                self.close(Tok::RBrace)?;
                CondExpr::Concat(items)
            }
        };
        if let CondExpr::Replicate(..) = result {
            self.close(Tok::RBrace)?;
        }
        self.depth -= 1;
        Ok(result)
    }

    /// Comma-separated expressions up to and including `close`.
    fn items(&mut self, close: Tok) -> Result<Vec<CondExpr>, SdfError> {
        let mut items = vec![self.expression()?];
        while self.lexer.peek(Mode::Expression)?.tok == Tok::Comma {
            self.lexer.next(Mode::Expression)?;
            items.push(self.expression()?);
        }
        self.close(close)?;
        Ok(items)
    }

    fn close(&mut self, close: Tok) -> Result<(), SdfError> {
        let token = self.lexer.next(Mode::Expression)?;
        if token.tok != close {
            return Err(SdfError::new(
                token.pos,
                format!(
                    "expected {} in a concatenation, found {}",
                    close.describe(),
                    token.tok.describe()
                ),
            ));
        }
        Ok(())
    }

    /// A timing_check_condition: `node`, `~node`, `!node` or
    /// `node == constant` (also `!=`, `===`, `!==`), where a node is a
    /// scalar port or net. Ends before the next token.
    pub(super) fn timing_check_condition(&mut self) -> Result<CondExpr, SdfError> {
        let first = self.lexer.next(Mode::Expression)?;
        let inversion = match &first.tok {
            Tok::Op("!") => Some(UnaryOp::Not),
            Tok::Op("~") => Some(UnaryOp::BitNot),
            _ => None,
        };
        let node_token = if inversion.is_some() {
            self.lexer.next(Mode::Expression)?
        } else {
            first
        };
        let Tok::Ident(word) = &node_token.tok else {
            return Err(SdfError::new(
                node_token.pos,
                format!(
                    "expected the scalar port or net of a timing check condition, found {}",
                    node_token.tok.describe()
                ),
            ));
        };
        let (name, bits) = split_identifier(word, "the timing check condition node")
            .map_err(|message| SdfError::new(node_token.pos, message))?;
        if let Some(SdfBits::Range(..)) = bits {
            return Err(SdfError::new(
                node_token.pos,
                format!("the timing check condition node `{word}` must be scalar, not a bus"),
            ));
        }
        let node = CondExpr::Ident { name, bits };
        if let Some(op) = inversion {
            return Ok(CondExpr::Unary(op, Box::new(node)));
        }
        let op = match &self.lexer.peek(Mode::Expression)?.tok {
            Tok::Op(op) => BinaryOp::from_token(op).filter(|op| op.is_equality()),
            _ => None,
        };
        let Some(op) = op else {
            return Ok(node);
        };
        self.lexer.next(Mode::Expression)?;
        let constant = self.lexer.next(Mode::Expression)?;
        let value = match &constant.tok {
            Tok::Const(text) => parse_scalar_constant(text),
            _ => None,
        };
        let Some(value) = value else {
            return Err(SdfError::new(
                constant.pos,
                format!(
                    "expected a scalar constant (0, 1, 1'b0, 1'b1, 'b0 or 'b1), found {}",
                    constant.tok.describe()
                ),
            ));
        };
        Ok(CondExpr::Binary(
            op,
            Box::new(node),
            Box::new(CondExpr::Const(value)),
        ))
    }
}

/// SCALAR_CONSTANT: `1'b0`, `1'b1`, `1'B0`, `1'B1`, `'b0`, `'b1`, `'B0`,
/// `'B1`, `0` or `1`.
fn parse_scalar_constant(text: &str) -> Option<u128> {
    match text {
        "0" | "1'b0" | "1'B0" | "'b0" | "'B0" => Some(0),
        "1" | "1'b1" | "1'B1" | "'b1" | "'B1" => Some(1),
        _ => None,
    }
}

/// Parse a specify path or timing check condition from its HDL source
/// text. Verilog escaped identifiers (`\a+b ` up to white space) become SDF
/// escapes first. `None` when the text is not in the SDF condition subset.
pub fn parse_hdl_condition(text: &str) -> Option<CondExpr> {
    let mut converted = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut previous = ' ';
    while let Some(character) = chars.next() {
        // `$` is an identifier character in HDL but special in SDF.
        if character == '$' && (previous.is_ascii_alphanumeric() || previous == '_') {
            converted.push_str("\\$");
            continue;
        }
        previous = character;
        if character == '\\' {
            while let Some(&next) = chars.peek() {
                if next.is_whitespace() {
                    break;
                }
                if !(next.is_ascii_alphanumeric() || next == '_') {
                    converted.push('\\');
                }
                converted.push(next);
                chars.next();
            }
            continue;
        }
        converted.push(character);
    }
    let limits = SdfLimits::default();
    let mut lexer = Lexer::new(converted.as_bytes(), limits);
    let expression = ExprParser::new(&mut lexer, limits).expression().ok()?;
    match lexer.next(Mode::Expression).ok()?.tok {
        Tok::Eof => Some(expression),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sdf(text: &str) -> CondExpr {
        let limits = SdfLimits::default();
        let mut lexer = Lexer::new(text.as_bytes(), limits);
        let expression = ExprParser::new(&mut lexer, limits)
            .expression()
            .unwrap_or_else(|error| panic!("{text}: {error}"));
        assert_eq!(
            lexer.next(Mode::Expression).unwrap().tok,
            Tok::Eof,
            "{text}"
        );
        expression
    }

    #[test]
    fn precedence_follows_the_sdf_table() {
        assert_eq!(sdf("a && b || c").to_string(), "((a && b) || c)");
        assert_eq!(sdf("a | b & c").to_string(), "(a | (b & c))");
        assert_eq!(sdf("a == 1 && b").to_string(), "((a == 1) && b)");
        assert_eq!(sdf("!a & ~b").to_string(), "(!(a) & ~(b))");
        assert_eq!(sdf("a + b * c").to_string(), "(a + (b * c))");
        assert_eq!(sdf("a - b - c").to_string(), "((a - b) - c)");
        assert_eq!(sdf("s ? a : b").to_string(), "(s ? a : b)");
        assert_eq!(sdf("{a, b[2]} == 'b1").to_string(), "({a, b[2]} == 1)");
        assert_eq!(sdf("{2{a}}").to_string(), "{2{a}}");
        assert_eq!(sdf("~&bus[3:0]").to_string(), "~&(bus[3:0])");
    }

    #[test]
    fn spelling_does_not_matter() {
        assert_eq!(sdf("(TE == 0 && RB == 1'b1)"), sdf("TE==1'B0&&(RB=='b1)"));
        assert_ne!(sdf("!a"), sdf("~a"));
        assert_ne!(sdf("a == 1"), sdf("a === 1"));
        assert_eq!(
            sdf("a\\+b"),
            CondExpr::Ident {
                name: "a+b".to_owned(),
                bits: None
            }
        );
    }

    #[test]
    fn hdl_text_uses_the_same_trees() {
        assert_eq!(parse_hdl_condition("mode == 1'b1"), Some(sdf("mode==1")));
        assert_eq!(parse_hdl_condition("(!rst)"), Some(sdf("!rst")));
        assert_eq!(parse_hdl_condition("\\a+b  && c"), Some(sdf("a\\+b && c")));
        assert_eq!(parse_hdl_condition("u.sig"), None);
        assert_eq!(parse_hdl_condition("f(a)"), None);
        assert_eq!(parse_hdl_condition("a == 4'hA"), Some(sdf("a == 10")));
    }

    #[test]
    fn timing_check_conditions_are_restricted() {
        let parse = |text: &str| {
            let limits = SdfLimits::default();
            let mut lexer = Lexer::new(text.as_bytes(), limits);
            ExprParser::new(&mut lexer, limits)
                .timing_check_condition()
                .map(|expression| expression.to_string())
                .map_err(|error| error.to_string())
        };
        assert_eq!(parse("~reset din"), Ok("~(reset)".to_owned()));
        assert_eq!(parse("en==1'b1 d"), Ok("(en == 1)".to_owned()));
        assert_eq!(parse("en !== 'B0"), Ok("(en !== 0)".to_owned()));
        assert_eq!(parse("en (posedge clk)"), Ok("en".to_owned()));
        assert_eq!(
            parse("en == 2"),
            Err(
                "1:7: expected a scalar constant (0, 1, 1'b0, 1'b1, 'b0 or 'b1), found `2`"
                    .to_owned()
            )
        );
        assert_eq!(
            parse("b[3:0]"),
            Err(
                "1:1: the timing check condition node `b[3:0]` must be scalar, not a bus"
                    .to_owned()
            )
        );
        assert_eq!(
            parse("(en)"),
            Err(
                "1:1: expected the scalar port or net of a timing check condition, found `(`"
                    .to_owned()
            )
        );
    }

    #[test]
    fn malformed_conditions_report_their_position() {
        let error = |text: &str| {
            let limits = SdfLimits::default();
            let mut lexer = Lexer::new(text.as_bytes(), limits);
            ExprParser::new(&mut lexer, limits)
                .expression()
                .unwrap_err()
                .to_string()
        };
        assert_eq!(
            error("a &&"),
            "1:5: expected a condition operand, found the end of the file"
        );
        assert_eq!(
            error("(a"),
            "1:3: expected `)` closing the parenthesized condition, found the end of the file"
        );
        assert_eq!(
            error("a ? b"),
            "1:6: expected `:` of a conditional operator, found the end of the file"
        );
        assert_eq!(error("== a"), "1:1: `==` is not a unary operator");
        assert_eq!(error("a[x]"), "1:1: the bit spec `[x]` of the identifier `a[x]` is not `[n]` or `[m:n]` with non-negative integers");
        let deep = format!("{}a{}", "(".repeat(100), ")".repeat(100));
        assert_eq!(
            error(&deep),
            "1:65: the condition nests deeper than 64 levels"
        );
    }

    #[test]
    fn identifiers_split_into_name_and_bits() {
        assert_eq!(
            split_identifier("pipe4\\-done\\&enb[3]", "x"),
            Ok(("pipe4-done&enb".to_owned(), Some(SdfBits::Bit(3))))
        );
        assert_eq!(
            split_identifier("mem_array\\[0\\:1023\\]\\(0\\:15\\)", "x"),
            Ok(("mem_array[0:1023](0:15)".to_owned(), None))
        );
        assert_eq!(
            split_identifier("d[+7:0]", "x"),
            Ok(("d".to_owned(), Some(SdfBits::Range(7, 0))))
        );
        assert_eq!(
            split_identifier("a+b", "the port"),
            Err("the character `+` in the port `a+b` must be escaped with `\\`".to_owned())
        );
        assert_eq!(
            split_identifier("a[3]b", "the port"),
            Err("the bit spec `[3]b` of the port `a[3]b` is not `[n]` or `[m:n]` with non-negative integers".to_owned())
        );
    }
}
