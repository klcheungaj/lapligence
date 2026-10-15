//! Constrained randomization (IEEE 1800-2009 clause 18).
//!
//! A randomize call lowers into ordinary IR (handle locals, dynamic-class
//! cases, pre/post hook calls, loops) around a few runtime primitives on one
//! static problem: [`IrRandStmt`] feeds the solver session and the
//! [`super::IrSysFunc`] variants `RandStep`, `RandCallReady` and `RandValue`
//! run it and read its solution. The problem itself is pure data that the
//! emitter turns into the `llg_cr_problem_t` tables of `llg_constraint.h`.

use super::{IrChandleExpr, IrExpr, IrValidationError};

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
    /// Expanded problems only (the solver builds it); never lowered.
    Elem,
    /// Template operations of problems with units (SIM-030, see
    /// `llg_constraint.h`). A "ref" operand is a `Ref` node or
    /// [`RAND_NONE`] (the expanding object itself).
    /// `a` = slot, `b` = base ref, `c` = handle-array index node or NONE.
    Ref,
    /// `a` = ref, `b` = variable.
    HVar,
    /// `a` = array, `b` = 0-based position node, `c` = ref.
    AElem,
    /// `a` = array (or [`RAND_SLOT_ARRAY`] | slot), `c` = ref.
    ASize,
    /// `a` = loop index.
    Iter,
    /// `args[b..b+4]` = array, body, ref, loop.
    Foreach,
    /// `args[b..b+5]` = array, body, ref, loop, operation code.
    Reduce,
    /// `a` = ref; 1 when it names no object.
    IsNull,
}

/// `LLG_CR_NONE`: an absent operand.
pub const RAND_NONE: u32 = u32::MAX;
/// `LLG_CR_SLOT_ARRAY`: marks a handle-array slot in `ASize`/`Foreach`.
pub const RAND_SLOT_ARRAY: u32 = 0x8000_0000;

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
            | Self::Dist
            | Self::Elem
            | Self::Ref
            | Self::HVar
            | Self::AElem
            | Self::ASize
            | Self::Iter
            | Self::Foreach
            | Self::Reduce
            | Self::IsNull => 0,
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

/// Array kinds (`LLG_CR_ARRAY_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IrRandArrayKind {
    Fixed = 0,
    Dynamic = 1,
    Queue = 2,
}

/// A random (or state) unpacked array of packed elements of one unit
/// (`llg_cr_array_t`); fixed arrays keep their declared bounds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandArray {
    pub name: String,
    pub elem_width: u32,
    pub elem_signed: bool,
    pub kind: IrRandArrayKind,
    pub left: i32,
    pub right: i32,
}

/// An object handle property reached by constraints (`llg_cr_slot_t`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandSlot {
    pub name: String,
    pub kind: IrRandArrayKind,
    pub is_array: bool,
    pub left: i32,
    pub right: i32,
}

