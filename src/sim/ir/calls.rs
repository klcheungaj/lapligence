//! Calls.

use super::*;

/// Structural call arguments shared by statement-position and
/// expression-position calls.  Input arguments arrive already converted to
/// the formal's width/signedness (defaults substituted at lowering).
#[derive(Clone, Debug, PartialEq)]
pub enum IrCallArg {
    /// Input formal value.
    Val(IrExpr),
    /// Input native string formal. String bytes stay owned and typed.
    StringVal(IrStringExpr),
    /// Input chandle formal value.  Chandles remain native opaque pointers;
    /// they are never reinterpreted as packed storage.
    ChandleVal(IrChandleExpr),
    /// Output/inout chandle formal bound to a caller-owned `void **`.
    ChandleAddr(String),
    /// Chandle `ref` formal bound to a caller-owned pointer slot.
    ChandleRefAddr(String),
    /// Output/inout native string formal bound to a caller-owned slot.
    StringOutAddr(String),
    /// Native string `ref` formal bound to a whole caller-owned slot.
    StringRefAddr { addr: String, const_ref: bool },
    /// Output/inout formal bound to a direct C address (statement calls):
    /// `&G_sig`, a whole-reference address (`o0`, `&_l0`) or a caller-side
    /// temp declared separately (`&_t5`); passed to the callee verbatim.
    OutAddr(String),
    /// Reference formal bound to a canonical lvalue descriptor.  The string
    /// is a complete `llg_ref_t*` expression and remains valid for the call;
    /// the remaining fields retain the checked actual shape in typed IR.
    RefAddr {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        const_ref: bool,
        /// Typed caller-side lvalue used for dependency and write analysis.
        lhs: Box<IrLhs>,
        /// Caller-side read used for dependency/effect analysis. The C
        /// descriptor evaluates the address separately at the call boundary.
        read: Box<IrExpr>,
    },
    /// Output/inout formal bound to a caller-side temp inside an
    /// expression-position GNU statement expression.  `init` is `None` for
    /// outputs (all-X temp sized by the formal's type) and the actual's
    /// current value for inouts; `writeback` copies the temp back into the
    /// actual after the call.
    OutTemp {
        name: String,
        init: Option<Box<IrExpr>>,
        writeback: Box<IrLhs>,
        /// Optional persistent formal storage used by a static function call.
        /// The caller-side temp still stages the actual value, while the C
        /// call receives this address and the value is copied back from the
        /// typed storage after return.
        storage_addr: Option<String>,
        storage_lhs: Option<Box<IrLhs>>,
        storage_read: Option<Box<IrExpr>>,
        /// Caller-side selector values captured before the callee runs. Each
        /// tuple is `(name, width, signed, two_state, initializer)`.
        selector_inits: Vec<(String, u32, bool, bool, IrExpr)>,
    },
    /// Caller-side temp for an output/inout native string formal.
    StringOutTemp {
        name: String,
        init: Option<Box<IrStringExpr>>,
        writeback: String,
        storage_addr: Option<String>,
        storage_read: Option<Box<IrStringExpr>>,
    },
}

/// A function/task call used in expression position (functions only): rendered
/// as a plain call when no output formal needs a temp, otherwise as one GNU
/// statement expression `({ temps; call/writeback chain; result })`.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCallExpr {
    /// Callee index into [`IrModel::funcs`].
    pub(in crate::sim) f: usize,
    pub(in crate::sim) args: Vec<IrCallArg>,
    /// Recursion depth argument at the call site.
    pub(in crate::sim) depth: IrDepth,
    /// Optional hidden receiver passed before ordinary method formals.
    pub(in crate::sim) receiver: Option<IrChandleExpr>,
    /// Dispatch through the callee's virtual slot using the runtime class id.
    pub(in crate::sim) virtual_dispatch: bool,
    /// Optional virtual-interface dispatch metadata. The receiver is kept
    /// separate from class receivers because the generated dispatcher owns
    /// the rebinding lookup rather than a concrete function body.
    pub(in crate::sim) virtual_call: Option<IrVirtualCall>,
    /// Void callee used as a value: yield all-X (warning issued at lowering).
    pub(in crate::sim) void_x: bool,
}

impl IrCallExpr {
    /// Create a call-expression staging value. The containing model validates
    /// the function index, arguments, and formal bindings.
    pub fn new(f: usize, args: Vec<IrCallArg>, depth: IrDepth, void_x: bool) -> Self {
        Self {
            f,
            args,
            depth,
            receiver: None,
            virtual_dispatch: false,
            virtual_call: None,
            void_x,
        }
    }

