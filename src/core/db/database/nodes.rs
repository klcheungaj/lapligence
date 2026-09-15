//! Nodes.

use super::*;

#[derive(Debug)]
pub struct Node {
    pub kind: NodeKind,
    /// Captured child objects, in capture order.
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub name: String,
    pub full_name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Node {
    pub fn kind(&self) -> &NodeKind {
        &self.kind
    }

    pub fn children(&self) -> &[NodeId] {
        &self.children
    }

    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    pub const fn line(&self) -> u32 {
        self.line
    }

    pub const fn column(&self) -> u32 {
        self.col
    }

    pub const fn end_line(&self) -> u32 {
        self.end_line
    }

    pub const fn end_column(&self) -> u32 {
        self.end_col
    }
}

/// What a captured node is.
#[derive(Debug)]
pub enum NodeKind {
    ModuleInst {
        def_name: String,
        is_top: bool,
        /// `true` when the instance is an interface (`interface_inst`) rather
        /// than a module.
        is_interface: bool,
        timeunit: i32,
        timeprecision: i32,
    },
    Package,
    ClassDef,
    Port {
        direction: Direction,
        ty: TypeInfo,
        /// Strength endpoints declared on an output/inout port.  Keep these
        /// on the owned node so collapsed net groups do not lose the port's
        /// contribution when the frontend binding is projected away.
        strength0: Strength,
        strength1: Strength,
        high: Option<NodeId>,
        low: Option<NodeId>,
        high_expr: Option<NodeId>,
        high_present: bool,
        high_open: bool,
    },
    ModPort,
    IoDecl {
        direction: Direction,
        expr: Option<NodeId>,
    },
    IfaceConn {
        /// Arena node of the actual interface instance being connected.
        actual: NodeId,
        /// Connected modport name, or `""` for a bare interface port.
        modport: String,
    },
    Net {
        ty: TypeInfo,
        net_type: NetType,
        /// Drive-strength endpoints declared on the net, used when the net
        /// is exposed through an output port or collapsed structural path.
        strength0: Strength,
        strength1: Strength,
    },
    /// A structural SystemVerilog `alias` declaration. The expressions are
    /// retained in source order so lowering can validate and merge their
    /// canonical net identities before executable children are collected.
    NetAlias {
        nets: Vec<NodeId>,
    },
    Var {
        ty: TypeInfo,
    },
    /// Elaboration-only generate-loop variable; never runtime storage.
    Genvar {
        ty: TypeInfo,
    },
    Array {
        ty: TypeInfo,
    },
    NamedEvent,
    Param {
        ty: TypeInfo,
        value: Option<Val>,
        local: bool,
    },
    ParamAssign {
        overridden: bool,
    },
    /// Slang's container for an array of elaborated instances. Concrete
    /// instance entries are normalized into the enclosing instance hierarchy.
    InstanceArray,
    GenScopeArray,
    GenScope,
    Process {
        kind: ProcessKind,
    },
    ContAssign {
        net_decl: bool,
        delay: Option<DriverDelay>,
        /// Drive strengths retained so consumers can reject unsupported
        /// strength-aware resolution instead of silently treating it as
        /// equal-strength.
        strength0: Strength,
        strength1: Strength,
    },
    Gate {
        class: PrimClass,
        prim_type: PrimitiveType,
        strength0: Strength,
        strength1: Strength,
        delay: Option<DriverDelay>,
        terms: Vec<GateTerm>,
    },
    Stmt(StmtKind),
    AssertionExpr(AssertionExprKind),
    Expr(ExprKind),
    SysCall {
        name: String,
    },
    /// Built-in object method. `receiver` is authoritative; structural
    /// children retain captured receiver, argument, and with-clause expressions
    /// without a positional guarantee.
    MethodCall {
        name: String,
        receiver: Option<NodeId>,
        /// Arena node of the resolved method callee, when Slang exposed it.
        /// Keeping this identity separate from the receiver lets simulator
        /// lowering bind class methods without reconstructing a name lookup.
        callee: Option<NodeId>,
    },
    FuncCall {
        name: String,
        /// `true` for a `task_call` (statement), `false` for a `func_call`.
        is_task: bool,
        /// `true` when the call uses the `super` qualifier and must bind to
        /// the declaring base implementation.
        is_super: bool,
        /// Arena node of the callee [`NodeKind::FuncTask`] (the per-instance
        /// clone), when it was already captured when the call site was walked.
        callee: Option<NodeId>,
    },
    FuncTask {
        is_task: bool,
        automatic: bool,
        /// `true` for a class method declared with the `static` qualifier.
        is_static: bool,
        /// `true` when the method participates in virtual dispatch.
        is_virtual: bool,
        /// `true` for a pure virtual method with no implementation.
        is_pure: bool,
        /// `true` when a virtual method forbids overrides in derived classes.
        is_final: bool,
        /// `true` for a class constructor (`new`).
        is_constructor: bool,
        /// Return type, `None` for void functions and tasks.
        ret: Option<TypeInfo>,
        /// Exact executable body attached by Slang.
        body: Option<NodeId>,
    },
    FuncArg {
        direction: Direction,
        ty: TypeInfo,
        default: Option<NodeId>,
        /// `true` for a `const ref` formal.
        const_ref: bool,
        /// `true` for a `ref static` formal.
        ref_static: bool,
    },
    EnumConst {
        value: Option<Val>,
    },
    /// Anything the walk does not model explicitly (never fails the build).
    Other,
}

#[derive(Debug)]
pub enum ProcessKind {
    Always { always_type: AlwaysKind },
    Initial,
    Final,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimClass {
    Gate,
    Switch,
    Udp,
    Array,
}

/// One terminal of a [`NodeKind::Gate`].
#[derive(Clone, Debug)]
pub struct GateTerm {
    pub direction: Direction,
    pub term_index: i32,
    pub expr: NodeId,
}

impl GateTerm {
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    pub const fn term_index(&self) -> i32 {
        self.term_index
    }

    pub const fn expression(&self) -> NodeId {
        self.expr
    }
}

/// One `case` item: the item expressions plus the (optional) body statement.
#[derive(Debug)]
pub struct CaseItem {
    pub exprs: Vec<NodeId>,
    pub body: Option<NodeId>,
}

impl CaseItem {
    pub fn expressions(&self) -> &[NodeId] {
        &self.exprs
    }

    pub const fn body(&self) -> Option<NodeId> {
        self.body
    }
}
