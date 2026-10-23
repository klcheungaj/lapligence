//! Exact SDF real numbers and min:typ:max values (OVI SDF 3.0 "Variables"
//! and "Data Values").
//!
//! SDF numbers are decimal text such as `.145`, `2.4e2` or `-5.3e-1`. They
//! are kept as an exact decimal `mantissa * 10^exponent`, so equal text
//! always yields equal values and the conversion to scheduler ticks (with
//! the file's timescale, a scale factor and the design precision) rounds
//! exactly once, when the plan is applied.

use std::fmt;

/// Significant decimal digits kept exactly; an `i64` holds 18 of them,
/// more than any delay calculator writes, and keeps a number at 16 bytes.
pub const MAX_SIGNIFICANT_DIGITS: usize = 18;

/// Largest magnitude of the decimal exponent. Delays beyond `1e300` time
/// units or below `1e-300` have no representable tick value anyway.
pub const MAX_DECIMAL_EXPONENT: i32 = 300;

/// An exact decimal number: `mantissa * 10^exponent`, normalized so that the
/// mantissa has no trailing zero digit (zero is `0 * 10^0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SdfNumber {
    mantissa: i64,
    exponent: i16,
}

/// Why a number was rejected; the parser adds the position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberError {
    Malformed,
    TooManyDigits,
    ExponentRange,
}

impl SdfNumber {
    pub const ZERO: Self = Self {
        mantissa: 0,
        exponent: 0,
    };

    /// `mantissa * 10^exponent`, normalized.
    pub fn new(mantissa: i64, exponent: i16) -> Self {
        let mut number = Self { mantissa, exponent };
        number.normalize();
        number
    }

    /// The number with the opposite sign.
    pub fn negated(&self) -> Self {
        Self {
            mantissa: -self.mantissa,
            exponent: self.exponent,
        }
    }

    fn normalize(&mut self) {
        if self.mantissa == 0 {
            self.exponent = 0;
            return;
        }
        while self.mantissa % 10 == 0 {
            self.mantissa /= 10;
            self.exponent += 1;
        }
    }

    pub fn mantissa(&self) -> i64 {
        self.mantissa
    }

    pub fn exponent(&self) -> i32 {
        i32::from(self.exponent)
    }

    pub fn is_negative(&self) -> bool {
        self.mantissa < 0
    }

    /// Parse `[+-]? (digits [. digits?] | . digits) ([eE] [+-]? digits)?`.
    /// The sign is accepted here; callers that need a NUMBER (non-negative)
    /// reject a negative result themselves.
    pub fn parse(text: &str) -> Result<Self, NumberError> {
        let bytes = text.as_bytes();
        let mut index = 0;
        let negative = match bytes.first() {
            Some(b'-') => {
                index += 1;
                true
            }
            Some(b'+') => {
                index += 1;
                false
            }
            _ => false,
        };
        let mut mantissa: i64 = 0;
        let mut significant = 0usize;
        let mut exponent: i64 = 0;
        let mut digits = 0usize;
        let mut fraction = false;
        while index < bytes.len() {
            match bytes[index] {
                digit @ b'0'..=b'9' => {
                    digits += 1;
                    let value = i64::from(digit - b'0');
                    if significant > 0 || value != 0 {
                        significant += 1;
                        if significant > MAX_SIGNIFICANT_DIGITS {
                            return Err(NumberError::TooManyDigits);
                        }
                        mantissa = mantissa * 10 + value;
                        if fraction {
                            exponent -= 1;
                        }
                    } else if fraction {
                        // A leading zero after the point still scales.
                        exponent -= 1;
                    }
                }
                b'.' if !fraction => fraction = true,
                b'e' | b'E' => break,
                _ => return Err(NumberError::Malformed),
            }
            index += 1;
        }
        if digits == 0 {
            return Err(NumberError::Malformed);
        }
        if index < bytes.len() {
            // The exponent part.
            index += 1;
            let mut exponent_negative = false;
            match bytes.get(index) {
                Some(b'-') => {
                    exponent_negative = true;
                    index += 1;
                }
                Some(b'+') => index += 1,
                _ => {}
            }
            let start = index;
            let mut value: i64 = 0;
            while index < bytes.len() {
                let digit = bytes[index];
                if !digit.is_ascii_digit() {
                    return Err(NumberError::Malformed);
                }
                value = value
                    .saturating_mul(10)
                    .saturating_add(i64::from(digit - b'0'));
                index += 1;
            }
            if index == start {
                return Err(NumberError::Malformed);
            }
            exponent += if exponent_negative { -value } else { value };
        }
        if mantissa == 0 {
            return Ok(Self::ZERO);
        }
        let mut number = Self {
            mantissa: if negative { -mantissa } else { mantissa },
            exponent: 0,
        };
        // Normalize before the range check so `1000e-302` stays legal.
        let mut trailing = 0i64;
        while number.mantissa % 10 == 0 {
            number.mantissa /= 10;
            trailing += 1;
        }
        let exponent = exponent + trailing;
        if exponent.abs() > i64::from(MAX_DECIMAL_EXPONENT) {
            return Err(NumberError::ExponentRange);
        }
        // |exponent| <= MAX_DECIMAL_EXPONENT fits.
        number.exponent = exponent as i16;
        Ok(number)
    }

