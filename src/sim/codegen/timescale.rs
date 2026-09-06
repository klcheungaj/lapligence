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

/// A source-recovered procedural delay after constant folding but before it
/// is converted to the design-wide scheduler precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProceduralDelay {
    /// An integer expression measured in the calling module's time unit.
    ModuleUnitTicks(u64),
    /// A real or time literal already rounded to the calling module's time
    /// precision, measured in precision-sized quanta.
    ModulePrecisionTicks(u64),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum DelayParameter {
    Integer(DelayValue),
    Real(f64),
}

impl ProceduralDelay {
    pub(super) fn ticks_and_unit_ps(self, timescale: Timescale) -> (u64, u64) {
        match self {
            Self::ModuleUnitTicks(ticks) => (ticks, timescale.unit_ps),
            Self::ModulePrecisionTicks(ticks) => (ticks, timescale.precision_ps),
        }
    }
}

/// Fold a procedural delay, additionally recognizing the literal forms whose
/// value must be rounded to the calling module's precision before scheduling
/// (IEEE 1800-2009 §3.14.1 and §5.8).
pub(super) fn eval_procedural_delay(
    expression: &str,
    timescale: Timescale,
    mut parameter: impl FnMut(&str) -> Option<DelayParameter>,
) -> Result<ProceduralDelay, String> {
    let literal = strip_wrapping_parentheses(expression);
    if let Some(result) = parse_real_or_time_literal(literal, timescale) {
        return result.map(ProceduralDelay::ModulePrecisionTicks);
    }
    if let Some(DelayParameter::Real(value)) = parameter(literal) {
        return real_delay_ticks(value, timescale).map(ProceduralDelay::ModulePrecisionTicks);
    }
    eval_delay_expression(expression, |name| match parameter(name) {
        Some(DelayParameter::Integer(value)) => Some(value),
        _ => None,
    })
    .map(ProceduralDelay::ModuleUnitTicks)
}

/// Convert a resolved real delay in module units to local precision ticks.
fn real_delay_ticks(value: f64, timescale: Timescale) -> Result<u64, String> {
    if !value.is_finite() || value < 0.0 {
        return Err("real parameter delay must be finite and nonnegative".to_string());
    }
    if timescale.unit_ps == 0 || timescale.precision_ps == 0 {
        return Err("delay requires a nonzero time unit and precision".to_string());
    }
    let ticks = (value * (timescale.unit_ps as f64 / timescale.precision_ps as f64)).round();
    // u64::MAX rounds to 2^64 as an f64, so the upper bound is exclusive.
    if !ticks.is_finite() || ticks >= 18_446_744_073_709_551_616.0 {
        return Err("rounded real parameter delay exceeds the 64-bit tick range".to_string());
    }
    Ok(ticks as u64)
}

/// Interpret a unit-suffixed literal as a realtime value in module units,
/// rounded first to local precision (IEEE 1800-2009 §5.8).
pub(super) fn time_literal_to_real(
    literal: &str,
    timescale: Timescale,
) -> Result<Option<f64>, String> {
    if !["fs", "ps", "ns", "us", "ms", "s"]
        .iter()
        .any(|suffix| literal.ends_with(suffix))
    {
        return Ok(None);
    }
    parse_real_or_time_literal(literal, timescale)
        .transpose()
        .map(|ticks| {
            ticks.map(|ticks| {
                ticks as f64 * timescale.precision_ps as f64 / timescale.unit_ps as f64
            })
        })
}

fn strip_wrapping_parentheses(mut expression: &str) -> &str {
    for _ in 0..256 {
        expression = expression.trim();
        let bytes = expression.as_bytes();
        if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
            return expression;
        }
        let mut depth = 0usize;
        let mut closes_at_end = false;
        for (index, byte) in bytes.iter().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    let Some(next) = depth.checked_sub(1) else {
                        return expression;
                    };
                    depth = next;
                    if depth == 0 {
                        closes_at_end = index + 1 == bytes.len();
                        break;
                    }
                }
                _ => {}
            }
        }
        if !closes_at_end {
            return expression;
        }
        expression = &expression[1..expression.len() - 1];
    }
    expression
}

