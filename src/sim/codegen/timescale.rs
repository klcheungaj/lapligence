/// A Verilog time unit/precision pair in picoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Timescale {
    pub(super) unit_ps: u64,
    pub(super) precision_ps: u64,
}

impl Timescale {
    /// Default for a module without a `timescale directive (1ns/1ps).
    pub(super) const DEFAULT: Self = Self {
        unit_ps: 1_000,
        precision_ps: 1,
    };
}

/// Parse the first `` `timescale <unit>/<precision> `` directive.
pub(super) fn parse_timescale(text: &str) -> Option<Timescale> {
    let index = text.find("`timescale")?;
    let rest = &text[index + "`timescale".len()..];
    let (unit_ps, rest) = parse_value(skip_ws(rest))?;
    let rest = skip_ws(rest).strip_prefix('/')?;
    let (precision_ps, _) = parse_value(skip_ws(rest))?;
    Some(Timescale {
        unit_ps,
        precision_ps,
    })
}

fn parse_value(value: &str) -> Option<(u64, &str)> {
    let (digits, rest) = split_at_while(value, |character| character.is_ascii_digit());
    if digits.is_empty() {
        return None;
    }
    let multiplier: u64 = digits.parse().ok()?;
    let (unit, rest) = split_at_while(skip_ws(rest), |character| character.is_ascii_alphabetic());
    let base = match unit {
        "s" => 1_000_000_000_000,
        "ms" => 1_000_000_000,
        "us" => 1_000_000,
        "ns" => 1_000,
        "ps" | "fs" => 1,
        _ => return None,
    };
    multiplier.checked_mul(base).map(|value| (value, rest))
}

fn split_at_while(value: &str, predicate: impl Fn(char) -> bool) -> (&str, &str) {
    let end = value
        .find(|character| !predicate(character))
        .unwrap_or(value.len());
    (&value[..end], &value[end..])
}

fn skip_ws(value: &str) -> &str {
    value.trim_start_matches([' ', '\t'])
}

/// Fold a source-recovered procedural delay expression. Surelog v1.87 does
/// not expose these expressions through VPI, so this deliberately small
/// parser covers integer arithmetic and elaborated parameter identifiers.
pub(super) fn eval_delay_expression(
    expression: &str,
    mut parameter: impl FnMut(&str) -> Option<DelayValue>,
) -> Result<u64, String> {
    const MAX_DELAY_EXPRESSION_STEPS: usize = 256;
    let mut parser = DelayExprParser {
        input: expression.as_bytes(),
        offset: 0,
        parameter: &mut parameter,
        steps_left: MAX_DELAY_EXPRESSION_STEPS,
    };
    let value = parser.parse_binary(0)?;
    parser.skip_ws();
    if parser.offset != parser.input.len() {
        return Err(format!(
            "unsupported token in procedural delay expression `{expression}`"
        ));
    }
    value.to_delay_ticks().ok_or_else(|| {
        format!("procedural delay expression `{expression}` is negative or exceeds 64 bits")
    })
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DelayValue {
    raw: u128,
    width: u32,
    signed: bool,
    computed: bool,
}

impl DelayValue {
    pub(super) fn from_raw(raw: u128, width: u32, signed: bool) -> Option<Self> {
        (width > 0 && width <= 128).then(|| Self {
            raw: raw & width_mask(width),
            width,
            signed,
            computed: false,
        })
    }

    fn integer_literal(value: u128) -> Option<Self> {
        let significant = u128::BITS - value.leading_zeros();
        let width = 32.max(significant.saturating_add(1));
        Self::from_raw(value, width, true)
    }

    fn extend_raw(self, width: u32, signed_context: bool) -> u128 {
        if signed_context && self.signed && self.width < width && self.sign_bit() {
            self.raw | (width_mask(width) & !width_mask(self.width))
        } else {
            self.raw
        }
    }

    fn signed_value(self, width: u32) -> i128 {
        let raw = self.extend_raw(width, true);
        if width == 128 || raw & (1u128 << (width - 1)) == 0 {
            raw as i128
        } else {
            (raw | !width_mask(width)) as i128
        }
    }

    fn sign_bit(self) -> bool {
        self.raw & (1u128 << (self.width - 1)) != 0
    }

    fn to_delay_ticks(self) -> Option<u64> {
        if self.signed && self.sign_bit() {
            return None;
        }
        self.raw.try_into().ok()
    }
}

fn width_mask(width: u32) -> u128 {
    if width == 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}

struct DelayExprParser<'a, F> {
    input: &'a [u8],
    offset: usize,
    parameter: &'a mut F,
    steps_left: usize,
}