    /// Compare two numbers exactly.
    pub fn cmp_exact(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let (a, b) = (self.mantissa, other.mantissa);
        if a.signum() != b.signum() {
            return a.signum().cmp(&b.signum());
        }
        if a == 0 {
            return Ordering::Equal;
        }
        // Same sign and both nonzero: compare magnitudes by aligning the
        // exponents on the digit strings, which never overflows.
        let magnitude = |mantissa: i64, exponent: i16| {
            let digits = mantissa.unsigned_abs().to_string();
            (digits.len() as i64 + i64::from(exponent), digits)
        };
        let (order_a, digits_a) = magnitude(a, self.exponent);
        let (order_b, digits_b) = magnitude(b, other.exponent);
        let by_magnitude = order_a.cmp(&order_b).then_with(|| {
            let width = digits_a.len().max(digits_b.len());
            format!("{digits_a:0<width$}").cmp(&format!("{digits_b:0<width$}"))
        });
        if a > 0 {
            by_magnitude
        } else {
            by_magnitude.reverse()
        }
    }
}

impl fmt::Display for SdfNumber {
    /// Plain decimal text without exponent: `0.145`, `-3`, `240`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = self.mantissa.unsigned_abs().to_string();
        let sign = if self.mantissa < 0 { "-" } else { "" };
        if self.exponent >= 0 {
            // Large exponents print in scientific form to stay bounded.
            if self.exponent > 30 {
                return write!(f, "{sign}{digits}e{}", self.exponent);
            }
            return write!(f, "{sign}{digits}{}", "0".repeat(self.exponent as usize));
        }
        let fraction = (-self.exponent) as usize;
        if fraction > 30 + digits.len() {
            return write!(f, "{sign}{digits}e{}", self.exponent);
        }
        if digits.len() > fraction {
            let (whole, part) = digits.split_at(digits.len() - fraction);
            write!(f, "{sign}{whole}.{part}")
        } else {
            write!(f, "{sign}0.{}{digits}", "0".repeat(fraction - digits.len()))
        }
    }
}

/// A data value: one number for every data set, or a min:typ:max triple
/// whose members may be absent ("no value is included for that data set").
/// An empty `()` is represented by the absence of a value (`None`) at the
/// use site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfValue {
    Single(SdfNumber),
    Triple([Option<SdfNumber>; 3]),
}

impl SdfValue {
    /// The member for data set `index` (0 minimum, 1 typical, 2 maximum).
    pub fn member(&self, index: usize) -> Option<SdfNumber> {
        match self {
            Self::Single(number) => Some(*number),
            Self::Triple(members) => members.get(index).copied().flatten(),
        }
    }

    pub fn any_negative(&self) -> bool {
        (0..3).any(|index| {
            self.member(index)
                .is_some_and(|number| number.is_negative())
        })
    }
}

impl fmt::Display for SdfValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(number) => write!(f, "{number}"),
            Self::Triple(members) => {
                for (index, member) in members.iter().enumerate() {
                    if index > 0 {
                        f.write_str(":")?;
                    }
                    if let Some(number) = member {
                        write!(f, "{number}")?;
                    }
                }
                Ok(())
            }
        }
    }
}

