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
}
