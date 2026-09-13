/// A Verilog time unit/precision pair in femtoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Timescale {
    pub(super) unit_fs: u64,
    pub(super) precision_fs: u64,
}

impl Timescale {
    /// Default for a module without an explicit time scale (1ns/1ps).
    pub(super) const DEFAULT: Self = Self {
        unit_fs: 1_000_000,
        precision_fs: 1_000,
    };
}

/// Parse the first `` `timescale <unit>/<precision> `` directive.
#[cfg(test)]
pub(super) fn parse_timescale(text: &str) -> Option<Timescale> {
    let index = text.find("`timescale")?;
    let rest = &text[index + "`timescale".len()..];
    let (unit_fs, rest) = parse_value(skip_ws(rest))?;
    let rest = skip_ws(rest).strip_prefix('/')?;
    let (precision_fs, _) = parse_value(skip_ws(rest))?;
    Some(Timescale {
        unit_fs,
        precision_fs,
    })
}

#[cfg(test)]
fn parse_value(value: &str) -> Option<(u64, &str)> {
    let (digits, rest) = split_at_while(value, |character| character.is_ascii_digit());
    if digits.is_empty() {
        return None;
    }
    let multiplier: u64 = digits.parse().ok()?;
    let (unit, rest) = split_at_while(skip_ws(rest), |character| character.is_ascii_alphabetic());
    let base = match unit {
        "s" => 1_000_000_000_000_000,
        "ms" => 1_000_000_000_000,
        "us" => 1_000_000_000,
        "ns" => 1_000_000,
        "ps" => 1_000,
        "fs" => 1,
        _ => return None,
    };
    multiplier.checked_mul(base).map(|value| (value, rest))
}

#[cfg(test)]
fn split_at_while(value: &str, predicate: impl Fn(char) -> bool) -> (&str, &str) {
    let end = value
        .find(|character| !predicate(character))
        .unwrap_or(value.len());
    (&value[..end], &value[end..])
}

#[cfg(test)]
fn skip_ws(value: &str) -> &str {
    value.trim_start_matches([' ', '\t'])
}

/// Convert a resolved real delay in module units to local precision ticks.
pub(super) fn real_delay_ticks(value: f64, timescale: Timescale) -> Result<u64, String> {
    if !value.is_finite() || value < 0.0 {
        return Err("real parameter delay must be finite and nonnegative".to_string());
    }
    if timescale.unit_fs == 0 || timescale.precision_fs == 0 {
        return Err("delay requires a nonzero time unit and precision".to_string());
    }
    let ticks = (value * (timescale.unit_fs as f64 / timescale.precision_fs as f64)).round();
    // u64::MAX rounds to 2^64 as an f64, so the upper bound is exclusive.
    if !ticks.is_finite() || ticks >= 18_446_744_073_709_551_616.0 {
        return Err("rounded real parameter delay exceeds the 64-bit tick range".to_string());
    }
    Ok(ticks as u64)
}

/// Round one 2009 time literal in its owning module's unit before ordinary
/// expression use. The source spelling is required so halfway cases do not
/// depend on the frontend's binary `f64` conversion.
pub(super) fn round_time_literal(
    value: f64,
    source: &str,
    scale: crate::core::db::TimeLiteralScale,
    timescale: Timescale,
) -> Result<f64, String> {
    let (physical_numerator, decimal_denominator, unit_femtoseconds) =
        time_literal_rational(value, source, scale, timescale)?;
    let precision_femtoseconds = u128::from(timescale.precision_fs);
    let tick_denominator = decimal_denominator
        .checked_mul(precision_femtoseconds)
        .ok_or_else(|| "time literal value exceeds the supported range".to_owned())?;
    let ticks = round_rational(physical_numerator, tick_denominator)?;
    let rounded_numerator = ticks
        .checked_mul(
            i128::try_from(precision_femtoseconds)
                .map_err(|_| "time literal precision exceeds the supported range".to_owned())?,
        )
        .ok_or_else(|| "rounded time literal exceeds the supported range".to_owned())?;
    rational_to_f64(rounded_numerator, unit_femtoseconds)
}

