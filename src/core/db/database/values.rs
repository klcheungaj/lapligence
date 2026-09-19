//! Values.

use super::*;

pub(in crate::core::db) fn value_data_from_slang(value: &SlangConstantValue) -> ValueData {
    match value {
        SlangConstantValue::None => ValueData::None,
        SlangConstantValue::Integer {
            is_signed,
            bit_width,
            value_words,
            unknown_words,
        } => ValueData::Vector {
            bit_width: *bit_width,
            is_signed: *is_signed,
            value_words: value_words.clone(),
            unknown_words: unknown_words.clone(),
        },
        SlangConstantValue::Real(value) => ValueData::Real(*value),
        SlangConstantValue::ShortReal(value) => ValueData::Real(f64::from(*value)),
        SlangConstantValue::String(value) => ValueData::Bytes(value.clone()),
        SlangConstantValue::Other(_) => ValueData::None,
    }
}

pub(super) fn val_from_slang(value: &SlangConstantValue) -> Option<Val> {
    match value {
        SlangConstantValue::Integer {
            is_signed,
            bit_width,
            value_words,
            unknown_words,
        } => {
            let mut bits = Vec::with_capacity(usize::try_from(*bit_width).ok()?);
            for index in (0..*bit_width).rev() {
                let word = usize::try_from(index / 64).ok()?;
                let mask = 1_u64 << (index % 64);
                let value = value_words.get(word).is_some_and(|word| word & mask != 0);
                let unknown = unknown_words.get(word).is_some_and(|word| word & mask != 0);
                bits.push(match (unknown, value) {
                    (false, false) => crate::core::elab::Bit::Zero,
                    (false, true) => crate::core::elab::Bit::One,
                    (true, false) => crate::core::elab::Bit::X,
                    (true, true) => crate::core::elab::Bit::Z,
                });
            }
            Some(Val::Bits(crate::core::elab::Value::from_bits(
                bits, *is_signed,
            )))
        }
        SlangConstantValue::Real(value) => Some(Val::Real(*value)),
        SlangConstantValue::ShortReal(value) => Some(Val::Real(f64::from(*value))),
        SlangConstantValue::String(value) => String::from_utf8(value.clone()).ok().map(Val::Str),
        SlangConstantValue::None | SlangConstantValue::Other(_) => None,
    }
}

