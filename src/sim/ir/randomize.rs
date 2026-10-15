//! Constrained randomization (IEEE 1800-2009 clause 18).
//!
//! A randomize call lowers into ordinary IR (handle locals, dynamic-class
//! cases, pre/post hook calls, loops) around a few runtime primitives on one
//! static problem: [`IrRandStmt`] feeds the solver session and the
//! [`super::IrSysFunc`] variants `RandStep`, `RandCallReady` and `RandValue`
//! run it and read its solution. The problem itself is pure data that the
//! emitter turns into the `llg_cr_problem_t` tables of `llg_constraint.h`.

use super::{IrExpr, IrValidationError};

/// Solver node operation codes; values match `llg_constraint.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum IrRandOp {
    Const = 1,
    Var,
    State,
    Call,
    Resize,
    Not,
    Neg,
    RedAnd,
    RedOr,
    RedXor,
    RedNand,
    RedNor,
    RedXnor,
    LogNot,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Xnor,
    Shl,
    Shr,
    Ashr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LogAnd,
    LogOr,
    Implies,
    Equiv,
    Cond,
    Concat,
    Repl,
    Slice,
    DynSlice,
    All,
    /// `a ** b` (11.4.3, Table 11-4); `b` is self-determined.
    Pow,
    /// `a` indexes [`IrRandProblem::dists`]; 1 when the expression matches
    /// an item of nonzero weight (18.5.4).
    Dist,
}

impl IrRandOp {
    pub fn code(self) -> u16 {
        self as u16
    }

    /// Operand count fixed by the operation (list operations use `args`).
    fn fixed_operands(self) -> usize {
        match self {
            Self::Const
            | Self::Var
            | Self::State
            | Self::Call
            | Self::Concat
            | Self::All
            | Self::Dist => 0,
            Self::Resize
            | Self::Not
            | Self::Neg
            | Self::RedAnd
            | Self::RedOr
            | Self::RedXor
            | Self::RedNand
            | Self::RedNor
            | Self::RedXnor
            | Self::LogNot
            | Self::Repl
            | Self::Slice => 1,
            Self::Cond => 3,
            _ => 2,
        }
    }
}

/// One solver expression node (see `llg_cr_node_t`). Operands precede
/// their node; `width`/`signed` are the node's sized result type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandNode {
    pub op: IrRandOp,
    pub signed: bool,
    pub width: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

/// A candidate random variable; `domain` is a node that must hold while the
/// variable is active (an enum's named values, S29-D2).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandVar {
    pub name: String,
    pub width: u32,
    pub signed: bool,
    pub domain: Option<u32>,
    /// `randc` (18.4.2): solved first and cycling through its values; the
    /// cycle state lives with the variable (decision S30-D3).
    pub cyclic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandBlock {
    pub name: String,
    pub root: u32,
}

/// A value the generated code evaluates before solving (18.5.12 state).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandLeaf {
    pub name: String,
    pub width: u32,
    pub signed: bool,
}

/// A function call whose arguments involve random variables (18.5.11).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandCall {
    pub name: String,
    pub width: u32,
    pub signed: bool,
    pub args: Vec<u32>,
}

/// One `dist` constraint (`llg_cr_dist_t`, 18.5.4). `guard` is the
/// conjunction of the enclosing implication and `if` conditions; the weights
/// apply where it holds (decision S30-D1).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandDist {
    pub expr: u32,
    pub guard: Option<u32>,
    pub items: Vec<IrRandDistItem>,
}

/// One dist item (`llg_cr_dist_item_t`): `matches` is the 1-bit node "the
/// expression is in the item"; `lo`/`hi` are its bounds in the comparison
/// type; `weight` is `None` for the default weight 1.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandDistItem {
    pub matches: u32,
    pub lo: u32,
    pub hi: u32,
    pub weight: Option<u32>,
    pub per_range: bool,
    pub signed: bool,
}

/// `solve before before after` from constraint block `block` (18.5.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IrRandOrder {
    pub block: u32,
    pub before: u32,
    pub after: u32,
}