/// Parse a real delay, optionally followed immediately by an explicit
/// time unit. Plain integers are left to the integer-expression evaluator.
fn parse_real_or_time_literal(
    expression: &str,
    timescale: Timescale,
) -> Option<Result<u64, String>> {
    let expression = expression.trim();
    let bytes = expression.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let mut offset = 0usize;
    let mut digits = String::new();
    while let Some(byte) = bytes.get(offset) {
        if byte.is_ascii_digit() {
            digits.push(char::from(*byte));
            offset += 1;
        } else if *byte == b'_' {
            offset += 1;
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }

    let mut fractional_digits = 0usize;
    let fixed_point = bytes.get(offset) == Some(&b'.');
    if fixed_point {
        offset += 1;
        let fraction_start = digits.len();
        while let Some(byte) = bytes.get(offset) {
            if byte.is_ascii_digit() {
                digits.push(char::from(*byte));
                offset += 1;
            } else if *byte == b'_' {
                offset += 1;
            } else {
                break;
            }
        }
        fractional_digits = digits.len() - fraction_start;
        if fractional_digits == 0 {
            return Some(Err(
                "fractional procedural delay requires digits after `.`".to_string()
            ));
        }
    }

    let scientific = matches!(bytes.get(offset), Some(b'e' | b'E'));
    let mut exponent = 0i32;
    if scientific {
        offset += 1;
        let negative = bytes.get(offset) == Some(&b'-');
        if matches!(bytes.get(offset), Some(b'+' | b'-')) {
            offset += 1;
        }
        let start = offset;
        while let Some(byte) = bytes.get(offset) {
            if *byte == b'_' && offset > start {
                offset += 1;
                continue;
            }
            if !byte.is_ascii_digit() {
                break;
            }
            exponent = match exponent
                .checked_mul(10)
                .and_then(|n| n.checked_add(i32::from(*byte - b'0')))
            {
                Some(n) if n <= 38 => n,
                _ => {
                    return Some(Err(
                        "scientific delay exponent exceeds supported decimal precision".to_string(),
                    ))
                }
            };
            offset += 1;
        }
        if offset == start {
            return Some(Err("scientific delay requires exponent digits".to_string()));
        }
        if negative {
            exponent = -exponent;
        }
    }

    let suffix = &expression[offset..];
    // §5.8 permits integer/fixed-point time literals, not exponent notation.
    if scientific && !suffix.is_empty() {
        return None;
    }
    let explicit_unit_fs = match suffix {
        "" => None,
        "s" => Some(1_000_000_000_000_000u128),
        "ms" => Some(1_000_000_000_000u128),
        "us" => Some(1_000_000_000u128),
        "ns" => Some(1_000_000u128),
        "ps" => Some(1_000u128),
        "fs" => Some(1u128),
        _ => return None,
    };
    if !fixed_point && !scientific && explicit_unit_fs.is_none() {
        return None;
    }

    Some((|| {
        if timescale.unit_ps == 0 || timescale.precision_ps == 0 {
            return Err("delay requires a nonzero time unit and precision".to_string());
        }
        let mut numerator = digits.parse::<u128>().map_err(|_| {
            "real/time literal in procedural delay exceeds 128-bit precision".to_string()
        })?;
        if numerator == 0 {
            return Ok(0);
        }
        let decimal_scale = i32::try_from(fractional_digits)
            .ok()
            .and_then(|scale| scale.checked_sub(exponent))
            .ok_or_else(|| "delay literal exceeds supported decimal precision".to_string())?;
        if decimal_scale < 0 {
            numerator = numerator
                .checked_mul(
                    10u128
                        .checked_pow(decimal_scale.unsigned_abs())
                        .ok_or_else(|| {
                            "delay literal exceeds supported decimal precision".to_string()
                        })?,
                )
                .ok_or_else(|| "delay literal exceeds supported decimal precision".to_string())?;
        }
        let denominator = (0..decimal_scale.max(0)).try_fold(1u128, |value, _| {
            value.checked_mul(10).ok_or_else(|| {
                "real/time literal in procedural delay exceeds 128-bit fractional precision"
                    .to_string()
            })
        })?;
        let mut literal_unit_fs = explicit_unit_fs.unwrap_or(u128::from(timescale.unit_ps) * 1_000);
        // The scheduler's current timescale representation clamps fs units to
        // one ps, so the same effective precision is used for local rounding.
        let mut precision_fs = u128::from(timescale.precision_ps) * 1_000;
        // Cancel common units before multiplying so tiny scientific values
        // remain representable even when their unreduced denominator is large.
        let common_unit = greatest_common_divisor(literal_unit_fs, precision_fs);
        literal_unit_fs /= common_unit;
        precision_fs /= common_unit;
        let physical_numerator = numerator.checked_mul(literal_unit_fs).ok_or_else(|| {
            "real/time literal in procedural delay exceeds the supported range".to_string()
        })?;
        let precision_denominator = denominator.checked_mul(precision_fs).ok_or_else(|| {
            "real/time literal in procedural delay exceeds the supported range".to_string()
        })?;
        let whole = physical_numerator / precision_denominator;
        let remainder = physical_numerator % precision_denominator;
        // All supported literals are nonnegative. Round to nearest, with an
        // exact half rounded up (away from zero), without floating-point loss.
        let rounded = if remainder >= precision_denominator - remainder {
            whole.checked_add(1)
        } else {
            Some(whole)
        }
        .ok_or_else(|| "rounded procedural delay exceeds the supported range".to_string())?;
        u64::try_from(rounded)
            .map_err(|_| "rounded procedural delay exceeds the 64-bit tick range".to_string())
    })())
}

fn greatest_common_divisor(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
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
    fn rounds_real_and_time_literals_to_local_precision() {
        let timescale = Timescale {
            unit_ps: 1_000,
            precision_ps: 100,
        };
        assert_eq!(
            eval_procedural_delay("2.75", timescale, |_| None),
            Ok(ProceduralDelay::ModulePrecisionTicks(28))
        );
        assert_eq!(
            eval_procedural_delay("2.15ns", timescale, |_| None),
            Ok(ProceduralDelay::ModulePrecisionTicks(22))
        );
        assert_eq!(
            eval_procedural_delay("((2.15ns))", timescale, |_| None),
            Ok(ProceduralDelay::ModulePrecisionTicks(22))
        );
        assert_eq!(
            eval_procedural_delay("50ps", timescale, |_| None),
            Ok(ProceduralDelay::ModulePrecisionTicks(1))
        );
    }

    #[test]
    fn scales_every_time_literal_suffix_and_rounds_fs_threshold() {
        let timescale = Timescale {
            unit_ps: 1_000,
            precision_ps: 1,
        };
        for (literal, expected) in [
            ("499fs", 0),
            ("500fs", 1),
            ("1ps", 1),
            ("2ns", 2_000),
            ("3us", 3_000_000),
            ("4ms", 4_000_000_000),
            ("5s", 5_000_000_000_000),
        ] {
            assert_eq!(
                eval_procedural_delay(literal, timescale, |_| None),
                Ok(ProceduralDelay::ModulePrecisionTicks(expected)),
                "literal {literal}"
            );
        }
    }

    #[test]
    fn keeps_integer_expressions_in_module_units() {
        assert_eq!(
            eval_procedural_delay("1_000", Timescale::DEFAULT, |_| None),
            Ok(ProceduralDelay::ModuleUnitTicks(1_000))
        );
        assert!(
            eval_procedural_delay("1.0 + 1.0", Timescale::DEFAULT, |_| None)
                .unwrap_err()
                .contains("unsupported token")
        );
    }

    #[test]
    fn scientific_literals_use_exact_decimal_rounding() {
        for (literal, ticks) in [
            ("1e-3", 1),
            ("1.25e-1", 125),
            ("2E+1", 20_000),
            ("1e0_1", 10_000),
            ("0e-38", 0),
            ("1e-38", 0),
        ] {
            assert_eq!(
                eval_procedural_delay(literal, Timescale::DEFAULT, |_| None),
                Ok(ProceduralDelay::ModulePrecisionTicks(ticks)),
                "{literal}"
            );
        }
        for literal in ["1e", "1e+", "1e9999999999999999", "1e-999999999", "1e2ns"] {
            assert!(
                eval_procedural_delay(literal, Timescale::DEFAULT, |_| None).is_err(),
                "{literal}"
            );
        }
    }

    #[test]
    fn time_values_are_local_precision_rounded_reals() {
        let timescale = Timescale {
            unit_ps: 1_000,
            precision_ps: 100,
        };
        assert_eq!(time_literal_to_real("2.15ns", timescale), Ok(Some(2.2)));
        assert_eq!(time_literal_to_real("40ps", timescale), Ok(Some(0.0)));
        assert_eq!(time_literal_to_real("50ps", timescale), Ok(Some(0.1)));
        assert_eq!(time_literal_to_real("1.25", timescale), Ok(None));
        assert_eq!(time_literal_to_real("2.1ns + 1ns", timescale), Ok(None));
    }

    #[test]
    fn real_parameter_delays_check_rounding_and_bounds() {
        let timescale = Timescale {
            unit_ps: 1_000,
            precision_ps: 100,
        };
        assert_eq!(
            eval_procedural_delay("((P))", timescale, |name| {
                (name == "P").then_some(DelayParameter::Real(0.25))
            }),
            Ok(ProceduralDelay::ModulePrecisionTicks(3))
        );
        for name in ["_1e3", "_1s"] {
            assert_eq!(
                eval_procedural_delay(name, timescale, |_| Some(DelayParameter::Real(0.25))),
                Ok(ProceduralDelay::ModulePrecisionTicks(3))
            );
        }
        for value in [-0.1, f64::NAN, f64::INFINITY, 18_446_744_073_709_551_616.0] {
            assert!(real_delay_ticks(value, timescale).is_err(), "{value}");
        }
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