    /// Bind a class/object receiver to this call while preserving the normal
    /// typed formal ABI for the remaining arguments.
    pub fn with_receiver(mut self, receiver: IrChandleExpr) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_virtual_call(mut self, virtual_call: IrVirtualCall) -> Self {
        self.virtual_call = Some(virtual_call);
        self
    }

    pub fn function_index(&self) -> usize {
        self.f
    }
    pub fn args(&self) -> &[IrCallArg] {
        &self.args
    }
    pub fn depth(&self) -> IrDepth {
        self.depth
    }
    pub fn yields_x_for_void(&self) -> bool {
        self.void_x
    }
}

/// A function/task call used in statement position: the temp declarations
/// for select/array-element output actuals (`sv4_t _aN = …`), the C call
/// itself, and the copy-out assignments back into the actuals — one emission
/// unit rendered as a single indented block.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCall {
    pub(in crate::sim) f: usize,
    pub(in crate::sim) args: Vec<IrCallArg>,
    pub(in crate::sim) depth: IrDepth,
    /// Optional hidden receiver passed before ordinary method formals.
    pub(in crate::sim) receiver: Option<IrChandleExpr>,
    /// Dispatch through the callee's virtual slot using the runtime class id.
    pub(in crate::sim) virtual_dispatch: bool,
    /// Optional virtual-interface dispatch metadata.
    pub(in crate::sim) virtual_call: Option<IrVirtualCall>,
    /// `(temp name, formal index, init)` triples declared right before the
    /// call; `init` is `None` for outputs (all-X temp sized by the formal)
    /// and the actual's current value for inouts.
    pub(in crate::sim) temps: Vec<(String, usize, Option<IrExpr>)>,
    /// Copy-outs after the call: `(actual LHS, temp name, width, signed)`.
    pub(in crate::sim) copyouts: Vec<(IrLhs, String, u32, bool)>,
}

impl IrCall {
    /// Create a statement-call staging value for validation by its model.
    pub fn new(
        f: usize,
        args: Vec<IrCallArg>,
        depth: IrDepth,
        temps: Vec<(String, usize, Option<IrExpr>)>,
        copyouts: Vec<(IrLhs, String, u32, bool)>,
    ) -> Self {
        Self {
            f,
            args,
            depth,
            receiver: None,
            virtual_dispatch: false,
            virtual_call: None,
            temps,
            copyouts,
        }
    }

    /// Bind a class/object receiver to this statement call.
    pub fn with_receiver(mut self, receiver: IrChandleExpr) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_virtual_call(mut self, virtual_call: IrVirtualCall) -> Self {
        self.virtual_call = Some(virtual_call);
        self
    }

    pub fn function_index(&self) -> usize {
        self.f
    }
    pub fn args(&self) -> &[IrCallArg] {
        &self.args
    }
    pub fn depth(&self) -> IrDepth {
        self.depth
    }
    pub fn temps(&self) -> &[(String, usize, Option<IrExpr>)] {
        &self.temps
    }
    pub fn copyouts(&self) -> &[(IrLhs, String, u32, bool)] {
        &self.copyouts
    }
}

/// Dynamic dispatch metadata for a call through a virtual-interface handle.
/// `function` on the descriptor is still used for the checked formal ABI;
/// emission selects the concrete implementation from the receiver environment.
#[derive(Clone, Debug, PartialEq)]
pub struct IrVirtualCall {
    pub(in crate::sim) interface: usize,
    pub(in crate::sim) method: usize,
    pub(in crate::sim) receiver: IrChandleExpr,
}

/// Recursion depth argument of a call site: `"0"` in process contexts,
/// `"depth + 1"` inside C function bodies, wrapped once per enclosing inlined
/// task (`(0) + 1`, `(depth + 1) + 1`, …) exactly like the pre-IR emitter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrDepth {
    /// `true` when the innermost non-inline scope is a C function body.
    pub(in crate::sim) func_base: bool,
    /// Number of enclosing inlined task bodies.
    pub(in crate::sim) nest: u32,
}

impl IrDepth {
    pub const PROC: IrDepth = IrDepth {
        func_base: false,
        nest: 0,
    };
    pub const FUNC: IrDepth = IrDepth {
        func_base: true,
        nest: 0,
    };

    /// One more level of task inlining around this context.
    pub fn inline(self) -> IrDepth {
        IrDepth {
            nest: self.nest + 1,
            ..self
        }
    }

    /// The C expression spelling this depth.
    pub fn code(&self) -> String {
        let mut s = if self.func_base {
            "depth + 1".to_string()
        } else {
            "0".to_string()
        };
        for _ in 0..self.nest {
            s = format!("({s}) + 1");
        }
        s
    }

    pub fn is_function_based(&self) -> bool {
        self.func_base
    }

    pub fn inline_nesting(&self) -> u32 {
        self.nest
    }
}