/// One class layout of a problem with units (`llg_cr_unit_t`): the
/// problem-wide variables, arrays and slots its objects have, and its
/// ranges of blocks and leaves.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrRandUnit {
    pub name: String,
    pub vars: Vec<u32>,
    pub arrays: Vec<u32>,
    pub slots: Vec<u32>,
    pub block_first: u32,
    pub block_count: u32,
    pub leaf_first: u32,
    pub leaf_count: u32,
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
    /// Problems with objects or arrays (SIM-030); empty otherwise.
    pub units: Vec<IrRandUnit>,
    pub arrays: Vec<IrRandArray>,
    pub slots: Vec<IrRandSlot>,
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
                IrRandOp::Elem => return fail(format!("node {index} is an expanded element")),
                IrRandOp::Ref
                | IrRandOp::HVar
                | IrRandOp::AElem
                | IrRandOp::ASize
                | IrRandOp::Iter
                | IrRandOp::Foreach
                | IrRandOp::Reduce
                | IrRandOp::IsNull => {
                    if self.units.is_empty() {
                        return fail(format!(
                            "template node {index} outside a problem with units"
                        ));
                    }
                    if let Some(message) = self.template_node_error(index) {
                        return fail(format!("template node {index} {message}"));
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
        if !self.units.is_empty() {
            let mut block_end = 0;
            let mut leaf_end = 0;
            for (index, unit) in self.units.iter().enumerate() {
                if unit.vars.iter().any(|v| *v >= vars)
                    || unit.arrays.iter().any(|a| *a as usize >= self.arrays.len())
                    || unit.slots.iter().any(|s| *s as usize >= self.slots.len())
                    || unit.block_first != block_end
                    || unit.leaf_first != leaf_end
                {
                    return fail(format!("unit {index} is malformed"));
                }
                block_end += unit.block_count;
                leaf_end += unit.leaf_count;
            }
            if block_end as usize != self.blocks.len() || leaf_end as usize != self.leaves.len() {
                return fail("units do not cover the blocks and leaves".to_owned());
            }
            for (index, array) in self.arrays.iter().enumerate() {
                if array.elem_width == 0 || array.elem_width >= crate::sim::emit_c::LLG_WIDTH_LIMIT
                {
                    return fail(format!("array {index} has an invalid element width"));
                }
            }
        }
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

impl IrRandProblem {
    /// Operand check of template node `index`, or the reason it is invalid.
    fn template_node_error(&self, index: u32) -> Option<&'static str> {
        let node = &self.nodes[index as usize];
        let is_ref =
            |x: u32| x == RAND_NONE || (x < index && self.nodes[x as usize].op == IrRandOp::Ref);
        let is_array = |x: u32| {
            if x & RAND_SLOT_ARRAY != 0 {
                self.slots
                    .get((x & !RAND_SLOT_ARRAY) as usize)
                    .is_some_and(|slot| slot.is_array)
            } else {
                (x as usize) < self.arrays.len()
            }
        };
        let ok = match node.op {
            IrRandOp::Ref => {
                (node.a as usize) < self.slots.len()
                    && is_ref(node.b)
                    && (node.c == RAND_NONE || node.c < index)
            }
            IrRandOp::HVar => {
                is_ref(node.a)
                    && self
                        .vars
                        .get(node.b as usize)
                        .is_some_and(|var| (var.width, var.signed) == (node.width, node.signed))
            }
            IrRandOp::AElem => {
                self.arrays.get(node.a as usize).is_some_and(|array| {
                    (array.elem_width, array.elem_signed) == (node.width, node.signed)
                }) && node.b < index
                    && is_ref(node.c)
            }
            IrRandOp::ASize => is_array(node.a) && is_ref(node.c) && node.width == 32,
            IrRandOp::Iter => node.width == 32 && node.signed,
            IrRandOp::IsNull => is_ref(node.a) && node.width == 1,
            IrRandOp::Foreach | IrRandOp::Reduce => {
                let count = if node.op == IrRandOp::Foreach { 4 } else { 5 };
                let first = node.b as usize;
                first + count <= self.args.len() && {
                    let x = &self.args[first..first + count];
                    is_array(x[0])
                        && x[1] < index
                        && is_ref(x[2])
                        && (count == 4
                            || [
                                IrRandOp::Add,
                                IrRandOp::Mul,
                                IrRandOp::And,
                                IrRandOp::Or,
                                IrRandOp::Xor,
                                IrRandOp::LogAnd,
                                IrRandOp::LogOr,
                            ]
                            .iter()
                            .any(|op| u32::from(op.code()) == x[4]))
                }
            }
            _ => true,
        };
        (!ok).then_some("has invalid operands")
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
    /// Problems with units (SIM-030): the randomized object's handle, so
    /// a handle that names it again is the same instance.
    InstRoot {
        problem: usize,
        handle: IrChandleExpr,
    },
    /// [`Self::SetVar`] of variable `var` of instance `inst`.
    ISetVar {
        problem: usize,
        inst: IrExpr,
        var: u32,
        value: IrExpr,
        active: IrExpr,
    },
    ISetBlock {
        problem: usize,
        inst: IrExpr,
        block: u32,
        enabled: IrExpr,
    },
    ISetLeaf {
        problem: usize,
        inst: IrExpr,
        leaf: u32,
        value: Option<IrExpr>,
    },
    ISetCyclic {
        problem: usize,
        inst: IrExpr,
        var: u32,
        state: IrExpr,
    },
    /// Array `array` of instance `inst`: its current size and whether it
    /// is random.
    SetArray {
        problem: usize,
        inst: IrExpr,
        array: u32,
        size: IrExpr,
        active: IrExpr,
    },
    /// The current value of element `index` (0-based position).
    SetElem {
        problem: usize,
        inst: IrExpr,
        array: u32,
        index: IrExpr,
        value: IrExpr,
    },
    /// Handle slot `slot` of instance `inst` names `child`, an object of
    /// class unit `unit` (`random`: the handle is `rand` and enabled).
    Link {
        problem: usize,
        inst: IrExpr,
        slot: u32,
        child: IrChandleExpr,
        unit: u32,
        random: IrExpr,
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
            | Self::SetCallResult { problem, .. }
            | Self::InstRoot { problem, .. }
            | Self::ISetVar { problem, .. }
            | Self::ISetBlock { problem, .. }
            | Self::ISetLeaf { problem, .. }
            | Self::ISetCyclic { problem, .. }
            | Self::SetArray { problem, .. }
            | Self::SetElem { problem, .. }
            | Self::Link { problem, .. } => *problem,
        }
    }

    /// Every handle expression the statement evaluates.
    pub fn chandles(&self) -> Vec<&IrChandleExpr> {
        match self {
            Self::InstRoot { handle, .. } => vec![handle],
            Self::Link { child, .. } => vec![child],
            _ => Vec::new(),
        }
    }

    pub fn chandles_mut(&mut self) -> Vec<&mut IrChandleExpr> {
        match self {
            Self::InstRoot { handle, .. } => vec![handle],
            Self::Link { child, .. } => vec![child],
            _ => Vec::new(),
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
            Self::InstRoot { .. } => Vec::new(),
            Self::ISetVar {
                inst,
                value,
                active,
                ..
            } => vec![inst, value, active],
            Self::ISetBlock { inst, enabled, .. } => vec![inst, enabled],
            Self::ISetLeaf { inst, value, .. } => {
                std::iter::once(inst).chain(value.iter()).collect()
            }
            Self::ISetCyclic { inst, state, .. } => vec![inst, state],
            Self::SetArray {
                inst, size, active, ..
            } => vec![inst, size, active],
            Self::SetElem {
                inst, index, value, ..
            } => vec![inst, index, value],
            Self::Link { inst, random, .. } => vec![inst, random],
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
            Self::InstRoot { .. } => Vec::new(),
            Self::ISetVar {
                inst,
                value,
                active,
                ..
            } => vec![inst, value, active],
            Self::ISetBlock { inst, enabled, .. } => vec![inst, enabled],
            Self::ISetLeaf { inst, value, .. } => {
                std::iter::once(inst).chain(value.iter_mut()).collect()
            }
            Self::ISetCyclic { inst, state, .. } => vec![inst, state],
            Self::SetArray {
                inst, size, active, ..
            } => vec![inst, size, active],
            Self::SetElem {
                inst, index, value, ..
            } => vec![inst, index, value],
            Self::Link { inst, random, .. } => vec![inst, random],
        }
    }
}

/// Instance queries of a problem with units (SIM-030), read by
/// [`super::IrSysFunc::RandInst`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IrRandQuery {
    /// The next instance to feed, or -1 (32-bit signed).
    NextInst,
    /// 1 when instance `inst`'s `pre_randomize` is due (first random feed).
    Pre,
    /// Registered instances (32-bit signed).
    Count,
    /// Unit of `inst` (32-bit signed).
    Unit,
    /// 1 when `inst` takes part as random.
    Random,
    /// Solved value of variable `var` of `inst`, typed like the variable.
    Value(u32),
    /// 1 when variable `var` of `inst` was randomized.
    Active(u32),
    /// Cycle state of `randc` variable `var` of `inst`.
    Cyclic(u32),
    /// Solved size of array `array` of `inst` (32-bit signed).
    ArraySize(u32),
    /// 1 when array `array` of `inst` was randomized.
    ArrayActive(u32),
    /// Solved element `index` of array `array` of `inst`.
    Elem(u32),
}