/// Convert one pure time literal delay directly to local precision ticks.
/// Keeping the decimal numerator/denominator intact avoids losing a
/// femtosecond at the delay boundary when the frontend value is an `f64`.
pub(super) fn time_literal_delay_ticks(
    value: f64,
    source: &str,
    scale: crate::core::db::TimeLiteralScale,
    timescale: Timescale,
) -> Result<u64, String> {
    let (physical_numerator, decimal_denominator, _) =
        time_literal_rational(value, source, scale, timescale)?;
    let precision_femtoseconds = u128::from(timescale.precision_fs);
    let tick_denominator = decimal_denominator
        .checked_mul(precision_femtoseconds)
        .ok_or_else(|| "time literal value exceeds the supported range".to_owned())?;
    let ticks = round_rational(physical_numerator, tick_denominator)?;
    if ticks < 0 {
        return Err("time literal delay must be finite and nonnegative".to_owned());
    }
    u64::try_from(ticks).map_err(|_| {
        "rounded time literal delay exceeds the 64-bit tick range".to_owned()
    })
}

fn time_literal_rational(
    value: f64,
    source: &str,
    scale: crate::core::db::TimeLiteralScale,
    timescale: Timescale,
) -> Result<(i128, u128, u128), String> {
    if !value.is_finite() {
        return Err("time literal value is not finite".to_owned());
    }
    if !matches!(scale.magnitude, 1 | 10 | 100) {
        return Err("time literal has an invalid unit magnitude".to_owned());
    }
    let (coefficient, decimal_denominator, literal_unit) = parse_decimal_rational(source)?;
    let literal_femtoseconds = time_unit_femtoseconds(literal_unit);
    let unit_femtoseconds = time_unit_femtoseconds(scale.unit)
        .checked_mul(u128::from(scale.magnitude))
        .ok_or_else(|| "time literal unit scale overflows the supported range".to_owned())?;
    let precision_femtoseconds = u128::from(timescale.precision_fs);
    if precision_femtoseconds == 0 {
        return Err("time literal has an invalid owning time precision".to_owned());
    }
    let physical_numerator = coefficient
        .checked_mul(
            i128::try_from(literal_femtoseconds)
                .map_err(|_| "time literal unit scale exceeds the supported range".to_owned())?,
        )
        .ok_or_else(|| "time literal value exceeds the supported range".to_owned())?;
    let expected_denominator = decimal_denominator
        .checked_mul(unit_femtoseconds)
        .ok_or_else(|| "time literal value exceeds the supported range".to_owned())?;
    let expected = rational_to_f64(physical_numerator, expected_denominator)?;
    let tolerance = expected.abs().max(1.0) * 1.0e-9;
    if (value - expected).abs() > tolerance {
        return Err(format!(
            "time literal source does not match its captured value ({source}: {value} != {expected})"
        ));
    }
    Ok((physical_numerator, decimal_denominator, unit_femtoseconds))
}

fn time_unit_femtoseconds(unit: crate::core::db::TimeUnit) -> u128 {
    match unit {
        crate::core::db::TimeUnit::Seconds => 1_000_000_000_000_000,
        crate::core::db::TimeUnit::Milliseconds => 1_000_000_000_000,
        crate::core::db::TimeUnit::Microseconds => 1_000_000_000,
        crate::core::db::TimeUnit::Nanoseconds => 1_000_000,
        crate::core::db::TimeUnit::Picoseconds => 1_000,
        crate::core::db::TimeUnit::Femtoseconds => 1,
    }
}

