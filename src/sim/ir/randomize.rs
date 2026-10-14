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
}

impl IrRandOp {
    pub fn code(self) -> u16 {
        self as u16
    }

    /// Operand count fixed by the operation (list operations use `args`).
    fn fixed_operands(self) -> usize {
        match self {
            Self::Const | Self::Var | Self::State | Self::Call | Self::Concat | Self::All => 0,
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
            if var.width == 0 || var.domain.is_some_and(|d| d >= count) {
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
            Self::SetCallResult { value, .. } => vec![value],
        }
    }

    pub fn expressions_mut(&mut self) -> Vec<&mut IrExpr> {
        match self {
            Self::Begin { .. } => Vec::new(),
            Self::SetVar { value, active, .. } => vec![value, active],
            Self::SetBlock { enabled, .. } => vec![enabled],
            Self::SetLeaf { value, .. } => value.iter_mut().collect(),
            Self::SetCallResult { value, .. } => vec![value],
        }
    }
}

/// Status values [`super::IrSysFunc::RandStep`] returns.
pub const RAND_STEP_FAILED: i64 = 0;
pub const RAND_STEP_OK: i64 = 1;
pub const RAND_STEP_NEED_CALLS: i64 = 4;