/// One randomize problem: the static tables of `llg_cr_problem_t`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IrRandProblem {
    pub name: String,
    pub nodes: Vec<IrRandNode>,
    /// Operand lists of `Concat`/`All` nodes (`b` = first, `c` = count).
    pub args: Vec<u32>,
    pub consts: Vec<u64>,
    pub vars: Vec<IrRandVar>,
    pub blocks: Vec<IrRandBlock>,
    pub leaves: Vec<IrRandLeaf>,
    pub calls: Vec<IrRandCall>,
    pub dists: Vec<IrRandDist>,
    pub orders: Vec<IrRandOrder>,
}

/// Widest `randc` variable (`LLG_CR_CYCLIC_MAX_WIDTH`).
pub const RAND_CYCLIC_MAX_WIDTH: u32 = 16;

/// Packed width of a `randc` variable's cycle state
/// (`LLG_CR_CYCLIC_WORDS` 64-bit words).
pub fn rand_cyclic_state_width(width: u32) -> u32 {
    let words = if width >= 6 {
        (1 << (width - 6)) + 1
    } else {
        2
    };
    words * 64
}

fn words(width: u32) -> usize {
    width.div_ceil(64).max(1) as usize
}

impl IrRandProblem {
    /// Check the invariants the C solver also enforces, so a malformed
    /// problem fails at model construction rather than at run time.
    pub(in crate::sim) fn validate(&self, path: &str) -> Result<(), IrValidationError> {
        let fail = |message: String| Err(IrValidationError::new(path.to_owned(), message));
        let count = self.nodes.len() as u32;
        for (index, node) in self.nodes.iter().enumerate() {
            let index = index as u32;
            if node.width == 0 || node.width >= crate::sim::emit_c::LLG_WIDTH_LIMIT {
                return fail(format!("node {index} has an invalid width"));
            }
            let before = |operand: u32| operand < index;
            match node.op {
                IrRandOp::Const => {
                    if node.a as usize + words(node.width) > self.consts.len() {
                        return fail(format!("constant node {index} is out of range"));
                    }
                }
                IrRandOp::Var => {
                    let Some(var) = self.vars.get(node.a as usize) else {
                        return fail(format!("variable node {index} is out of range"));
                    };
                    if (var.width, var.signed) != (node.width, node.signed) {
                        return fail(format!("variable node {index} changes its type"));
                    }
                }
                IrRandOp::State => {
                    let Some(leaf) = self.leaves.get(node.a as usize) else {
                        return fail(format!("state node {index} is out of range"));
                    };
                    if (leaf.width, leaf.signed) != (node.width, node.signed) {
                        return fail(format!("state node {index} changes its type"));
                    }
                }
                IrRandOp::Call => {
                    let Some(call) = self.calls.get(node.a as usize) else {
                        return fail(format!("call node {index} is out of range"));
                    };
                    if call.args.iter().any(|arg| !before(*arg)) {
                        return fail(format!("call node {index} reads a later argument"));
                    }
                }
                IrRandOp::Concat | IrRandOp::All => {
                    let (first, len) = (node.b as usize, node.c as usize);
                    if first + len > self.args.len() || len == 0 {
                        return fail(format!("list node {index} is out of range"));
                    }
                    if self.args[first..first + len]
                        .iter()
                        .any(|arg| !before(*arg))
                    {
                        return fail(format!("list node {index} reads a later operand"));
                    }
                }
                IrRandOp::Dist => {
                    let Some(dist) = self.dists.get(node.a as usize) else {
                        return fail(format!("dist node {index} is out of range"));
                    };
                    let mut operands = vec![dist.expr];
                    operands.extend(dist.guard);
                    for item in &dist.items {
                        operands.extend([item.matches, item.lo, item.hi]);
                        operands.extend(item.weight);
                    }
                    if dist.items.is_empty() || operands.iter().any(|x| !before(*x)) {
                        return fail(format!("dist node {index} reads a later operand"));
                    }
                }
                IrRandOp::Repl | IrRandOp::Slice => {
                    if !before(node.a) {
                        return fail(format!("node {index} reads a later operand"));
                    }
                }
                op => {
                    let operands = [node.a, node.b, node.c];
                    if operands[..op.fixed_operands()].iter().any(|x| !before(*x)) {
                        return fail(format!("node {index} reads a later operand"));
                    }
                }
            }
        }
        for (index, var) in self.vars.iter().enumerate() {
            if var.width == 0
                || var.domain.is_some_and(|d| d >= count)
                || (var.cyclic && var.width > RAND_CYCLIC_MAX_WIDTH)
            {
                return fail(format!("variable {index} is malformed"));
            }
        }
        for (index, block) in self.blocks.iter().enumerate() {
            if block.root >= count {
                return fail(format!("block {index} root is out of range"));
            }
        }
        for (index, leaf) in self.leaves.iter().enumerate() {
            if leaf.width == 0 {
                return fail(format!("state leaf {index} has no packed width"));
            }
        }
        let vars = self.vars.len() as u32;
        for (index, order) in self.orders.iter().enumerate() {
            if order.block as usize >= self.blocks.len()
                || order.before >= vars
                || order.after >= vars
            {
                return fail(format!("solve-before order {index} is out of range"));
            }
        }
        Ok(())
    }
}

