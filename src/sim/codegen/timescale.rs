/// A Verilog time unit/precision pair in picoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Timescale {
    pub(super) unit_ps: u64,
    pub(super) precision_ps: u64,
}

impl Timescale {
    /// Default for a module without an explicit time scale (1ns/1ps).
    pub(super) const DEFAULT: Self = Self {
        unit_ps: 1_000,
        precision_ps: 1,
    };
}

/// Parse the first `` `timescale <unit>/<precision> `` directive.
#[cfg(test)]
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

#[cfg(test)]
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
    fn real_delays_check_rounding_and_bounds() {
        let timescale = Timescale {
            unit_ps: 1_000,
            precision_ps: 100,
        };
        assert_eq!(real_delay_ticks(0.25, timescale), Ok(3));
        for value in [-0.1, f64::NAN, f64::INFINITY, 18_446_744_073_709_551_616.0] {
            assert!(real_delay_ticks(value, timescale).is_err(), "{value}");
        }
    }
}