pub(super) fn operation_from_slang(operation: SemanticOperation, unary: bool) -> Operation {
    match operation {
        SemanticOperation::None => Operation::Null,
        SemanticOperation::Plus if unary => Operation::UnaryPlus,
        SemanticOperation::Minus if unary => Operation::UnaryMinus,
        SemanticOperation::Plus => Operation::Add,
        SemanticOperation::Minus => Operation::Subtract,
        SemanticOperation::Multiply => Operation::Multiply,
        SemanticOperation::Divide => Operation::Divide,
        SemanticOperation::Modulo => Operation::Modulo,
        SemanticOperation::Power => Operation::Power,
        SemanticOperation::BitNot => Operation::BitwiseNot,
        SemanticOperation::BitAnd if unary => Operation::ReductionAnd,
        SemanticOperation::BitOr if unary => Operation::ReductionOr,
        SemanticOperation::BitXor if unary => Operation::ReductionXor,
        SemanticOperation::BitNand if unary => Operation::ReductionNand,
        SemanticOperation::BitNor if unary => Operation::ReductionNor,
        SemanticOperation::BitXnor if unary => Operation::ReductionXnor,
        SemanticOperation::BitAnd | SemanticOperation::BitNand => Operation::BitwiseAnd,
        SemanticOperation::BitOr | SemanticOperation::BitNor => Operation::BitwiseOr,
        SemanticOperation::BitXor => Operation::BitwiseXor,
        SemanticOperation::BitXnor => Operation::BitwiseXnor,
        SemanticOperation::LogicalNot => Operation::LogicalNot,
        SemanticOperation::LogicalAnd => Operation::LogicalAnd,
        SemanticOperation::LogicalOr => Operation::LogicalOr,
        SemanticOperation::LogicalImplication => Operation::Imply,
        SemanticOperation::LogicalEquivalence => Operation::LogicalEquivalence,
        SemanticOperation::Equal => Operation::Equal,
        SemanticOperation::NotEqual => Operation::NotEqual,
        SemanticOperation::CaseEqual => Operation::CaseEqual,
        SemanticOperation::CaseNotEqual => Operation::CaseNotEqual,
        SemanticOperation::WildcardEqual => Operation::WildEqual,
        SemanticOperation::WildcardNotEqual => Operation::WildNotEqual,
        SemanticOperation::Greater => Operation::Greater,
        SemanticOperation::GreaterEqual => Operation::GreaterEqual,
        SemanticOperation::Less => Operation::Less,
        SemanticOperation::LessEqual => Operation::LessEqual,
        SemanticOperation::ShiftLeft => Operation::ShiftLeft,
        SemanticOperation::ShiftRight => Operation::ShiftRight,
        SemanticOperation::ArithmeticShiftLeft => Operation::ArithmeticShiftLeft,
        SemanticOperation::ArithmeticShiftRight => Operation::ArithmeticShiftRight,
        SemanticOperation::PreIncrement => Operation::PreIncrement,
        SemanticOperation::PreDecrement => Operation::PreDecrement,
        SemanticOperation::PostIncrement => Operation::PostIncrement,
        SemanticOperation::PostDecrement => Operation::PostDecrement,
        SemanticOperation::Concat => Operation::Concat,
        SemanticOperation::Replicate => Operation::MultiConcat,
        SemanticOperation::Conditional => Operation::Conditional,
        SemanticOperation::StreamLeft => Operation::StreamLeftToRight,
        SemanticOperation::StreamRight => Operation::StreamRightToLeft,
        SemanticOperation::Assign => Operation::Assignment,
        SemanticOperation::Inside => Operation::Inside,
        SemanticOperation::AssignmentPattern => Operation::AssignmentPattern,
        SemanticOperation::MinTypMax => Operation::MinTypMax,
        SemanticOperation::MultiAssignmentPattern => Operation::MultiAssignmentPattern,
        SemanticOperation::List => Operation::List,
        SemanticOperation::AssertionAnd
        | SemanticOperation::AssertionOr
        | SemanticOperation::AssertionIntersect
        | SemanticOperation::AssertionThroughout
        | SemanticOperation::AssertionWithin
        | SemanticOperation::AssertionIff
        | SemanticOperation::AssertionUntil
        | SemanticOperation::AssertionSUntil
        | SemanticOperation::AssertionUntilWith
        | SemanticOperation::AssertionSUntilWith
        | SemanticOperation::AssertionImplies
        | SemanticOperation::AssertionOverlappedImplies
        | SemanticOperation::AssertionNonOverlappedImplies
        | SemanticOperation::AssertionOverlappedFollowedBy
        | SemanticOperation::AssertionNonOverlappedFollowedBy
        | SemanticOperation::AssertionNot
        | SemanticOperation::AssertionNextTime
        | SemanticOperation::AssertionSNextTime
        | SemanticOperation::AssertionAlways
        | SemanticOperation::AssertionSAlways
        | SemanticOperation::AssertionEventually
        | SemanticOperation::AssertionSEventually => Operation::Null,
    }
}

