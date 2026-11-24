//! The expression corpus of FIN-002 step 3d: random well-typed integral
//! expressions against an independent evaluator.
//!
//! Each case generates one module from the fixed corpus seed: variables of
//! mixed width, signedness and state domain take new values every round
//! (from another process, so the values are not constants of the checking
//! process), and every expression is used in one of five contexts: the
//! right-hand side of a blocking assignment, a self-determined `$display`
//! argument, an `if` condition, a function actual and a continuous
//! assignment. The `constant` variant replaces the variables by parameters,
//! which sends the same expressions through constant evaluation.
//!
//! The oracle below is the text of IEEE 1800-2009 written out bit by bit:
//! expression sizes and types (11.6.1 Table 11-21, 11.8.1), propagation of
//! the context size and type to the operands (11.8.2), assignment extension
//! and truncation (11.8.3, 10.7) and the operator tables of 11.4. It shares
//! nothing with the simulator. A difference is minimized by hand into
//! `fin_002b/fuzz_expr_*.sv` and stays in the tree as an ordinary fixture
//! of the parent module.
//!
//! Every module runs through the public command line in both optimizer
//! modes on the legacy, compact/portable and compact/GMP value backends.
use crate::fuzz::Fuzz;
use crate::{sim_cli, sim_harness};
use std::fmt::Write as _;
use std::process::Command;
use std::time::Duration;

/// Rounds of new variable values in one module.
const ROUNDS: usize = 4;
/// Expressions checked in each round.
const EXPRESSIONS: usize = 36;
/// Continuous assignments, each checked in every round.
const WIRES: usize = 6;
/// No generated expression is wider than this.
const MAX_WIDTH: u32 = 384;
/// One `llg` run (lint, generation, model build and execution).
const RUN_TIMEOUT: Duration = Duration::from_secs(240);

const X: u8 = 2;
const Z: u8 = 3;
/// A four-state value, least significant bit first.
type Bits = Vec<u8>;

struct Var {
    declaration: &'static str,
    width: u32,
    signed: bool,
    four_state: bool,
}

const fn var(declaration: &'static str, width: u32, signed: bool, four_state: bool) -> Var {
    Var {
        declaration,
        width,
        signed,
        four_state,
    }
}

/// The operands. Widths sit on both sides of the 32, 64 and 128 bit
/// boundaries of the value representations.
const VARS: &[Var] = &[
    var("logic", 1, false, true),
    var("logic [2:0]", 3, false, true),
    var("logic signed [7:0]", 8, true, true),
    var("byte", 8, true, false),
    var("logic [12:0]", 13, false, true),
    var("shortint", 16, true, false),
    var("integer", 32, true, true),
    var("int", 32, true, false),
    var("bit [32:0]", 33, false, false),
    var("logic signed [32:0]", 33, true, true),
    var("longint", 64, true, false),
    var("logic [63:0]", 64, false, true),
    var("logic signed [64:0]", 65, true, true),
    var("bit [99:0]", 100, false, false),
    var("logic signed [127:0]", 128, true, true),
    var("logic [128:0]", 129, false, true),
    var("bit signed [199:0]", 200, true, false),
    var("logic [255:0]", 256, false, true),
];

