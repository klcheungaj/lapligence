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

/// Frontend-neutral category retained for coverage of semantic records whose
/// normalized [`NodeKind`](crate::core::db::NodeKind) has no direct variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CapturedSemanticKind {
    Instance,
    Package,
    Class,
    GenerateScope,
    Port,
    Modport,
    InterfaceConnection,
    Net,
    Variable,
    Array,
    NamedEvent,
    Parameter,
    Process,
    ContinuousAssign,
    Primitive,
    Subroutine,
    Argument,
    Statement,
    Expression,
    SystemCall,
    MethodCall,
    FunctionCall,
    EnumConstant,
    Definition,
    Scope,
    TimingControl,
    Unsupported,
}

impl From<crate::ffi::slang::SemanticKind> for CapturedSemanticKind {
    fn from(kind: crate::ffi::slang::SemanticKind) -> Self {
        match kind {
            crate::ffi::slang::SemanticKind::Instance => Self::Instance,
            crate::ffi::slang::SemanticKind::Package => Self::Package,
            crate::ffi::slang::SemanticKind::Class => Self::Class,
            crate::ffi::slang::SemanticKind::GenerateScope => Self::GenerateScope,
            crate::ffi::slang::SemanticKind::Port => Self::Port,
            crate::ffi::slang::SemanticKind::Modport => Self::Modport,
            crate::ffi::slang::SemanticKind::InterfaceConnection => Self::InterfaceConnection,
            crate::ffi::slang::SemanticKind::Net => Self::Net,
            crate::ffi::slang::SemanticKind::Variable => Self::Variable,
            crate::ffi::slang::SemanticKind::Array => Self::Array,
            crate::ffi::slang::SemanticKind::NamedEvent => Self::NamedEvent,
            crate::ffi::slang::SemanticKind::Parameter => Self::Parameter,
            crate::ffi::slang::SemanticKind::Process => Self::Process,
            crate::ffi::slang::SemanticKind::ContinuousAssign => Self::ContinuousAssign,
            crate::ffi::slang::SemanticKind::Primitive => Self::Primitive,
            crate::ffi::slang::SemanticKind::Subroutine => Self::Subroutine,
            crate::ffi::slang::SemanticKind::Argument => Self::Argument,
            crate::ffi::slang::SemanticKind::Statement => Self::Statement,
            crate::ffi::slang::SemanticKind::Expression => Self::Expression,
            crate::ffi::slang::SemanticKind::SystemCall => Self::SystemCall,
            crate::ffi::slang::SemanticKind::MethodCall => Self::MethodCall,
            crate::ffi::slang::SemanticKind::FunctionCall => Self::FunctionCall,
            crate::ffi::slang::SemanticKind::EnumConstant => Self::EnumConstant,
            crate::ffi::slang::SemanticKind::Definition => Self::Definition,
            crate::ffi::slang::SemanticKind::Scope => Self::Scope,
            crate::ffi::slang::SemanticKind::TimingControl => Self::TimingControl,
            crate::ffi::slang::SemanticKind::Unsupported => Self::Unsupported,
        }
    }
}

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