fn parse_decimal_rational(source: &str) -> Result<(i128, u128, crate::core::db::TimeUnit), String> {
    let token = source.trim();
    let (numeric, literal_unit) = [
        ("ms", crate::core::db::TimeUnit::Milliseconds),
        ("us", crate::core::db::TimeUnit::Microseconds),
        ("ns", crate::core::db::TimeUnit::Nanoseconds),
        ("ps", crate::core::db::TimeUnit::Picoseconds),
        ("fs", crate::core::db::TimeUnit::Femtoseconds),
        ("s", crate::core::db::TimeUnit::Seconds),
    ]
    .iter()
    .find_map(|(suffix, unit)| token.strip_suffix(suffix).map(|numeric| (numeric, *unit)))
    .ok_or_else(|| "time literal source has an invalid unit suffix".to_owned())?;
    let numeric = numeric.trim();
    let numeric_bytes = numeric.as_bytes();
    if numeric_bytes.iter().enumerate().any(|(index, byte)| {
        *byte == b'_'
            && (index == 0
                || index + 1 == numeric_bytes.len()
                || !numeric_bytes[index - 1].is_ascii_digit()
                || !numeric_bytes[index + 1].is_ascii_digit())
    }) {
        return Err("time literal source has an invalid digit separator".to_owned());
    }
    let numeric = numeric.replace('_', "");
    let bytes = numeric.as_bytes();
    let mut index = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let negative = bytes.first() == Some(&b'-');
    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    let integer_end = index;
    let fraction_start = if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        Some(start)
    } else {
        None
    };
    let fraction_end = index;
    let has_integer = index != integer_start;
    let has_fraction = fraction_start.is_some_and(|start| start != fraction_end);
    if !has_integer && !has_fraction {
        return Err("time literal source has no numeric digits".to_owned());
    }
    let exponent = if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        let exponent_negative = if matches!(bytes.get(index), Some(b'+' | b'-')) {
            let negative = bytes[index] == b'-';
            index += 1;
            negative
        } else {
            false
        };
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if start == index {
            return Err("time literal source has an invalid exponent".to_owned());
        }
        let value = numeric[start..index]
            .parse::<i32>()
            .map_err(|_| "time literal exponent exceeds the supported range".to_owned())?;
        if exponent_negative {
            value
                .checked_neg()
                .ok_or_else(|| "time literal exponent exceeds the supported range".to_owned())?
        } else {
            value
        }
    } else {
        0
    };
    if index != bytes.len() {
        return Err("time literal source has an invalid numeric value".to_owned());
    }
    let digits = format!(
        "{}{}",
        &numeric[integer_start..integer_end],
        fraction_start.map_or("", |start| &numeric[start..fraction_end])
    );
    let mut coefficient = digits
        .parse::<i128>()
        .map_err(|_| "time literal value exceeds the supported range".to_owned())?;
    let fraction_digits = fraction_start.map_or(0, |start| fraction_end - start);
    let decimal_exponent = exponent
        .checked_sub(i32::try_from(fraction_digits).map_err(|_| {
            "time literal fractional precision exceeds the supported range".to_owned()
        })?)
        .ok_or_else(|| "time literal decimal scale exceeds the supported range".to_owned())?;
    let denominator =
        if decimal_exponent >= 0 {
            coefficient = coefficient
                .checked_mul(
                    pow10(u32::try_from(decimal_exponent).map_err(|_| {
                        "time literal exponent exceeds the supported range".to_owned()
                    })?)?
                    .try_into()
                    .map_err(|_| "time literal exponent exceeds the supported range".to_owned())?,
                )
                .ok_or_else(|| "time literal value exceeds the supported range".to_owned())?;
            1
        } else {
            pow10(
                u32::try_from(decimal_exponent.checked_neg().ok_or_else(|| {
                    "time literal exponent exceeds the supported range".to_owned()
                })?)
                .map_err(|_| "time literal exponent exceeds the supported range".to_owned())?,
            )?
        };
    if negative {
        coefficient = coefficient
            .checked_neg()
            .ok_or_else(|| "time literal value exceeds the supported range".to_owned())?;
    }
    Ok((coefficient, denominator, literal_unit))
}

fn pow10(exponent: u32) -> Result<u128, String> {
    if exponent > 38 {
        return Err("time literal decimal scale exceeds the supported range".to_owned());
    }
    (0..exponent).try_fold(1_u128, |value, _| {
        value
            .checked_mul(10)
            .ok_or_else(|| "time literal decimal scale exceeds the supported range".to_owned())
    })
}