/// Assignment targets and function formals: (width, signed, four-state).
const TARGETS: &[(u32, bool, bool)] = &[
    (1, false, true),
    (8, true, true),
    (8, false, false),
    (32, true, false),
    (33, false, true),
    (64, true, true),
    (70, false, false),
    (128, false, true),
    (130, true, true),
    (200, false, false),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Unary {
    Plus,
    Minus,
    Not,
    LogicalNot,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Binary {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    BitAnd,
    BitOr,
    BitXor,
    BitXnor,
    Eq,
    Ne,
    CaseEq,
    CaseNe,
    WildEq,
    WildNe,
    Lt,
    Le,
    Gt,
    Ge,
    LogicalAnd,
    LogicalOr,
    Shl,
    Shr,
    AShl,
    AShr,
}

#[derive(Clone, Debug)]
enum Expr {
    Var(usize),
    Literal {
        text: String,
        bits: Bits,
        signed: bool,
    },
    /// `'0`, `'1`, `'x` or `'z`.
    Fill(u8),
    Unary(Unary, Box<Expr>),
    Binary(Binary, Box<Expr>, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    Concat(Vec<Expr>),
    Replicate(u32, Box<Expr>),
    /// `v[high:low]`.
    Part(usize, u32, u32),
    /// `v[index]`.
    Bit(usize, Box<Expr>),
    /// `v[index +: width]`.
    Indexed(usize, Box<Expr>, u32),
    /// `$signed(e)` or `$unsigned(e)`.
    Sign(bool, Box<Expr>),
    /// `width'(e)`.
    Size(u32, Box<Expr>),
}

fn has_unknown(bits: &[u8]) -> bool {
    bits.iter().any(|bit| *bit >= X)
}

fn unknown(width: u32) -> Bits {
    vec![X; width as usize]
}

fn number(mut value: u128, width: u32) -> Bits {
    (0..width)
        .map(|_| {
            let bit = (value & 1) as u8;
            value >>= 1;
            bit
        })
        .collect()
}

/// `bits` in a context of `width` bits: extended by its sign bit when the
/// propagated type is signed (11.8.2), else by zeros; truncated when the
/// target is narrower (10.7).
fn resize(mut bits: Bits, width: u32, signed: bool) -> Bits {
    let fill = if signed {
        *bits.last().unwrap_or(&0)
    } else {
        0
    };
    bits.resize(width as usize, fill);
    bits
}

fn add(left: &[u8], right: &[u8]) -> Bits {
    let mut carry = 0;
    left.iter()
        .zip(right)
        .map(|(a, b)| {
            let sum = a + b + carry;
            carry = sum >> 1;
            sum & 1
        })
        .collect()
}

fn negate(value: &[u8]) -> Bits {
    let inverted: Bits = value.iter().map(|bit| 1 - bit).collect();
    add(&inverted, &number(1, value.len() as u32))
}

fn multiply(left: &[u8], right: &[u8]) -> Bits {
    let mut product = vec![0; left.len()];
    for (shift, bit) in right.iter().enumerate() {
        if *bit == 1 {
            let mut shifted = vec![0; shift];
            shifted.extend_from_slice(&left[..left.len() - shift]);
            product = add(&product, &shifted);
        }
    }
    product
}

/// `left < right` as unsigned numbers of one width.
fn below(left: &[u8], right: &[u8]) -> bool {
    for (a, b) in left.iter().zip(right).rev() {
        if a != b {
            return a < b;
        }
    }
    false
}

fn is_zero(value: &[u8]) -> bool {
    value.iter().all(|bit| *bit == 0)
}

fn negative(value: &[u8], signed: bool) -> bool {
    signed && value.last() == Some(&1)
}

/// Unsigned quotient and remainder by restoring division.
fn divide(left: &[u8], right: &[u8]) -> (Bits, Bits) {
    let width = left.len();
    let mut quotient = vec![0; width];
    let mut remainder = vec![0; width];
    for position in (0..width).rev() {
        // The remainder never exceeds the divisor, so the shifted-out bit
        // matters only when it is set: then the subtraction is due.
        let overflow = remainder[width - 1] == 1;
        remainder.rotate_right(1);
        remainder[0] = left[position];
        if overflow || !below(&remainder, right) {
            remainder = add(&remainder, &negate(right));
            quotient[position] = 1;
        }
    }
    (quotient, remainder)
}

/// Truth of a condition (11.4.7, 12.4): 1 when a bit is 1, 0 when all bits
/// are 0, else unknown.
fn truth(value: &[u8]) -> u8 {
    if value.contains(&1) {
        1
    } else if has_unknown(value) {
        X
    } else {
        0
    }
}

struct Oracle<'a> {
    values: &'a [Bits],
}

impl Oracle<'_> {
    /// Self-determined size (Table 11-21).
    fn width(expression: &Expr) -> u32 {
        match expression {
            Expr::Var(index) => VARS[*index].width,
            Expr::Literal { bits, .. } => bits.len() as u32,
            Expr::Fill(_) => 1,
            Expr::Unary(Unary::Plus | Unary::Minus | Unary::Not, operand) => Self::width(operand),
            Expr::Unary(..) => 1,
            Expr::Binary(operator, left, right) => match operator {
                Binary::Add
                | Binary::Sub
                | Binary::Mul
                | Binary::Div
                | Binary::Mod
                | Binary::BitAnd
                | Binary::BitOr
                | Binary::BitXor
                | Binary::BitXnor => Self::width(left).max(Self::width(right)),
                Binary::Pow | Binary::Shl | Binary::Shr | Binary::AShl | Binary::AShr => {
                    Self::width(left)
                }
                _ => 1,
            },
            Expr::Conditional(_, left, right) => Self::width(left).max(Self::width(right)),
            Expr::Concat(parts) => parts.iter().map(Self::width).sum(),
            Expr::Replicate(count, operand) => count * Self::width(operand),
            Expr::Part(_, high, low) => high - low + 1,
            Expr::Bit(..) => 1,
            Expr::Indexed(_, _, width) | Expr::Size(width, _) => *width,
            Expr::Sign(_, operand) => Self::width(operand),
        }
    }

    /// Type of the expression (11.8.1): signed only when every operand that
    /// is not self-determined is signed.
    fn signed(expression: &Expr) -> bool {
        match expression {
            Expr::Var(index) => VARS[*index].signed,
            Expr::Literal { signed, .. } => *signed,
            Expr::Unary(Unary::Plus | Unary::Minus | Unary::Not, operand) => Self::signed(operand),
            Expr::Binary(operator, left, right) => match operator {
                Binary::Add
                | Binary::Sub
                | Binary::Mul
                | Binary::Div
                | Binary::Mod
                | Binary::BitAnd
                | Binary::BitOr
                | Binary::BitXor
                | Binary::BitXnor => Self::signed(left) && Self::signed(right),
                Binary::Pow | Binary::Shl | Binary::Shr | Binary::AShl | Binary::AShr => {
                    Self::signed(left)
                }
                _ => false,
            },
            Expr::Conditional(_, left, right) => Self::signed(left) && Self::signed(right),
            Expr::Sign(signed, _) => *signed,
            Expr::Size(_, operand) => Self::signed(operand),
            _ => false,
        }
    }

    fn own(&self, expression: &Expr) -> Bits {
        self.eval(
            expression,
            Self::width(expression),
            Self::signed(expression),
        )
    }

    /// The numeric value of a known index, when it fits.
    fn index(&self, expression: &Expr) -> Option<Option<i128>> {
        let bits = self.own(expression);
        if has_unknown(&bits) {
            return None;
        }
        let signed = Self::signed(expression);
        let fill = u8::from(negative(&bits, signed));
        if bits.iter().skip(127).any(|bit| *bit != fill) {
            return Some(None);
        }
        let mut value: i128 = if fill == 1 { -1 } else { 0 };
        for bit in bits.iter().take(127).rev() {
            value = (value << 1) | i128::from(*bit);
        }
        Some(Some(value))
    }

    /// Bits `start..start + width` of a variable; a bit outside the variable
    /// or selected by an unknown index reads x (11.5.1), and 0 from a
    /// two-state vector (S07-D7).
    fn select(&self, variable: usize, start: Option<Option<i128>>, width: u32) -> Bits {
        let value = &self.values[variable];
        let missing = if VARS[variable].four_state { X } else { 0 };
        (0..i128::from(width))
            .map(|offset| match start {
                Some(Some(start)) => usize::try_from(start + offset)
                    .ok()
                    .and_then(|position| value.get(position).copied())
                    .unwrap_or(missing),
                _ => missing,
            })
            .collect()
    }

    /// The value of `expression` in a context of `width` bits whose
    /// propagated type is `signed`.
    fn eval(&self, expression: &Expr, width: u32, signed: bool) -> Bits {
        let operand = |bits: Bits, own_signed: bool| resize(bits, width, signed && own_signed);
        let bit = |value: u8| resize(vec![value], width, false);
        match expression {
            Expr::Var(index) => operand(self.values[*index].clone(), VARS[*index].signed),
            Expr::Literal {
                bits,
                signed: literal_signed,
                ..
            } => operand(bits.clone(), *literal_signed),
            Expr::Fill(value) => vec![*value; width as usize],
            Expr::Unary(operator, inner) => match operator {
                Unary::Plus => self.eval(inner, width, signed),
                Unary::Minus => {
                    let value = self.eval(inner, width, signed);
                    if has_unknown(&value) {
                        unknown(width)
                    } else {
                        negate(&value)
                    }
                }
                Unary::Not => self
                    .eval(inner, width, signed)
                    .iter()
                    .map(|bit| if *bit >= X { X } else { 1 - bit })
                    .collect(),
                Unary::LogicalNot => bit(match truth(&self.own(inner)) {
                    0 => 1,
                    1 => 0,
                    _ => X,
                }),
                reduction => {
                    let value = self.own(inner);
                    let ones = value.iter().filter(|bit| **bit == 1).count();
                    let unknown = has_unknown(&value);
                    let invert = |bit: u8| if bit == X { X } else { 1 - bit };
                    let and = if value.contains(&0) {
                        0
                    } else if unknown {
                        X
                    } else {
                        1
                    };
                    let or = if ones > 0 {
                        1
                    } else if unknown {
                        X
                    } else {
                        0
                    };
                    let xor = if unknown { X } else { (ones % 2) as u8 };
                    bit(match reduction {
                        Unary::And => and,
                        Unary::Nand => invert(and),
                        Unary::Or => or,
                        Unary::Nor => invert(or),
                        Unary::Xor => xor,
                        _ => invert(xor),
                    })
                }
            },
            Expr::Binary(operator, left, right) => {
                self.binary(*operator, left, right, width, signed)
            }
            Expr::Conditional(condition, left, right) => {
                let left = self.eval(left, width, signed);
                let right = self.eval(right, width, signed);
                match truth(&self.own(condition)) {
                    1 => left,
                    0 => right,
                    // 11.4.11: the arms combine bit by bit.
                    _ => left
                        .iter()
                        .zip(&right)
                        .map(|(a, b)| if a == b && *a < X { *a } else { X })
                        .collect(),
                }
            }
            Expr::Concat(parts) => {
                let mut bits = Bits::new();
                for part in parts.iter().rev() {
                    bits.extend(self.own(part));
                }
                operand(bits, false)
            }
            Expr::Replicate(count, inner) => {
                let part = self.own(inner);
                operand(part.repeat(*count as usize), false)
            }
            Expr::Part(variable, high, low) => operand(
                self.values[*variable][*low as usize..=*high as usize].to_vec(),
                false,
            ),
            Expr::Bit(variable, index) => {
                operand(self.select(*variable, self.index(index), 1), false)
            }
            Expr::Indexed(variable, index, part) => {
                operand(self.select(*variable, self.index(index), *part), false)
            }
            Expr::Sign(own_signed, inner) => operand(self.own(inner), *own_signed),
            Expr::Size(size, inner) => {
                // 6.24.1: the value a vector of that size would hold after
                // being assigned the expression.
                let inner_signed = Self::signed(inner);
                let value = self.eval(inner, Self::width(inner).max(*size), inner_signed);
                operand(resize(value, *size, false), inner_signed)
            }
        }
    }

    fn binary(
        &self,
        operator: Binary,
        left: &Expr,
        right: &Expr,
        width: u32,
        signed: bool,
    ) -> Bits {
        let bit = |value: u8| resize(vec![value], width, false);
        match operator {
            Binary::Add | Binary::Sub | Binary::Mul | Binary::Div | Binary::Mod => {
                let a = self.eval(left, width, signed);
                let b = self.eval(right, width, signed);
                // 11.4.2: an unknown operand bit makes the whole result x.
                if has_unknown(&a) || has_unknown(&b) {
                    return unknown(width);
                }
                match operator {
                    Binary::Add => add(&a, &b),
                    Binary::Sub => add(&a, &negate(&b)),
                    Binary::Mul => multiply(&a, &b),
                    _ if is_zero(&b) => unknown(width),
                    _ => {
                        let magnitude = |value: &Bits| {
                            if negative(value, signed) {
                                negate(value)
                            } else {
                                value.clone()
                            }
                        };
                        let (quotient, remainder) = divide(&magnitude(&a), &magnitude(&b));
                        // The quotient truncates toward zero and the
                        // remainder takes the sign of the first operand.
                        if operator == Binary::Div {
                            if negative(&a, signed) != negative(&b, signed) {
                                negate(&quotient)
                            } else {
                                quotient
                            }
                        } else if negative(&a, signed) {
                            negate(&remainder)
                        } else {
                            remainder
                        }
                    }
                }
            }
            Binary::Pow => {
                let base = self.eval(left, width, signed);
                let exponent = self.own(right);
                if has_unknown(&base) || has_unknown(&exponent) {
                    return unknown(width);
                }
                let one = number(1, width);
                if is_zero(&exponent) {
                    return one;
                }
                // Table 11-4.
                if negative(&exponent, Self::signed(right)) {
                    return if is_zero(&base) {
                        unknown(width)
                    } else if base == one {
                        one
                    } else if signed && !base.contains(&0) {
                        if exponent[0] == 1 {
                            base
                        } else {
                            one
                        }
                    } else {
                        number(0, width)
                    };
                }
                let mut result = one;
                for bit in exponent.iter().rev() {
                    result = multiply(&result, &result);
                    if *bit == 1 {
                        result = multiply(&result, &base);
                    }
                }
                result
            }
            Binary::BitAnd | Binary::BitOr | Binary::BitXor | Binary::BitXnor => {
                let a = self.eval(left, width, signed);
                let b = self.eval(right, width, signed);
                a.iter()
                    .zip(&b)
                    .map(|(a, b)| {
                        let (a, b) = ((*a).min(X), (*b).min(X));
                        match operator {
                            Binary::BitAnd if a == 0 || b == 0 => 0,
                            Binary::BitOr if a == 1 || b == 1 => 1,
                            _ if a == X || b == X => X,
                            Binary::BitAnd => 1,
                            Binary::BitOr => 0,
                            Binary::BitXor => a ^ b,
                            _ => 1 - (a ^ b),
                        }
                    })
                    .collect()
            }
            Binary::LogicalAnd | Binary::LogicalOr => {
                let a = truth(&self.own(left));
                let b = truth(&self.own(right));
                let decisive = u8::from(operator == Binary::LogicalOr);
                bit(if a == decisive || b == decisive {
                    decisive
                } else if a == X || b == X {
                    X
                } else {
                    1 - decisive
                })
            }
            Binary::Shl | Binary::Shr | Binary::AShl | Binary::AShr => {
                let value = self.eval(left, width, signed);
                let amount = self.own(right);
                if has_unknown(&amount) {
                    return unknown(width);
                }
                // The right operand is an unsigned number (11.4.10).
                let distance = amount
                    .iter()
                    .enumerate()
                    .filter(|(_, bit)| **bit == 1)
                    .fold(0usize, |sum, (position, _)| {
                        sum.saturating_add(
                            1usize.checked_shl(position as u32).unwrap_or(usize::MAX),
                        )
                    })
                    .min(width as usize);
                let size = width as usize;
                match operator {
                    Binary::Shl | Binary::AShl => {
                        let mut shifted = vec![0; distance];
                        shifted.extend_from_slice(&value[..size - distance]);
                        shifted
                    }
                    _ => {
                        let fill = if operator == Binary::AShr && signed {
                            value[size - 1]
                        } else {
                            0
                        };
                        let mut shifted = value[distance..].to_vec();
                        shifted.resize(size, fill);
                        shifted
                    }
                }
            }
            comparison => {
                // The operands size each other and compare as signed only
                // when both are signed.
                let size = Self::width(left).max(Self::width(right));
                let both_signed = Self::signed(left) && Self::signed(right);
                let a = self.eval(left, size, both_signed);
                let b = self.eval(right, size, both_signed);
                let invert = |bit: u8| if bit == X { X } else { 1 - bit };
                bit(match comparison {
                    Binary::CaseEq => u8::from(a == b),
                    Binary::CaseNe => u8::from(a != b),
                    Binary::Eq | Binary::Ne | Binary::WildEq | Binary::WildNe => {
                        // 11.4.5, 11.4.6: a pair of known bits that differ
                        // decides the result; otherwise an unknown bit makes
                        // it ambiguous. An x or z of the right operand of
                        // the wildcard operators matches anything.
                        let wild = matches!(comparison, Binary::WildEq | Binary::WildNe);
                        let mut equal = 1;
                        for (a, b) in a.iter().zip(&b) {
                            if wild && *b >= X {
                                continue;
                            }
                            if *a >= X || *b >= X {
                                if equal == 1 {
                                    equal = X;
                                }
                            } else if a != b {
                                equal = 0;
                            }
                        }
                        if matches!(comparison, Binary::Eq | Binary::WildEq) {
                            equal
                        } else {
                            invert(equal)
                        }
                    }
                    _ if has_unknown(&a) || has_unknown(&b) => X,
                    _ => {
                        let less = |a: &Bits, b: &Bits| match (
                            negative(a, both_signed),
                            negative(b, both_signed),
                        ) {
                            (true, false) => true,
                            (false, true) => false,
                            _ => below(a, b),
                        };
                        u8::from(match comparison {
                            Binary::Lt => less(&a, &b),
                            Binary::Le => !less(&b, &a),
                            Binary::Gt => less(&b, &a),
                            _ => !less(&a, &b),
                        })
                    }
                })
            }
        }
    }
}

fn binary_digits(bits: &[u8]) -> String {
    bits.iter()
        .rev()
        .map(|bit| match *bit {
            0 => '0',
            1 => '1',
            X => 'x',
            _ => 'z',
        })
        .collect()
}

fn spelled_literal(bits: &[u8], signed: bool) -> String {
    format!(
        "{}'{}b{}",
        bits.len(),
        if signed { "s" } else { "" },
        binary_digits(bits)
    )
}

/// SystemVerilog text of `expression`; `name` spells an operand.
fn spell(expression: &Expr, name: &dyn Fn(usize) -> String) -> String {
    let sub = |expression: &Expr| spell(expression, name);
    match expression {
        Expr::Var(index) => name(*index),
        Expr::Literal { text, .. } => text.clone(),
        Expr::Fill(value) => format!("'{}", ['0', '1', 'x', 'z'][*value as usize]),
        Expr::Unary(operator, inner) => {
            let operator = match operator {
                Unary::Plus => "+",
                Unary::Minus => "-",
                Unary::Not => "~",
                Unary::LogicalNot => "!",
                Unary::And => "&",
                Unary::Nand => "~&",
                Unary::Or => "|",
                Unary::Nor => "~|",
                Unary::Xor => "^",
                Unary::Xnor => "~^",
            };
            format!("({operator}({}))", sub(inner))
        }
        Expr::Binary(operator, left, right) => {
            let operator = match operator {
                Binary::Add => "+",
                Binary::Sub => "-",
                Binary::Mul => "*",
                Binary::Div => "/",
                Binary::Mod => "%",
                Binary::Pow => "**",
                Binary::BitAnd => "&",
                Binary::BitOr => "|",
                Binary::BitXor => "^",
                Binary::BitXnor => "~^",
                Binary::Eq => "==",
                Binary::Ne => "!=",
                Binary::CaseEq => "===",
                Binary::CaseNe => "!==",
                Binary::WildEq => "==?",
                Binary::WildNe => "!=?",
                Binary::Lt => "<",
                Binary::Le => "<=",
                Binary::Gt => ">",
                Binary::Ge => ">=",
                Binary::LogicalAnd => "&&",
                Binary::LogicalOr => "||",
                Binary::Shl => "<<",
                Binary::Shr => ">>",
                Binary::AShl => "<<<",
                Binary::AShr => ">>>",
            };
            format!("({} {operator} {})", sub(left), sub(right))
        }
        Expr::Conditional(condition, left, right) => {
            format!("({} ? {} : {})", sub(condition), sub(left), sub(right))
        }
        Expr::Concat(parts) => {
            let parts: Vec<String> = parts.iter().map(sub).collect();
            format!("{{{}}}", parts.join(", "))
        }
        Expr::Replicate(count, inner) => format!("{{{count}{{{}}}}}", sub(inner)),
        Expr::Part(variable, high, low) => format!("{}[{high}:{low}]", name(*variable)),
        Expr::Bit(variable, index) => format!("{}[{}]", name(*variable), sub(index)),
        Expr::Indexed(variable, index, width) => {
            format!("{}[{} +: {width}]", name(*variable), sub(index))
        }
        Expr::Sign(true, inner) => format!("$signed({})", sub(inner)),
        Expr::Sign(false, inner) => format!("$unsigned({})", sub(inner)),
        Expr::Size(size, inner) => format!("{size}'({})", sub(inner)),
    }
}

fn operands(expression: &Expr, used: &mut Vec<usize>) {
    match expression {
        Expr::Var(index) | Expr::Part(index, ..) => used.push(*index),
        Expr::Literal { .. } | Expr::Fill(_) => {}
        Expr::Unary(_, inner)
        | Expr::Replicate(_, inner)
        | Expr::Sign(_, inner)
        | Expr::Size(_, inner) => operands(inner, used),
        Expr::Binary(_, left, right) => {
            operands(left, used);
            operands(right, used);
        }
        Expr::Conditional(condition, left, right) => {
            operands(condition, used);
            operands(left, used);
            operands(right, used);
        }
        Expr::Concat(parts) => parts.iter().for_each(|part| operands(part, used)),
        Expr::Bit(variable, index) | Expr::Indexed(variable, index, _) => {
            used.push(*variable);
            operands(index, used);
        }
    }
}

struct Generator {
    fuzz: Fuzz,
    /// Parameters instead of variables: a selected position must be valid,
    /// because a constant index outside the vector is an error.
    constant: bool,
}

impl Generator {
    fn value(&mut self, width: u32, four_state: bool) -> Bits {
        let width_usize = width as usize;
        let mut bits = match self.fuzz.below(8) {
            0 => number(0, width),
            1 => number(1, width),
            2 => vec![1; width_usize],
            3 => {
                // The most negative value of a signed operand.
                let mut bits = number(0, width);
                bits[width_usize - 1] = 1;
                bits
            }
            4 => {
                let mut bits = vec![1; width_usize];
                bits[width_usize - 1] = 0;
                bits
            }
            5 => number(u128::from(self.fuzz.below(16)), width),
            _ => (0..width).map(|_| self.fuzz.below(2) as u8).collect(),
        };
        if four_state && self.fuzz.chance(1, 4) {
            if self.fuzz.chance(1, 5) {
                return vec![*self.fuzz.pick(&[X, Z]); width_usize];
            }
            for _ in 0..=self.fuzz.below(3) {
                let at = self.fuzz.index(width_usize);
                bits[at] = *self.fuzz.pick(&[X, X, Z]);
            }
        }
        bits
    }

    fn literal(&mut self, sized: bool) -> Expr {
        if !sized && self.fuzz.chance(1, 3) {
            let value = match self.fuzz.below(4) {
                0 => self.fuzz.below(4),
                1 => self.fuzz.below(70),
                2 => 0x7fff_ffff,
                _ => self.fuzz.below(1 << 31),
            };
            let (text, signed) = match self.fuzz.below(3) {
                0 => (format!("'d{value}"), false),
                1 => (format!("'sh{value:x}"), true),
                _ => (value.to_string(), true),
            };
            return Expr::Literal {
                text,
                bits: number(u128::from(value), 32),
                signed,
            };
        }
        let width = match self.fuzz.below(4) {
            0 => self.fuzz.range(1, 8),
            1 => *self.fuzz.pick(&[16, 32, 33, 64, 65]),
            _ => self.fuzz.range(1, 72),
        } as u32;
        let bits = self.value(width, true);
        let signed = self.fuzz.chance(1, 3);
        Expr::Literal {
            text: spelled_literal(&bits, signed),
            bits,
            signed,
        }
    }

    fn variable(&mut self) -> usize {
        self.fuzz.index(VARS.len())
    }

    /// An index operand: any variable, or a valid literal position.
    fn position(&mut self, limit: u32) -> Expr {
        if self.constant || self.fuzz.chance(1, 4) {
            let bits = number(u128::from(self.fuzz.below(u64::from(limit))), 9);
            return Expr::Literal {
                text: spelled_literal(&bits, false),
                bits,
                signed: false,
            };
        }
        Expr::Var(self.variable())
    }

    /// An expression of at most `depth` operator levels. `sized` excludes
    /// the operands a concatenation may not hold (11.4.12).
    fn expression(&mut self, depth: u32, sized: bool) -> Expr {
        if depth == 0 || self.fuzz.chance(1, 6) {
            return match self.fuzz.below(10) {
                0..=4 => Expr::Var(self.variable()),
                5 | 6 => self.literal(sized),
                7 if !sized => Expr::Fill(self.fuzz.below(4) as u8),
                _ => {
                    let variable = self.variable();
                    let width = VARS[variable].width;
                    if width == 1 {
                        return Expr::Var(variable);
                    }
                    match self.fuzz.below(3) {
                        0 => {
                            let low = self.fuzz.below(u64::from(width)) as u32;
                            let high = self.fuzz.range(u64::from(low), u64::from(width - 1)) as u32;
                            Expr::Part(variable, high, low)
                        }
                        1 => Expr::Bit(variable, Box::new(self.position(width))),
                        _ => {
                            let part = self.fuzz.range(1, u64::from(width.min(40))) as u32;
                            let limit = width - part + 1;
                            Expr::Indexed(variable, Box::new(self.position(limit)), part)
                        }
                    }
                }
            };
        }
        let next = depth - 1;
        let expression = match self.fuzz.below(20) {
            0..=6 => {
                let operator = *self.fuzz.pick(&[
                    Binary::Add,
                    Binary::Add,
                    Binary::Sub,
                    Binary::Sub,
                    Binary::Mul,
                    Binary::Div,
                    Binary::Mod,
                    Binary::Pow,
                    Binary::BitAnd,
                    Binary::BitOr,
                    Binary::BitXor,
                    Binary::BitXnor,
                ]);
                let left = self.expression(next, false);
                // A sized exponent keeps the power within the corpus budget
                // of meaningful values; any width is still legal.
                let right = self.expression(next, false);
                Expr::Binary(operator, Box::new(left), Box::new(right))
            }
            7..=9 => {
                let operator = *self.fuzz.pick(&[
                    Binary::Eq,
                    Binary::Ne,
                    Binary::CaseEq,
                    Binary::CaseNe,
                    Binary::WildEq,
                    Binary::WildNe,
                    Binary::Lt,
                    Binary::Le,
                    Binary::Gt,
                    Binary::Ge,
                    Binary::LogicalAnd,
                    Binary::LogicalOr,
                ]);
                Expr::Binary(
                    operator,
                    Box::new(self.expression(next, false)),
                    Box::new(self.expression(next, false)),
                )
            }
            10 | 11 => {
                let operator =
                    *self
                        .fuzz
                        .pick(&[Binary::Shl, Binary::Shr, Binary::AShl, Binary::AShr]);
                let amount = if self.fuzz.chance(2, 3) {
                    let bits = number(u128::from(self.fuzz.below(140)), 8);
                    Expr::Literal {
                        text: spelled_literal(&bits, false),
                        bits,
                        signed: false,
                    }
                } else {
                    self.expression(next, false)
                };
                Expr::Binary(
                    operator,
                    Box::new(self.expression(next, false)),
                    Box::new(amount),
                )
            }
            12 | 13 => {
                let operator = *self.fuzz.pick(&[
                    Unary::Plus,
                    Unary::Minus,
                    Unary::Minus,
                    Unary::Not,
                    Unary::Not,
                    Unary::LogicalNot,
                    Unary::And,
                    Unary::Nand,
                    Unary::Or,
                    Unary::Nor,
                    Unary::Xor,
                    Unary::Xnor,
                ]);
                Expr::Unary(operator, Box::new(self.expression(next, false)))
            }
            14 | 15 => Expr::Conditional(
                Box::new(self.expression(next, false)),
                Box::new(self.expression(next, false)),
                Box::new(self.expression(next, false)),
            ),
            16 => {
                let count = self.fuzz.range(1, 3);
                Expr::Concat((0..count).map(|_| self.expression(next, true)).collect())
            }
            17 => Expr::Replicate(
                self.fuzz.range(1, 4) as u32,
                Box::new(self.expression(next, true)),
            ),
            18 => Expr::Sign(
                self.fuzz.chance(1, 2),
                Box::new(self.expression(next, true)),
            ),
            _ => Expr::Size(
                *self
                    .fuzz
                    .pick(&[1, 7, 8, 16, 32, 33, 64, 65, 100, 128, 150]),
                Box::new(self.expression(next, false)),
            ),
        };
        if Oracle::width(&expression) > MAX_WIDTH {
            return Expr::Var(self.variable());
        }
        expression
    }
}

/// How a statement uses its expression.
#[derive(Clone, Copy)]
enum Context {
    Assign(usize),
    Display,
    Condition,
    Actual(usize),
}

/// The value an assignment leaves in a target of `(width, _, four_state)`.
fn assigned(oracle: &Oracle, expression: &Expr, target: (u32, bool, bool)) -> Bits {
    let (width, _, four_state) = target;
    let signed = Oracle::signed(expression);
    let value = oracle.eval(expression, Oracle::width(expression).max(width), signed);
    resize(value, width, false)
        .into_iter()
        .map(|bit| {
            if four_state {
                bit
            } else {
                bit & u8::from(bit < X)
            }
        })
        .collect()
}

fn target_declaration(target: (u32, bool, bool)) -> String {
    let (width, signed, four_state) = target;
    format!(
        "{}{} [{}:0]",
        if four_state { "logic" } else { "bit" },
        if signed { " signed" } else { "" },
        width - 1
    )
}

struct Case {
    source: String,
    expected: String,
    /// Per expected line, a standalone module that prints that line alone.
    reproductions: Vec<String>,
}

fn generate(corpus: &str, case: u64, constant: bool) -> Case {
    let mut generator = Generator {
        fuzz: Fuzz::new(corpus, case),
        constant,
    };
    let mut source = String::from("module tb;\n");
    let mut expected = String::new();
    let mut reproductions = Vec::new();
    let variable = |index: usize| format!("v{index}");
    if !constant {
        for (index, var) in VARS.iter().enumerate() {
            writeln!(source, "  {} v{index};", var.declaration).unwrap();
        }
    }
    for (index, target) in TARGETS.iter().enumerate() {
        let declaration = target_declaration(*target);
        writeln!(source, "  {declaration} t{index};").unwrap();
        writeln!(
            source,
            "  function automatic logic [{}:0] f{index}(input {declaration} a);\n    return a;\n  endfunction",
            target.0 - 1
        )
        .unwrap();
    }
    let rounds: Vec<Vec<Bits>> = (0..ROUNDS)
        .map(|_| {
            VARS.iter()
                .map(|var| generator.value(var.width, var.four_state))
                .collect()
        })
        .collect();
    let assignments = |values: &[Bits], used: &[usize], indent: &str| {
        let mut text = String::new();
        for index in used {
            let var = &VARS[*index];
            writeln!(
                text,
                "{indent}v{index} = {};",
                spelled_literal(&values[*index], var.signed)
            )
            .unwrap();
        }
        text
    };
    let reproduction = |values: &[Bits], expression: &Expr, body: &str, wire: Option<&str>| {
        let mut used = Vec::new();
        operands(expression, &mut used);
        used.sort_unstable();
        used.dedup();
        let mut text = String::from("module tb;\n");
        for index in &used {
            writeln!(text, "  {} v{index};", VARS[*index].declaration).unwrap();
        }
        if let Some(wire) = wire {
            text.push_str(wire);
        }
        text.push_str("  initial begin\n");
        text.push_str(&assignments(values, &used, "    "));
        writeln!(text, "    #1;\n{body}    $finish;\n  end\nendmodule").unwrap();
        text
    };
    let wires: Vec<(Expr, usize)> = if constant {
        Vec::new()
    } else {
        (0..WIRES)
            .map(|_| {
                (
                    generator.expression(3, false),
                    generator.fuzz.index(TARGETS.len()),
                )
            })
            .collect()
    };
    for (index, (expression, target)) in wires.iter().enumerate() {
        let (width, signed, _) = TARGETS[*target];
        writeln!(
            source,
            "  wire{} [{}:0] w{index} = {};",
            if signed { " signed" } else { "" },
            width - 1,
            spell(expression, &variable)
        )
        .unwrap();
    }
    if !constant {
        source.push_str("  initial begin : stimulus\n");
        let all: Vec<usize> = (0..VARS.len()).collect();
        for (round, values) in rounds.iter().enumerate() {
            if round > 0 {
                source.push_str("    #2;\n");
            }
            source.push_str(&assignments(values, &all, "    "));
        }
        source.push_str("  end\n");
    }
    let mut parameters = String::new();
    let mut body = String::new();
    for (round, values) in rounds.iter().enumerate() {
        let oracle = Oracle { values };
        writeln!(body, "    #{};", if round == 0 { 1 } else { 2 }).unwrap();
        for (index, (expression, target)) in wires.iter().enumerate() {
            let label = format!("r{round}w{index}");
            let (width, signed, _) = TARGETS[*target];
            // A net has no two-state domain.
            let bits = assigned(&oracle, expression, (width, signed, true));
            writeln!(body, "    $display(\"{label} %b\", w{index});").unwrap();
            writeln!(expected, "{label} {}", binary_digits(&bits)).unwrap();
            reproductions.push(reproduction(
                values,
                expression,
                &format!("    $display(\"{label} %b\", w);\n"),
                Some(&format!(
                    "  wire{} [{}:0] w = {};\n",
                    if signed { " signed" } else { "" },
                    width - 1,
                    spell(expression, &variable)
                )),
            ));
        }
        for index in 0..EXPRESSIONS {
            let expression = generator.expression(4, false);
            let label = format!("r{round}e{index}");
            let operand = |index: usize| {
                if constant {
                    format!("P{round}_{index}")
                } else {
                    variable(index)
                }
            };
            let context = match generator.fuzz.below(8) {
                0..=3 => Context::Assign(generator.fuzz.index(TARGETS.len())),
                4 | 5 => Context::Display,
                6 => Context::Condition,
                _ => Context::Actual(generator.fuzz.index(TARGETS.len())),
            };
            let statement = |text: &str| match context {
                Context::Assign(target) => {
                    format!("    t{target} = {text};\n    $display(\"{label} %b\", t{target});\n")
                }
                Context::Display => format!("    $display(\"{label} %b\", {text});\n"),
                Context::Condition => format!(
                    "    if ({text}) $display(\"{label} 1\");\n    else $display(\"{label} 0\");\n"
                ),
                Context::Actual(target) => {
                    format!("    $display(\"{label} %b\", f{target}({text}));\n")
                }
            };
            let bits = match context {
                Context::Assign(target) | Context::Actual(target) => {
                    let (width, signed, four_state) = TARGETS[target];
                    let bits = assigned(&oracle, &expression, (width, signed, four_state));
                    // The function returns a four-state vector of the
                    // formal's value.
                    bits
                }
                Context::Display => oracle.own(&expression),
                // An unknown condition takes the else branch (12.4).
                Context::Condition => vec![u8::from(truth(&oracle.own(&expression)) == 1)],
            };
            body.push_str(&statement(&spell(&expression, &operand)));
            writeln!(expected, "{label} {}", binary_digits(&bits)).unwrap();
            let mut helpers = String::new();
            if let Context::Assign(target) | Context::Actual(target) = context {
                let declaration = target_declaration(TARGETS[target]);
                writeln!(helpers, "  {declaration} t{target};").unwrap();
                writeln!(
                    helpers,
                    "  function automatic logic [{}:0] f{target}(input {declaration} a);\n    return a;\n  endfunction",
                    TARGETS[target].0 - 1
                )
                .unwrap();
            }
            reproductions.push(reproduction(
                values,
                &expression,
                &statement(&spell(&expression, &variable)),
                Some(&helpers),
            ));
        }
        if constant {
            for (index, var) in VARS.iter().enumerate() {
                writeln!(
                    parameters,
                    "  localparam {} P{round}_{index} = {};",
                    var.declaration,
                    spelled_literal(&values[index], var.signed)
                )
                .unwrap();
            }
        }
    }
    source.push_str(&parameters);
    writeln!(
        source,
        "  initial begin : check\n{body}    $finish;\n  end\nendmodule"
    )
    .unwrap();
    Case {
        source,
        expected,
        reproductions,
    }
}

/// Generate case `case` of `corpus` and run it in both optimizer modes on
/// every value backend.
fn check(corpus: &str, case: u64, constant: bool) {
    let generated = generate(corpus, case, constant);
    let directory = sim_harness::TempDir::new("fin-002-expressions").expect("test directory");
    let source = directory.path().join("tb.sv");
    std::fs::write(&source, &generated.source).expect("write the generated module");
    for (lane, environment) in sim_cli::backend_matrix() {
        for optimized in [true, false] {
            let label = format!("{corpus} case {case}, {lane}, optimized={optimized}");
            let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
            command
                .current_dir(directory.path())
                .args(["--top", "tb", "--out-dir"])
                .arg(format!("build-{}-{optimized}", lane.replace('/', "-")));
            if !optimized {
                command.arg("--no-opt");
            }
            command.arg(&source);
            command.envs(environment.iter().map(|(name, value)| (*name, value)));
            let output = sim_harness::run_command(&mut command, RUN_TIMEOUT)
                .unwrap_or_else(|error| panic!("{label}: {error}"));
            let stderr: Vec<&str> = std::str::from_utf8(&output.stderr)
                .unwrap_or("<stderr is not UTF-8>")
                .lines()
                .filter(|line| !sim_harness::is_compile_report_line(line))
                .collect();
            let stderr = stderr.join("\n");
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{label}: {stderr}\n{}",
                generated.source
            );
            let mut actual = stdout.lines();
            for (line, expected) in generated.expected.lines().enumerate() {
                let actual = actual.next().unwrap_or("<no line>");
                assert!(
                    actual == expected,
                    "{label}: line {}\n  expected {expected}\n  printed  {actual}\nstandalone reproduction:\n{}",
                    line + 1,
                    generated.reproductions[line]
                );
            }
            assert_eq!(actual.next(), None, "{label}: more output than expected");
        }
    }
}