/// Statements that feed one problem's solver session. Inputs are packed
/// values; `active`/`enabled` are tested for truth.
#[derive(Clone, Debug, PartialEq)]
pub enum IrRandStmt {
    /// Start a solve: forget solved variables and evaluated calls.
    Begin { problem: usize },
    /// Current value of a candidate variable and whether it is randomized.
    SetVar {
        problem: usize,
        var: u32,
        value: IrExpr,
        active: IrExpr,
    },
    /// Whether a constraint block takes part (constraint_mode, 18.9).
    SetBlock {
        problem: usize,
        block: u32,
        enabled: IrExpr,
    },
    /// A state leaf's value, or `None` when evaluating it would dereference
    /// a null handle (an 18.5.12 error the guards may eliminate).
    SetLeaf {
        problem: usize,
        leaf: u32,
        value: Option<IrExpr>,
    },
    /// The cycle state of `randc` variable `var` (S30-D3).
    SetCyclic {
        problem: usize,
        var: u32,
        state: IrExpr,
    },
    /// The result of a ready constraint function call.
    SetCallResult {
        problem: usize,
        call: u32,
        value: IrExpr,
    },
}

impl IrRandStmt {
    pub fn problem(&self) -> usize {
        match self {
            Self::Begin { problem }
            | Self::SetVar { problem, .. }
            | Self::SetBlock { problem, .. }
            | Self::SetLeaf { problem, .. }
            | Self::SetCyclic { problem, .. }
            | Self::SetCallResult { problem, .. } => *problem,
        }
    }

    /// Every packed expression the statement evaluates, in order.
    pub fn expressions(&self) -> Vec<&IrExpr> {
        match self {
            Self::Begin { .. } => Vec::new(),
            Self::SetVar { value, active, .. } => vec![value, active],
            Self::SetBlock { enabled, .. } => vec![enabled],
            Self::SetLeaf { value, .. } => value.iter().collect(),
            Self::SetCyclic { state, .. } => vec![state],
            Self::SetCallResult { value, .. } => vec![value],
        }
    }

    pub fn expressions_mut(&mut self) -> Vec<&mut IrExpr> {
        match self {
            Self::Begin { .. } => Vec::new(),
            Self::SetVar { value, active, .. } => vec![value, active],
            Self::SetBlock { enabled, .. } => vec![enabled],
            Self::SetLeaf { value, .. } => value.iter_mut().collect(),
            Self::SetCyclic { state, .. } => vec![state],
            Self::SetCallResult { value, .. } => vec![value],
        }
    }
}

/// Status values [`super::IrSysFunc::RandStep`] returns.
pub const RAND_STEP_FAILED: i64 = 0;
pub const RAND_STEP_OK: i64 = 1;
pub const RAND_STEP_NEED_CALLS: i64 = 4;
