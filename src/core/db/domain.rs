//! Frontend-independent SystemVerilog semantic categories.

macro_rules! semantic_enum {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($variant,)* Unsupported }
    };
}

semantic_enum! { pub enum Direction { Input, Output, Inout, Mixed, None, Ref } }
semantic_enum! { pub enum NetType { Wire, Wand, Wor, Tri, Tri0, Tri1, TriReg, TriAnd, TriOr, Supply1, Supply0, None, Uwire, Logic, Reg } }
semantic_enum! { pub enum PrimitiveType { And, Nand, Nor, Or, Xor, Xnor, Buf, Not, Bufif0, Bufif1, Notif0, Notif1, Nmos, Pmos, Cmos, Rnmos, Rpmos, Rcmos, Rtran, Rtranif0, Rtranif1, Tran, Tranif0, Tranif1, Pullup, Pulldown, Sequential, Combinational } }
semantic_enum! { pub enum Strength { Unspecified, Supply, Strong, Pull, Weak, Large, Medium, Small, HighZ } }
semantic_enum! { pub enum AlwaysKind { Always, Comb, FlipFlop, Latch } }
semantic_enum! { pub enum CaseKind { Exact, X, Z, Inside } }
semantic_enum! { pub enum JoinKind { All, None, Any } }
semantic_enum! { pub enum ConstantType { Decimal, Real, Binary, Octal, Hex, String, Integer, Time, UnsignedInteger, Unbounded, Null } }

semantic_enum! {
    pub enum Operation {
        UnaryMinus, UnaryPlus, LogicalNot, BitwiseNot,
        ReductionAnd, ReductionNand, ReductionOr, ReductionNor, ReductionXor, ReductionXnor,
        Subtract, Divide, Modulo, Equal, NotEqual, CaseEqual, CaseNotEqual,
        Greater, GreaterEqual, Less, LessEqual, ShiftLeft, ShiftRight, Add, Multiply,
        LogicalAnd, LogicalOr, BitwiseAnd, BitwiseOr, BitwiseXor, BitwiseXnor,
        Conditional, Concat, MultiConcat, EventOr, Null, List, MinTypMax, Posedge, Negedge,
        ArithmeticShiftLeft, ArithmeticShiftRight, Power, Imply, NonOverlapImply, OverlapImply,
        UnaryCycleDelay, CycleDelay, Intersect, FirstMatch, Throughout, Within, Repeat,
        ConsecutiveRepeat, GotoRepeat, PostIncrement, PreIncrement, PostDecrement, PreDecrement,
        Match, Cast, Iff, WildEqual, WildNotEqual, StreamLeftToRight, StreamRightToLeft,
        Matched, Triggered, AssignmentPattern, MultiAssignmentPattern, If, IfElse,
        CompositeAnd, CompositeOr, Type, Assignment, AcceptOn, RejectOn, SyncAcceptOn,
        SyncRejectOn, OverlapFollowedBy, NonOverlapFollowedBy, Nexttime, Always, Eventually,
        Until, UntilWith, Implies, Inside, Coverage
    }
}

semantic_enum! {
    pub enum ObjectType {
        UnsupportedStatement, ReturnStatement, RepeatControl, OrderedWait, ForeachStatement,
        ExpectStatement, ImmediateAssert, ImmediateAssume, ImmediateCover
    }
}