/// Write an optional value as SDF writes it: `()` when absent.
pub fn fmt_optional(value: Option<&SdfValue>) -> String {
    match value {
        Some(value) => format!("({value})"),
        None => "()".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(text: &str) -> SdfNumber {
        SdfNumber::parse(text).unwrap_or_else(|error| panic!("{text}: {error:?}"))
    }

    #[test]
    fn numbers_parse_exactly() {
        assert_eq!(number(".145"), SdfNumber::new(145, -3));
        assert_eq!(number("0.145"), SdfNumber::new(145, -3));
        assert_eq!(number("2.4e2"), SdfNumber::new(240, 0));
        assert_eq!(number("5.3e-1"), SdfNumber::new(53, -2));
        assert_eq!(number("-0.3"), SdfNumber::new(-3, -1));
        assert_eq!(number("+12"), SdfNumber::new(12, 0));
        assert_eq!(number("100.0"), SdfNumber::new(1, 2));
        assert_eq!(number("0"), SdfNumber::ZERO);
        assert_eq!(number("-0.0"), SdfNumber::ZERO);
        assert_eq!(number("7."), SdfNumber::new(7, 0));
        assert_eq!(number("1E3"), SdfNumber::new(1, 3));
        assert_eq!(number("0.000000"), SdfNumber::ZERO);
    }

    #[test]
    fn malformed_numbers_are_rejected() {
        for text in [
            "", ".", "-", "1.2.3", "1e", "1e+", "e3", "1x", "--1", "1..2", ".e1",
        ] {
            assert_eq!(
                SdfNumber::parse(text),
                Err(NumberError::Malformed),
                "{text}"
            );
        }
        assert_eq!(
            SdfNumber::parse(&"9".repeat(19)),
            Err(NumberError::TooManyDigits)
        );
        assert_eq!(SdfNumber::parse(&"9".repeat(18)).map(|_| ()), Ok(()));
        // Trailing zeros of an integer part are significant here.
        assert_eq!(
            SdfNumber::parse(&format!("1{}", "0".repeat(18))),
            Err(NumberError::TooManyDigits)
        );
        assert_eq!(SdfNumber::parse("1e18"), Ok(SdfNumber::new(1, 18)));
        assert_eq!(SdfNumber::parse("1e301"), Err(NumberError::ExponentRange));
        assert_eq!(SdfNumber::parse("1000e-302"), Ok(SdfNumber::new(1, -299)));
        // Leading zeros are not significant digits.
        assert_eq!(
            SdfNumber::parse(&format!("0000{}", "1".repeat(18))).map(|_| ()),
            Ok(())
        );
    }

    #[test]
    fn display_is_plain_decimal() {
        assert_eq!(number(".145").to_string(), "0.145");
        assert_eq!(number("-3").to_string(), "-3");
        assert_eq!(number("2.4e2").to_string(), "240");
        assert_eq!(number("0.0005").to_string(), "0.0005");
        assert_eq!(number("12.5").to_string(), "12.5");
        assert_eq!(number("1e40").to_string(), "1e40");
    }

    #[test]
    fn exact_comparison() {
        use std::cmp::Ordering::*;
        assert_eq!(number("0.1").cmp_exact(&number("0.10")), Equal);
        assert_eq!(number("0.1").cmp_exact(&number("0.09")), Greater);
        assert_eq!(number("-2").cmp_exact(&number("-10")), Greater);
        assert_eq!(number("-2").cmp_exact(&number("1")), Less);
        assert_eq!(number("1e3").cmp_exact(&number("999")), Greater);
        assert_eq!(number("0").cmp_exact(&number("-0.5")), Greater);
    }

    /// Plans keep every delay value until they are applied: keep them small.
    #[test]
    fn values_stay_compact() {
        assert_eq!(std::mem::size_of::<SdfNumber>(), 16);
        assert!(std::mem::size_of::<SdfValue>() <= 80);
        assert!(std::mem::size_of::<crate::sim::sdf::SdfDelval>() <= 88);
    }

    #[test]
    fn values_select_members() {
        let single = SdfValue::Single(number("2"));
        assert_eq!(single.member(2), Some(number("2")));
        let triple = SdfValue::Triple([Some(number("1")), None, Some(number("-3"))]);
        assert_eq!(triple.member(1), None);
        assert!(triple.any_negative());
        assert_eq!(triple.to_string(), "1::-3");
        assert_eq!(fmt_optional(None), "()");
    }
}
