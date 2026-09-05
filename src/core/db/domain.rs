//! Owned interpretations of integer-valued VPI properties.
//!
//! VPI is an extensible C interface, so every enum retains values introduced
//! by newer UHDM versions as `Unknown` instead of silently assigning them a
//! known meaning.

use crate::ffi::vpi;

macro_rules! vpi_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $($variant:ident = $value:path,)* }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($variant,)*
            Unknown(i32),
        }

        impl $name {
            pub const fn from_raw(raw: i32) -> Self {
                match raw {
                    $($value => Self::$variant,)*
                    other => Self::Unknown(other),
                }
            }

            pub const fn as_raw(self) -> i32 {
                match self {
                    $(Self::$variant => $value,)*
                    Self::Unknown(raw) => raw,
                }
            }
        }

        impl PartialEq<i32> for $name {
            fn eq(&self, other: &i32) -> bool {
                self.as_raw() == *other
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.as_raw().fmt(f)
            }
        }
    };
}

vpi_enum! {
    /// Direction reported by `vpiDirection`.
    pub enum Direction {
        Input = vpi::vpiInput,
        Output = vpi::vpiOutput,
        Inout = vpi::vpiInout,
        Mixed = vpi::vpiMixedIO,
        None = vpi::vpiNoDirection,
        Ref = vpi::vpiRef,
    }
}

vpi_enum! {
    /// Net flavour reported by `vpiNetType`.
    pub enum NetType {
        Wire = vpi::vpiWire,
        Wand = vpi::vpiWand,
        Wor = vpi::vpiWor,
        Tri = vpi::vpiTri,
        Tri0 = vpi::vpiTri0,
        Tri1 = vpi::vpiTri1,
        TriReg = vpi::vpiTriReg,
        TriAnd = vpi::vpiTriAnd,
        TriOr = vpi::vpiTriOr,
        Supply1 = vpi::vpiSupply1,
        Supply0 = vpi::vpiSupply0,
        None = vpi::vpiNone,
        Uwire = vpi::vpiUwire,
        Logic = vpi::vpiLogicNet,
        Reg = vpi::vpiReg,
    }
}

vpi_enum! {
    /// Structural primitive flavour reported by `vpiPrimType`.
    pub enum PrimitiveType {
        And = vpi::vpiAndPrim,
        Nand = vpi::vpiNandPrim,
        Nor = vpi::vpiNorPrim,
        Or = vpi::vpiOrPrim,
        Xor = vpi::vpiXorPrim,
        Xnor = vpi::vpiXnorPrim,
        Buf = vpi::vpiBufPrim,
        Not = vpi::vpiNotPrim,
        Bufif0 = vpi::vpiBufif0Prim,
        Bufif1 = vpi::vpiBufif1Prim,
        Notif0 = vpi::vpiNotif0Prim,
        Notif1 = vpi::vpiNotif1Prim,
        Nmos = vpi::vpiNmosPrim,
        Pmos = vpi::vpiPmosPrim,
        Cmos = vpi::vpiCmosPrim,
        Rnmos = vpi::vpiRnmosPrim,
        Rpmos = vpi::vpiRpmosPrim,
        Rcmos = vpi::vpiRcmosPrim,
        Rtran = vpi::vpiRtranPrim,
        Rtranif0 = vpi::vpiRtranif0Prim,
        Rtranif1 = vpi::vpiRtranif1Prim,
        Tran = vpi::vpiTranPrim,
        Tranif0 = vpi::vpiTranif0Prim,
        Tranif1 = vpi::vpiTranif1Prim,
        Pullup = vpi::vpiPullupPrim,
        Pulldown = vpi::vpiPulldownPrim,
        Sequential = vpi::vpiSeqPrim,
        Combinational = vpi::vpiCombPrim,
    }
}

/// Drive or charge strength reported by `vpiStrength0/1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Strength {
    Unspecified,
    Supply,
    Strong,
    Pull,
    Weak,
    Large,
    Medium,
    Small,
    HighZ,
    Unknown(i32),
}