/// The oracle on values worked out by hand from the clauses, so that an
/// error in it cannot hide behind an equal error in the simulator.
#[test]
fn component_oracle_follows_the_operator_tables() {
    let literal = |text: &str| {
        let (width, digits) = text.split_once('\'').unwrap();
        let signed = digits.starts_with("sb");
        let bits: Bits = digits
            .trim_start_matches("sb")
            .trim_start_matches('b')
            .chars()
            .rev()
            .map(|digit| match digit {
                '0' => 0,
                '1' => 1,
                'x' => X,
                _ => Z,
            })
            .collect();
        assert_eq!(bits.len(), width.parse::<usize>().unwrap());
        Expr::Literal {
            text: text.to_owned(),
            bits,
            signed,
        }
    };
    let binary = |operator, left: &str, right: &str| {
        Expr::Binary(operator, Box::new(literal(left)), Box::new(literal(right)))
    };
    let oracle = Oracle { values: &[] };
    let own = |expression: &Expr| binary_digits(&oracle.own(expression));
    let wide = |expression: &Expr, width| {
        binary_digits(&oracle.eval(expression, width, Oracle::signed(expression)))
    };
    // 11.6.2: the carry of an addition survives in a wider context.
    assert_eq!(wide(&binary(Binary::Add, "4'b1111", "4'b0001"), 5), "10000");
    assert_eq!(own(&binary(Binary::Add, "4'b1111", "4'b0001")), "0000");
    // 11.8.1: one unsigned operand makes the operation unsigned.
    assert_eq!(
        wide(&binary(Binary::Add, "4'sb1111", "4'sb0001"), 6),
        "000000"
    );
    assert_eq!(
        wide(&binary(Binary::Add, "4'sb1111", "4'b0001"), 6),
        "010000"
    );
    // 11.4.2: truncation toward zero, remainder with the first sign.
    assert_eq!(own(&binary(Binary::Div, "4'sb1001", "4'sb0010")), "1101");
    assert_eq!(own(&binary(Binary::Mod, "4'sb1001", "4'sb0010")), "1111");
    assert_eq!(own(&binary(Binary::Mod, "4'sb0111", "4'sb1110")), "0001");
    assert_eq!(own(&binary(Binary::Div, "4'b1001", "4'b0000")), "xxxx");
    assert_eq!(
        own(&binary(Binary::Div, "8'b11111111", "8'b00000011")),
        "01010101"
    );
    assert_eq!(own(&binary(Binary::Mul, "4'b0x01", "4'b0001")), "xxxx");
    // Table 11-4.
    assert_eq!(own(&binary(Binary::Pow, "4'sb1110", "3'sb011")), "1000");
    assert_eq!(own(&binary(Binary::Pow, "4'sb1111", "3'sb101")), "1111");
    assert_eq!(own(&binary(Binary::Pow, "4'sb1111", "3'sb110")), "0001");
    assert_eq!(own(&binary(Binary::Pow, "4'sb0000", "3'sb111")), "xxxx");
    assert_eq!(own(&binary(Binary::Pow, "4'sb0011", "3'sb111")), "0000");
    assert_eq!(own(&binary(Binary::Pow, "4'b0011", "3'b010")), "1001");
    assert_eq!(own(&binary(Binary::Pow, "4'b0000", "3'b000")), "0001");
    // 11.4.10.
    assert_eq!(own(&binary(Binary::AShr, "4'sb1000", "2'b01")), "1100");
    assert_eq!(own(&binary(Binary::AShr, "4'b1000", "2'b01")), "0100");
    assert_eq!(own(&binary(Binary::Shl, "4'b1011", "3'b111")), "0000");
    assert_eq!(own(&binary(Binary::Shr, "4'b1011", "2'bx1")), "xxxx");
    // 11.4.4, 11.4.5, 11.4.6.
    assert_eq!(own(&binary(Binary::Lt, "4'sb1000", "4'sb0001")), "1");
    assert_eq!(own(&binary(Binary::Lt, "4'sb1000", "4'b0001")), "0");
    assert_eq!(own(&binary(Binary::Lt, "4'sb100x", "4'b0001")), "x");
    assert_eq!(own(&binary(Binary::Eq, "4'b1x00", "4'b1100")), "x");
    assert_eq!(own(&binary(Binary::Eq, "4'b1x00", "4'b0000")), "0");
    assert_eq!(own(&binary(Binary::CaseEq, "4'b1x0z", "4'b1x0z")), "1");
    assert_eq!(own(&binary(Binary::WildEq, "4'b1100", "4'b1x0z")), "1");
    assert_eq!(own(&binary(Binary::WildEq, "4'b1x00", "4'b1x00")), "1");
    assert_eq!(own(&binary(Binary::WildEq, "4'b1x00", "4'b1100")), "x");
    // 11.4.7, 11.4.8, 11.4.9.
    assert_eq!(own(&binary(Binary::LogicalAnd, "2'b0x", "2'b00")), "0");
    assert_eq!(own(&binary(Binary::LogicalAnd, "2'b0x", "2'b01")), "x");
    assert_eq!(own(&binary(Binary::LogicalOr, "2'b1x", "2'b00")), "1");
    assert_eq!(own(&binary(Binary::BitAnd, "4'b01xz", "4'b0000")), "0000");
    assert_eq!(own(&binary(Binary::BitOr, "4'b01xz", "4'b1111")), "1111");
    assert_eq!(own(&binary(Binary::BitXor, "4'b01xz", "4'b1111")), "10xx");
    assert_eq!(
        own(&Expr::Unary(Unary::And, Box::new(literal("3'b1x0")))),
        "0"
    );
    assert_eq!(
        own(&Expr::Unary(Unary::Or, Box::new(literal("3'b0x0")))),
        "x"
    );
    assert_eq!(
        own(&Expr::Unary(Unary::Xnor, Box::new(literal("3'b110")))),
        "1"
    );
    // 11.4.11: an unknown condition merges the arms.
    assert_eq!(
        own(&Expr::Conditional(
            Box::new(literal("1'bx")),
            Box::new(literal("4'b0101")),
            Box::new(literal("4'b0011"))
        )),
        "0xx1"
    );
    // 11.8.2: a concatenation is unsigned; a cast keeps the operand's sign.
    assert_eq!(
        wide(&Expr::Concat(vec![literal("2'sb11"), literal("1'b0")]), 5),
        "00110"
    );
    assert_eq!(
        wide(
            &Expr::Size(3, Box::new(binary(Binary::Add, "2'sb11", "2'sb11"))),
            5
        ),
        "11110"
    );
}

/// Variables hold the operands: run-time evaluation.
#[test]
fn random_expressions_of_variables_follow_the_clauses() {
    for case in crate::fuzz::cases(2) {
        check("expressions/variables", case, false);
    }
}

/// Parameters hold the operands: constant evaluation.
#[test]
fn random_expressions_of_parameters_follow_the_clauses() {
    for case in crate::fuzz::cases(1) {
        check("expressions/parameters", case, true);
    }
}