fn round_rational(numerator: i128, denominator: u128) -> Result<i128, String> {
    let denominator = i128::try_from(denominator)
        .map_err(|_| "time literal denominator exceeds the supported range".to_owned())?;
    if denominator == 0 {
        return Err("time literal has a zero precision".to_owned());
    }
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let magnitude = if remainder < 0 { -remainder } else { remainder };
    let half_denominator = denominator / 2 + denominator % 2;
    if magnitude >= half_denominator {
        quotient
            .checked_add(if numerator < 0 { -1 } else { 1 })
            .ok_or_else(|| "rounded time literal exceeds the supported range".to_owned())
    } else {
        Ok(quotient)
    }
}

fn rational_to_f64(numerator: i128, denominator: u128) -> Result<f64, String> {
    if denominator == 0 {
        return Err("time literal has a zero scale".to_owned());
    }
    let value = numerator as f64 / denominator as f64;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| "time literal value exceeds the supported range".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spaced_timescale() {
        assert_eq!(
            parse_timescale("module top;\n`timescale 10 ns / 100 ps\n"),
            Some(Timescale {
                unit_fs: 10_000_000,
                precision_fs: 100_000
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
    fn parses_the_complete_physical_time_range() {
        assert_eq!(
            parse_timescale("`timescale 1fs/1fs"),
            Some(Timescale {
                unit_fs: 1,
                precision_fs: 1,
            })
        );
        assert_eq!(
            parse_timescale("`timescale 10fs/100fs"),
            Some(Timescale {
                unit_fs: 10,
                precision_fs: 100,
            })
        );
        assert_eq!(
            parse_timescale("`timescale 100s/10s"),
            Some(Timescale {
                unit_fs: 100_000_000_000_000_000,
                precision_fs: 10_000_000_000_000_000,
            })
        );
    }

    #[test]
    fn real_delays_check_rounding_and_bounds() {
        let timescale = Timescale {
            unit_fs: 1_000_000,
            precision_fs: 100_000,
        };
        assert_eq!(real_delay_ticks(0.25, timescale), Ok(3));
        for value in [-0.1, f64::NAN, f64::INFINITY, 18_446_744_073_709_551_616.0] {
            assert!(real_delay_ticks(value, timescale).is_err(), "{value}");
        }
    }

    #[test]
    fn time_literals_round_decimal_boundaries_away_from_zero() {
        let scale = crate::core::db::TimeLiteralScale {
            unit: crate::core::db::TimeUnit::Nanoseconds,
            magnitude: 1,
        };
        let timescale = Timescale {
            unit_fs: 1_000_000,
            precision_fs: 100_000,
        };
        assert_eq!(
            round_time_literal(1.54, "1.54ns", scale, timescale),
            Ok(1.5)
        );
        assert_eq!(
            round_time_literal(1.55, "1.55ns", scale, timescale),
            Ok(1.6)
        );
        assert_eq!(
            round_time_literal(-1.54, "-1.54ns", scale, timescale),
            Ok(-1.5)
        );
        assert_eq!(
            round_time_literal(-1.55, "-1.55ns", scale, timescale),
            Ok(-1.6)
        );
    }

    #[test]
    fn time_literal_source_parser_handles_exponents_and_leading_fraction() {
        assert_eq!(
            parse_decimal_rational("1e3ps"),
            Ok((1_000, 1, crate::core::db::TimeUnit::Picoseconds))
        );
        assert_eq!(
            parse_decimal_rational(".5ns"),
            Ok((5, 10, crate::core::db::TimeUnit::Nanoseconds))
        );
    }

    #[test]
    fn time_literals_scale_to_an_explicit_scope_unit() {
        let scale = crate::core::db::TimeLiteralScale {
            unit: crate::core::db::TimeUnit::Nanoseconds,
            magnitude: 10,
        };
        let timescale = Timescale {
            unit_fs: 10_000_000,
            precision_fs: 1_000_000,
        };
        assert_eq!(round_time_literal(1.6, "16ns", scale, timescale), Ok(1.6));
    }
}