impl Strength {
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::Unspecified,
            vpi::vpiSupplyDrive => Self::Supply,
            vpi::vpiStrongDrive => Self::Strong,
            vpi::vpiPullDrive => Self::Pull,
            vpi::vpiWeakDrive => Self::Weak,
            vpi::vpiLargeCharge => Self::Large,
            vpi::vpiMediumCharge => Self::Medium,
            vpi::vpiSmallCharge => Self::Small,
            vpi::vpiHiZ => Self::HighZ,
            other => Self::Unknown(other),
        }
    }

    pub const fn as_raw(self) -> i32 {
        match self {
            Self::Unspecified => 0,
            Self::Supply => vpi::vpiSupplyDrive,
            Self::Strong => vpi::vpiStrongDrive,
            Self::Pull => vpi::vpiPullDrive,
            Self::Weak => vpi::vpiWeakDrive,
            Self::Large => vpi::vpiLargeCharge,
            Self::Medium => vpi::vpiMediumCharge,
            Self::Small => vpi::vpiSmallCharge,
            Self::HighZ => vpi::vpiHiZ,
            Self::Unknown(raw) => raw,
        }
    }
}

impl PartialEq<i32> for Strength {
    fn eq(&self, other: &i32) -> bool {
        self.as_raw() == *other
    }
}

impl std::fmt::Display for Strength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_raw().fmt(f)
    }
}

vpi_enum! {
    /// `always` procedure subtype reported by `vpiAlwaysType`.
    pub enum AlwaysKind {
        Always = vpi::vpiAlways,
        Comb = vpi::vpiAlwaysComb,
        FlipFlop = vpi::vpiAlwaysFF,
        Latch = vpi::vpiAlwaysLatch,
    }
}

vpi_enum! {
    /// Case matching mode reported by `vpiCaseType`.
    pub enum CaseKind {
        Exact = vpi::vpiCaseExact,
        X = vpi::vpiCaseX,
        Z = vpi::vpiCaseZ,
    }
}

vpi_enum! {
    /// Fork completion mode reported by `vpiJoinType`.
    pub enum JoinKind {
        All = vpi::vpiJoin,
        None = vpi::vpiJoinNone,
        Any = vpi::vpiJoinAny,
    }
}

vpi_enum! {
    /// Literal representation reported by `vpiConstType`.
    pub enum ConstantType {
        Decimal = vpi::vpiDecConst,
        Real = vpi::vpiRealConst,
        Binary = vpi::vpiBinaryConst,
        Octal = vpi::vpiOctConst,
        Hex = vpi::vpiHexConst,
        String = vpi::vpiStringConst,
        Integer = vpi::vpiIntConst,
        Time = vpi::vpiTimeConst,
        UnsignedInteger = vpi::vpiUIntConst,
        Unbounded = vpi::vpiUnboundedConst,
        Null = vpi::vpiNullConst,
    }
}