/// Status values [`super::IrSysFunc::RandStep`] returns.
pub const RAND_STEP_FAILED: i64 = 0;
pub const RAND_STEP_OK: i64 = 1;
pub const RAND_STEP_NEED_CALLS: i64 = 4;

#[cfg(test)]
mod tests {
    use super::*;

    fn node(op: IrRandOp, signed: bool, width: u32, a: u32, b: u32, c: u32) -> IrRandNode {
        IrRandNode {
            op,
            signed,
            width,
            a,
            b,
            c,
        }
    }

    /// `foreach (h.a[i]) h.a[i] < x`: a unit 0 with a handle slot to unit 1,
    /// which owns array `a`; `x` is unit 0's variable.
    fn template_problem() -> IrRandProblem {
        IrRandProblem {
            name: "p".to_owned(),
            nodes: vec![
                node(IrRandOp::Ref, false, 1, 0, RAND_NONE, RAND_NONE),
                node(IrRandOp::Iter, true, 32, 0, 0, 0),
                node(IrRandOp::AElem, false, 8, 0, 1, 0),
                node(IrRandOp::Var, false, 8, 0, 0, 0),
                node(IrRandOp::Lt, false, 1, 2, 3, 0),
                node(IrRandOp::Foreach, false, 1, 0, 0, 0),
            ],
            args: vec![0, 4, 0, 0],
            vars: vec![IrRandVar {
                name: "x".to_owned(),
                width: 8,
                signed: false,
                domain: None,
                cyclic: false,
            }],
            blocks: vec![IrRandBlock {
                name: "c".to_owned(),
                root: 5,
            }],
            units: vec![
                IrRandUnit {
                    name: "top".to_owned(),
                    vars: vec![0],
                    arrays: Vec::new(),
                    slots: vec![0],
                    block_first: 0,
                    block_count: 1,
                    leaf_first: 0,
                    leaf_count: 0,
                },
                IrRandUnit {
                    name: "leaf".to_owned(),
                    vars: Vec::new(),
                    arrays: vec![0],
                    slots: Vec::new(),
                    block_first: 1,
                    block_count: 0,
                    leaf_first: 0,
                    leaf_count: 0,
                },
            ],
            arrays: vec![IrRandArray {
                name: "a".to_owned(),
                elem_width: 8,
                elem_signed: false,
                kind: IrRandArrayKind::Dynamic,
                left: 0,
                right: 0,
            }],
            slots: vec![IrRandSlot {
                name: "h".to_owned(),
                kind: IrRandArrayKind::Fixed,
                is_array: false,
                left: 0,
                right: 0,
            }],
            ..IrRandProblem::default()
        }
    }

    #[test]
    fn template_problems_validate_their_units_and_operands() {
        assert!(template_problem().validate("p").is_ok());

        // Template operations need units.
        let mut flat = template_problem();
        flat.units.clear();
        assert!(flat.validate("p").is_err());

        // An element node typed differently from its array.
        let mut mistyped = template_problem();
        mistyped.nodes[2].width = 4;
        assert!(mistyped.validate("p").is_err());

        // A reference must be a `Ref` node (or none).
        let mut bad_ref = template_problem();
        bad_ref.nodes[2].c = 1;
        assert!(bad_ref.validate("p").is_err());

        // Units cover the blocks exactly, in order.
        let mut gap = template_problem();
        gap.units[1].block_first = 2;
        assert!(gap.validate("p").is_err());

        // A unit names only existing arrays.
        let mut missing = template_problem();
        missing.units[1].arrays.push(3);
        assert!(missing.validate("p").is_err());

        // An expanded element table never appears in a lowered problem.
        let mut expanded = template_problem();
        expanded.nodes[2].op = IrRandOp::Elem;
        assert!(expanded.validate("p").is_err());
    }
}