pub(super) fn time_exponent(
    scale: Option<SemanticTimeScale>,
    precision: bool,
) -> Result<i32, DbError> {
    let Some(scale) = scale else {
        return Ok(if precision { -12 } else { -9 });
    };
    let (unit, magnitude) = if precision {
        (scale.precision_unit, scale.precision_magnitude)
    } else {
        (scale.unit, scale.magnitude)
    };
    let base = match unit {
        SemanticTimeUnit::Seconds => 0,
        SemanticTimeUnit::Milliseconds => -3,
        SemanticTimeUnit::Microseconds => -6,
        SemanticTimeUnit::Nanoseconds => -9,
        SemanticTimeUnit::Picoseconds => -12,
        SemanticTimeUnit::Femtoseconds => -15,
    };
    let offset = match magnitude {
        1 => 0,
        10 => 1,
        100 => 2,
        _ => {
            return Err(DbError::InvalidSnapshot(
                "invalid Slang time scale magnitude".into(),
            ))
        }
    };
    Ok(base + offset)
}

pub(super) fn time_literal_scale(scale: Option<SemanticTimeScale>) -> Option<TimeLiteralScale> {
    scale.map(|scale| TimeLiteralScale {
        unit: match scale.unit {
            SemanticTimeUnit::Seconds => TimeUnit::Seconds,
            SemanticTimeUnit::Milliseconds => TimeUnit::Milliseconds,
            SemanticTimeUnit::Microseconds => TimeUnit::Microseconds,
            SemanticTimeUnit::Nanoseconds => TimeUnit::Nanoseconds,
            SemanticTimeUnit::Picoseconds => TimeUnit::Picoseconds,
            SemanticTimeUnit::Femtoseconds => TimeUnit::Femtoseconds,
        },
        magnitude: scale.magnitude,
    })
}

pub(super) fn net_type_from_subkind(subkind: u32) -> NetType {
    match subkind {
        128 => NetType::Wire,
        129 => NetType::Wand,
        130 => NetType::Wor,
        131 => NetType::Tri,
        132 => NetType::TriAnd,
        133 => NetType::TriOr,
        134 => NetType::Tri0,
        135 => NetType::Tri1,
        136 => NetType::TriReg,
        137 => NetType::Supply0,
        138 => NetType::Supply1,
        139 => NetType::Uwire,
        140 | 141 => NetType::Unsupported,
        _ => NetType::Unsupported,
    }
}

pub(super) fn primitive_type_from_subkind(subkind: u32) -> PrimitiveType {
    match subkind {
        200 => PrimitiveType::And,
        201 => PrimitiveType::Nand,
        202 => PrimitiveType::Nor,
        203 => PrimitiveType::Or,
        204 => PrimitiveType::Xor,
        205 => PrimitiveType::Xnor,
        206 => PrimitiveType::Buf,
        207 => PrimitiveType::Not,
        208 => PrimitiveType::Bufif0,
        209 => PrimitiveType::Bufif1,
        210 => PrimitiveType::Notif0,
        211 => PrimitiveType::Notif1,
        212 => PrimitiveType::Nmos,
        213 => PrimitiveType::Pmos,
        214 => PrimitiveType::Cmos,
        215 => PrimitiveType::Rnmos,
        216 => PrimitiveType::Rpmos,
        217 => PrimitiveType::Rcmos,
        218 => PrimitiveType::Rtran,
        219 => PrimitiveType::Rtranif0,
        220 => PrimitiveType::Rtranif1,
        221 => PrimitiveType::Tran,
        222 => PrimitiveType::Tranif0,
        223 => PrimitiveType::Tranif1,
        224 => PrimitiveType::Pullup,
        225 => PrimitiveType::Pulldown,
        226 => PrimitiveType::Sequential,
        227 => PrimitiveType::Combinational,
        _ => PrimitiveType::Unsupported,
    }
}

pub(super) fn strength_from_slang(strength: SemanticDriveStrength) -> Strength {
    match strength {
        SemanticDriveStrength::Unspecified => Strength::Unspecified,
        SemanticDriveStrength::Supply => Strength::Supply,
        SemanticDriveStrength::Strong => Strength::Strong,
        SemanticDriveStrength::Pull => Strength::Pull,
        SemanticDriveStrength::Weak => Strength::Weak,
        SemanticDriveStrength::HighZ => Strength::HighZ,
    }
}