vpi_enum! {
    /// Operation reported by `vpiOpType`.
    pub enum Operation {
        UnaryMinus = vpi::vpiMinusOp,
        UnaryPlus = vpi::vpiPlusOp,
        LogicalNot = vpi::vpiNotOp,
        BitwiseNot = vpi::vpiBitNegOp,
        ReductionAnd = vpi::vpiUnaryAndOp,
        ReductionNand = vpi::vpiUnaryNandOp,
        ReductionOr = vpi::vpiUnaryOrOp,
        ReductionNor = vpi::vpiUnaryNorOp,
        ReductionXor = vpi::vpiUnaryXorOp,
        ReductionXnor = vpi::vpiUnaryXNorOp,
        Subtract = vpi::vpiSubOp,
        Divide = vpi::vpiDivOp,
        Modulo = vpi::vpiModOp,
        Equal = vpi::vpiEqOp,
        NotEqual = vpi::vpiNeqOp,
        CaseEqual = vpi::vpiCaseEqOp,
        CaseNotEqual = vpi::vpiCaseNeqOp,
        Greater = vpi::vpiGtOp,
        GreaterEqual = vpi::vpiGeOp,
        Less = vpi::vpiLtOp,
        LessEqual = vpi::vpiLeOp,
        ShiftLeft = vpi::vpiLShiftOp,
        ShiftRight = vpi::vpiRShiftOp,
        Add = vpi::vpiAddOp,
        Multiply = vpi::vpiMultOp,
        LogicalAnd = vpi::vpiLogAndOp,
        LogicalOr = vpi::vpiLogOrOp,
        BitwiseAnd = vpi::vpiBitAndOp,
        BitwiseOr = vpi::vpiBitOrOp,
        BitwiseXor = vpi::vpiBitXorOp,
        BitwiseXnor = vpi::vpiBitXNorOp,
        Conditional = vpi::vpiConditionOp,
        Concat = vpi::vpiConcatOp,
        MultiConcat = vpi::vpiMultiConcatOp,
        EventOr = vpi::vpiEventOrOp,
        Null = vpi::vpiNullOp,
        List = vpi::vpiListOp,
        MinTypMax = vpi::vpiMinTypMaxOp,
        Posedge = vpi::vpiPosedgeOp,
        Negedge = vpi::vpiNegedgeOp,
        ArithmeticShiftLeft = vpi::vpiArithLShiftOp,
        ArithmeticShiftRight = vpi::vpiArithRShiftOp,
        Power = vpi::vpiPowerOp,
        Imply = vpi::vpiImplyOp,
        NonOverlapImply = vpi::vpiNonOverlapImplyOp,
        OverlapImply = vpi::vpiOverlapImplyOp,
        UnaryCycleDelay = vpi::vpiUnaryCycleDelayOp,
        CycleDelay = vpi::vpiCycleDelayOp,
        Intersect = vpi::vpiIntersectOp,
        FirstMatch = vpi::vpiFirstMatchOp,
        Throughout = vpi::vpiThroughoutOp,
        Within = vpi::vpiWithinOp,
        Repeat = vpi::vpiRepeatOp,
        ConsecutiveRepeat = vpi::vpiConsecutiveRepeatOp,
        GotoRepeat = vpi::vpiGotoRepeatOp,
        PostIncrement = vpi::vpiPostIncOp,
        PreIncrement = vpi::vpiPreIncOp,
        PostDecrement = vpi::vpiPostDecOp,
        PreDecrement = vpi::vpiPreDecOp,
        Match = vpi::vpiMatchOp,
        Cast = vpi::vpiCastOp,
        Iff = vpi::vpiIffOp,
        WildEqual = vpi::vpiWildEqOp,
        WildNotEqual = vpi::vpiWildNeqOp,
        StreamLeftToRight = vpi::vpiStreamLROp,
        StreamRightToLeft = vpi::vpiStreamRLOp,
        Matched = vpi::vpiMatchedOp,
        Triggered = vpi::vpiTriggeredOp,
        AssignmentPattern = vpi::vpiAssignmentPatternOp,
        MultiAssignmentPattern = vpi::vpiMultiAssignmentPatternOp,
        If = vpi::vpiIfOp,
        IfElse = vpi::vpiIfElseOp,
        CompositeAnd = vpi::vpiCompAndOp,
        CompositeOr = vpi::vpiCompOrOp,
        Type = vpi::vpiTypeOp,
        Assignment = vpi::vpiAssignmentOp,
        AcceptOn = vpi::vpiAcceptOnOp,
        RejectOn = vpi::vpiRejectOnOp,
        SyncAcceptOn = vpi::vpiSyncAcceptOnOp,
        SyncRejectOn = vpi::vpiSyncRejectOnOp,
        OverlapFollowedBy = vpi::vpiOverlapFollowedByOp,
        NonOverlapFollowedBy = vpi::vpiNonOverlapFollowedByOp,
        Nexttime = vpi::vpiNexttimeOp,
        Always = vpi::vpiAlwaysOp,
        Eventually = vpi::vpiEventuallyOp,
        Until = vpi::vpiUntilOp,
        UntilWith = vpi::vpiUntilWithOp,
        Implies = vpi::vpiImpliesOp,
        Inside = vpi::vpiInsideOp,
        Coverage = vpi::vpiCoverageStOp,
    }
}

impl Operation {
    pub const fn is(self, raw: i32) -> bool {
        self.as_raw() == raw
    }
}

vpi_enum! {
    /// VPI object type retained for a recognized but unsupported statement.
    pub enum ObjectType {
        UnsupportedStatement = vpi::vpiUnsupportedStmt,
        ReturnStatement = vpi::vpiReturnStmt,
        RepeatControl = vpi::vpiRepeatControl,
        OrderedWait = vpi::vpiOrderedWait,
        ForeachStatement = vpi::vpiForeachStmt,
        ExpectStatement = vpi::vpiExpectStmt,
        ImmediateAssert = vpi::vpiImmediateAssert,
        ImmediateAssume = vpi::vpiImmediateAssume,
        ImmediateCover = vpi::vpiImmediateCover,
    }
}

impl ObjectType {
    pub(crate) fn from_statement_raw(raw: i32) -> Self {
        Self::from_raw(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_values_round_trip() {
        assert_eq!(NetType::from_raw(99), NetType::Unknown(99));
        assert_eq!(Operation::from_raw(777).as_raw(), 777);
        assert_eq!(Direction::from_raw(-1).as_raw(), -1);
    }
}