impl<F: FnMut(&str) -> Option<DelayValue>> DelayExprParser<'_, F> {
    fn parse_binary(&mut self, min_precedence: u8) -> Result<DelayValue, String> {
        self.take_step()?;
        let mut lhs = self.parse_unary()?;
        loop {
            self.skip_ws();
            let Some((operator, precedence)) = self.peek_binary() else {
                break;
            };
            if precedence < min_precedence {
                break;
            }
            self.offset += operator.len();
            let rhs = self.parse_binary(precedence + 1)?;
            lhs = Self::apply_binary(operator, lhs, rhs)?;
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<DelayValue, String> {
        self.take_step()?;
        self.skip_ws();
        if self.consume(b'+') {
            return self.parse_unary();
        }
        if self.consume(b'-') {
            let value = self.parse_unary()?;
            return DelayValue::from_raw((!value.raw).wrapping_add(1), value.width, value.signed)
                .map(|mut value| {
                    value.computed = true;
                    value
                })
                .ok_or_else(|| "invalid unary `-` delay expression".to_string());
        }
        if self.consume(b'~') {
            let value = self.parse_unary()?;
            return DelayValue::from_raw(!value.raw, value.width, value.signed)
                .map(|mut value| {
                    value.computed = true;
                    value
                })
                .ok_or_else(|| "invalid unary `~` delay expression".to_string());
        }
        if self.consume(b'(') {
            let value = self.parse_binary(0)?;
            self.skip_ws();
            if !self.consume(b')') {
                return Err("unclosed parenthesis in procedural delay expression".to_string());
            }
            return Ok(value);
        }
        if self.input.get(self.offset).is_some_and(u8::is_ascii_digit) {
            let start = self.offset;
            while self
                .input
                .get(self.offset)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
            {
                self.offset += 1;
            }
            let digits: String = self.input[start..self.offset]
                .iter()
                .filter(|byte| **byte != b'_')
                .map(|byte| char::from(*byte))
                .collect();
            let value = digits
                .parse::<u128>()
                .map_err(|_| "integer literal in procedural delay is too large".to_string())?;
            return DelayValue::integer_literal(value)
                .ok_or_else(|| "integer literal in procedural delay is too large".to_string());
        }
        if self
            .input
            .get(self.offset)
            .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(*byte, b'_' | b'$'))
        {
            let start = self.offset;
            while self
                .input
                .get(self.offset)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'$'))
            {
                self.offset += 1;
            }
            let name = std::str::from_utf8(&self.input[start..self.offset])
                .map_err(|_| "non-ASCII identifier in procedural delay".to_string())?;
            return (self.parameter)(name)
                .ok_or_else(|| format!("`{name}` is not a resolved integer parameter"));
        }
        Err("expected an integer or parameter in procedural delay expression".to_string())
    }

    fn apply_binary(
        operator: &str,
        lhs: DelayValue,
        rhs: DelayValue,
    ) -> Result<DelayValue, String> {
        // Verilog propagates an outer expression's width into nested
        // context-determined arithmetic. Folding a child eagerly can lose a
        // carry before a wider sibling is seen: with 4-bit A=15/B=1 and
        // 5-bit C=0, `(A+B)+C` is 16 rather than 0. Until the source recovery
        // path builds a complete expression tree, reject width transitions
        // instead of silently evaluating with host-language tree semantics.
        if !matches!(operator, "<<" | ">>") && lhs.width != rhs.width {
            return Err(format!(
                "mixed-width `{operator}` in procedural delay expression requires full Verilog context propagation"
            ));
        }
        if lhs.signed != rhs.signed && (lhs.computed || rhs.computed) {
            return Err(format!(
                "mixed signedness around a computed `{operator}` operand in procedural delay expression requires full Verilog context propagation"
            ));
        }
        let width = lhs.width.max(rhs.width);
        let signed = lhs.signed && rhs.signed;
        let left = lhs.extend_raw(width, signed);
        let right = rhs.extend_raw(width, signed);
        let raw = match operator {
            "+" => left.wrapping_add(right),
            "-" => left.wrapping_sub(right),
            "*" => left.wrapping_mul(right),
            "/" | "%" if right == 0 => {
                return Err(format!("division by zero in delay operation `{operator}`"))
            }
            "/" if signed => lhs
                .signed_value(width)
                .checked_div(rhs.signed_value(width))
                .map(|value| value as u128)
                .ok_or_else(|| "overflow in signed delay division".to_string())?,
            "%" if signed => lhs
                .signed_value(width)
                .checked_rem(rhs.signed_value(width))
                .map(|value| value as u128)
                .ok_or_else(|| "overflow in signed delay remainder".to_string())?,
            "/" => left / right,
            "%" => left % right,
            "&" => left & right,
            "^" => left ^ right,
            "|" => left | right,
            "<<" | ">>" => {
                let shift = rhs.to_delay_ticks().unwrap_or(u64::MAX);
                let raw = if shift >= u64::from(lhs.width) {
                    0
                } else if operator == "<<" {
                    lhs.raw << shift
                } else {
                    lhs.raw >> shift
                };
                return DelayValue::from_raw(raw, lhs.width, lhs.signed)
                    .map(|mut value| {
                        value.computed = true;
                        value
                    })
                    .ok_or_else(|| "invalid shift in delay expression".to_string());
            }
            _ => return Err(format!("unsupported delay operator `{operator}`")),
        };
        DelayValue::from_raw(raw, width, signed)
            .map(|mut value| {
                value.computed = true;
                value
            })
            .ok_or_else(|| format!("invalid operation `{operator}` in delay"))
    }

    fn take_step(&mut self) -> Result<(), String> {
        self.steps_left = self
            .steps_left
            .checked_sub(1)
            .ok_or_else(|| "procedural delay expression is too complex".to_string())?;
        Ok(())
    }

    fn peek_binary(&self) -> Option<(&'static str, u8)> {
        let rest = &self.input[self.offset..];
        for (operator, precedence) in [
            ("|", 1),
            ("^", 2),
            ("&", 3),
            ("<<", 4),
            (">>", 4),
            ("+", 5),
            ("-", 5),
            ("*", 6),
            ("/", 6),
            ("%", 6),
        ] {
            if rest.starts_with(operator.as_bytes()) {
                return Some((operator, precedence));
            }
        }
        None
    }

    fn skip_ws(&mut self) {
        while self
            .input
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.input.get(self.offset) == Some(&byte) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spaced_timescale() {
        assert_eq!(
            parse_timescale("module top;\n`timescale 10 ns / 100 ps\n"),
            Some(Timescale {
                unit_ps: 10_000,
                precision_ps: 100
            })
        );
    }

    #[test]
    fn rejects_overflowing_timescale() {
        assert_eq!(
            parse_timescale("`timescale 18446744073709551615s/1ps"),
            None
        );
    }

    #[test]
    fn folds_parameterized_delay_expression() {
        assert_eq!(
            eval_delay_expression("P * 2 + 1_000", |name| {
                (name == "P")
                    .then(|| DelayValue::from_raw(3, 32, true))
                    .flatten()
            }),
            Ok(1006)
        );
    }

    #[test]
    fn preserves_verilog_width_and_sign_rules() {
        assert_eq!(
            eval_delay_expression("P + 1", |name| {
                (name == "P")
                    .then(|| DelayValue::from_raw(0xffff_ffff, 32, false))
                    .flatten()
            }),
            Ok(0)
        );
        assert!(eval_delay_expression("N", |name| {
            (name == "N")
                .then(|| DelayValue::from_raw(0xff, 8, true))
                .flatten()
        })
        .unwrap_err()
        .contains("negative"));
    }

    #[test]
    fn rejects_nested_mixed_width_before_losing_carry() {
        let error = eval_delay_expression("(A + B) + C", |name| match name {
            "A" => DelayValue::from_raw(15, 4, false),
            "B" => DelayValue::from_raw(1, 4, false),
            "C" => DelayValue::from_raw(0, 5, false),
            _ => None,
        })
        .unwrap_err();
        assert!(
            error.contains("full Verilog context propagation"),
            "{error}"
        );
    }

    #[test]
    fn rejects_outer_signedness_reinterpretation_of_computed_child() {
        let error = eval_delay_expression("(A / B) + C", |name| match name {
            "A" => DelayValue::from_raw(8, 4, true),
            "B" => DelayValue::from_raw(2, 4, true),
            "C" => DelayValue::from_raw(0, 4, false),
            _ => None,
        })
        .unwrap_err();
        assert!(error.contains("mixed signedness"), "{error}");
        assert!(
            error.contains("full Verilog context propagation"),
            "{error}"
        );
    }

    #[test]
    fn bounds_expression_complexity() {
        let expression = "(".repeat(300) + "1" + &")".repeat(300);
        assert!(eval_delay_expression(&expression, |_| None)
            .unwrap_err()
            .contains("too complex"));
    }

    #[test]
    fn rejects_dynamic_delay_expression() {
        assert!(eval_delay_expression("signal + 1", |_| None)
            .unwrap_err()
            .contains("not a resolved integer parameter"));
    }
}
