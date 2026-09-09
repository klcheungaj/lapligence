//! Frontend-neutral owned values shared by semantic analysis and simulation.

/// A single four-state scalar value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarValue {
    Zero,
    One,
    X,
    Z,
    /// A wildcard / don't-care bit retained from source semantics.
    DontCare,
}

/// Owned value payload used by the semantic database.
///
/// Radix strings remain available while consumers migrate to the exact
/// four-state vector representation emitted by Slang. Vector planes are
/// least-significant-word first. For each bit `(unknown, value)` encodes
/// zero as `(0, 0)`, one as `(0, 1)`, X as `(1, 0)`, and Z as `(1, 1)`.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueData {
    None,
    Scalar(ScalarValue),
    Int(i64),
    UInt(u64),
    Real(f64),
    /// Text recovered from source spelling and known to be UTF-8.
    Str(String),
    /// Exact SystemVerilog string bytes; values need not be UTF-8.
    Bytes(Vec<u8>),
    Bin(String),
    Oct(String),
    Dec(String),
    Hex(String),
    Vector {
        bit_width: u64,
        is_signed: bool,
        value_words: Vec<u64>,
        unknown_words: Vec<u64>,
    },
    TooWide {
        bits: usize,
    },
}

impl ValueData {
    /// Convert a fully known integral value to `i128` without truncation.
    pub fn to_i128(&self) -> Option<i128> {
        match self {
            Self::Int(value) => Some(i128::from(*value)),
            Self::UInt(value) => Some(i128::from(*value)),
            Self::Scalar(ScalarValue::Zero) => Some(0),
            Self::Scalar(ScalarValue::One) => Some(1),
            Self::Bin(value) => parse_radix(value, 2),
            Self::Oct(value) => parse_radix(value, 8),
            Self::Dec(value) => value.replace('_', "").parse().ok(),
            Self::Hex(value) => parse_radix(value, 16),
            Self::Vector {
                bit_width,
                is_signed,
                value_words,
                unknown_words,
            } => vector_to_i128(*bit_width, *is_signed, value_words, unknown_words),
            _ => None,
        }
    }
}

fn vector_to_i128(
    bit_width: u64,
    is_signed: bool,
    value_words: &[u64],
    unknown_words: &[u64],
) -> Option<i128> {
    let expected_words = usize::try_from(bit_width.div_ceil(64)).ok()?;
    if value_words.len() != expected_words
        || unknown_words.len() != expected_words
        || unknown_words.iter().any(|word| *word != 0)
        || bit_width == 0
    {
        return None;
    }
    let sign = is_signed && bit_width > 0 && bit(value_words, bit_width - 1);
    if bit_width >= u64::from(i128::BITS)
        && (i128::BITS as u64 - 1..bit_width).any(|index| bit(value_words, index) != sign)
    {
        return None;
    }
    let retained = bit_width.min(u64::from(i128::BITS));
    let mut raw = 0_u128;
    for index in 0..retained {
        if bit(value_words, index) {
            raw |= 1_u128 << index;
        }
    }
    if sign && retained < u64::from(i128::BITS) {
        raw |= !0_u128 << retained;
    }
    if !is_signed && bit_width == u64::from(i128::BITS) && raw > i128::MAX as u128 {
        return None;
    }
    Some(raw as i128)
}

fn bit(words: &[u64], index: u64) -> bool {
    let word = usize::try_from(index / 64).ok();
    let shift = (index % 64) as u32;
    word.and_then(|word| words.get(word))
        .is_some_and(|value| value & (1_u64 << shift) != 0)
}

fn parse_radix(value: &str, radix: u32) -> Option<i128> {
    let digits = value.replace('_', "");
    if digits.is_empty()
        || digits
            .bytes()
            .any(|byte| matches!(byte, b'x' | b'X' | b'z' | b'Z' | b'?'))
    {
        return None;
    }
    let (negative, digits) = digits
        .strip_prefix('-')
        .map_or((false, digits.as_str()), |rest| (true, rest));
    let magnitude = u128::from_str_radix(digits, radix).ok()?;
    if negative {
        if magnitude == 1_u128 << 127 {
            Some(i128::MIN)
        } else {
            i128::try_from(magnitude).ok()?.checked_neg()
        }
    } else {
        i128::try_from(magnitude).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_vector_integer_conversion_rejects_unknown_and_overflow() {
        let known = ValueData::Vector {
            bit_width: 8,
            is_signed: true,
            value_words: vec![0xff],
            unknown_words: vec![0],
        };
        assert_eq!(known.to_i128(), Some(-1));

        let unknown = ValueData::Vector {
            bit_width: 1,
            is_signed: false,
            value_words: vec![0],
            unknown_words: vec![1],
        };
        assert_eq!(unknown.to_i128(), None);

        let overflow = ValueData::Vector {
            bit_width: 129,
            is_signed: false,
            value_words: vec![0, 0, 1],
            unknown_words: vec![0, 0, 0],
        };
        assert_eq!(overflow.to_i128(), None);

        let retained_sign_overflow = ValueData::Vector {
            bit_width: 129,
            is_signed: false,
            value_words: vec![0, 1_u64 << 63, 0],
            unknown_words: vec![0, 0, 0],
        };
        assert_eq!(retained_sign_overflow.to_i128(), None);

        let bad_signed_extension = ValueData::Vector {
            bit_width: 129,
            is_signed: true,
            value_words: vec![0, 0, 1],
            unknown_words: vec![0, 0, 0],
        };
        assert_eq!(bad_signed_extension.to_i128(), None);
        assert_eq!(ValueData::Hex("7f".into()).to_i128(), Some(127));
        assert_eq!(ValueData::Bin("1x".into()).to_i128(), None);
    }
}
