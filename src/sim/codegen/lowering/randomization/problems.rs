//! Translation of lowered constraint expressions into solver nodes.
//!
//! Constraint expressions are lowered with the ordinary expression lowering,
//! with every random variable replaced by a placeholder local (see
//! `RandLowering`). This module turns that typed IR into the post-order node
//! table of `llg_constraint.h`: subtrees that read no random variable become
//! constants or state leaves the generated code evaluates before solving;
//! the rest map operator by operator. Every node carries the width and
//! signedness the value facade gives the same operation (IEEE 1800-2009
//! 11.6, 11.8), and operands are converted explicitly, so the solver never
//! re-derives an expression type.

use crate::core::db::NodeId;
use crate::sim::ir::{
    IrBinOp, IrConst, IrExpr, IrExprKind, IrInsideItem, IrRandArrayKind, IrRandCall, IrRandDist,
    IrRandDistItem, IrRandLeaf, IrRandNode, IrRandOp, IrRandOrder, IrRandProblem, IrRandVar,
    IrUnOp,
};
use std::collections::HashMap;

/// Prefix of the placeholder locals that stand for random variables while a
/// constraint is lowered. They never reach emission.
pub(super) const PLACEHOLDER_PREFIX: &str = "_llg_randvar_";

/// Prefix of the placeholders that stand for template nodes (array
/// elements, sizes, reductions, loop indices, object members) of a problem
/// with units; they share [`PLACEHOLDER_PREFIX`] so they are never leaves.
pub(super) const TEMPLATE_PREFIX: &str = "_llg_randvar_t";

/// A state value evaluated before solving, with its source description.
pub(super) struct LeafPlan {
    pub(super) template: IrExpr,
}

/// A constraint function call whose arguments read random variables
/// (18.5.11); `template` still contains placeholders.
pub(super) struct CallPlan {
    pub(super) template: IrExpr,
}

pub(super) struct ProblemBuilder {
    pub(super) problem: IrRandProblem,
    pub(super) leaves: Vec<LeafPlan>,
    pub(super) calls: Vec<CallPlan>,
    /// Placeholder local name -> variable index.
    placeholders: HashMap<String, u32>,
    var_nodes: Vec<Option<u32>>,
    /// Description of the constraint item being translated (leaf names).
    pub(super) item: String,
    /// `solve...before` pairs of the block being translated; `add_block`
    /// assigns them to it.
    pending_orders: Vec<(u32, u32)>,
    /// Template placeholder name -> template node (problems with units).
    templates: HashMap<String, u32>,
    /// Property declaration -> its problem-wide variable (problems with
    /// units share one variable per property among all units).
    pub(super) decl_vars: HashMap<NodeId, u32>,
    /// The unit whose constraints are being translated, and its first leaf
    /// (leaves are unit ranges, so they are shared only within a unit).
    pub(super) unit: u32,
    leaf_first: usize,
    loops: u32,
    /// Objects, arrays and loops (`templates.rs`); `None` for a problem
    /// without units.
    pub(super) units: Option<Box<super::templates::UnitState>>,
}

/// One lowered `dist` item (18.5.4).
pub(super) struct DistSpec {
    pub(super) item: IrInsideItem,
    pub(super) weight: Option<IrExpr>,
    pub(super) per_range: bool,
}

type Typed = (u32, u32, bool);

fn has_x_or_z(constant: &IrConst) -> bool {
    constant.x_mask().iter().any(|word| *word != 0)
        || constant.z_mask().iter().any(|word| *word != 0)
}

/// Whether `expr` reads any placeholder.
pub(super) fn has_placeholder(expr: &IrExpr) -> bool {
    let mut copy = expr.clone();
    let mut found = false;
    crate::sim::opt::walk_expr_mut(&mut copy, &mut |node| {
        if let IrExprKind::LocalRead(name) = &node.kind {
            found |= name.starts_with(PLACEHOLDER_PREFIX);
        }
    });
    found
}

/// Placeholder names `expr` reads, in first-use order.
pub(super) fn placeholders_in(expr: &IrExpr) -> Vec<String> {
    let mut copy = expr.clone();
    let mut names = Vec::new();
    crate::sim::opt::walk_expr_mut(&mut copy, &mut |node| {
        if let IrExprKind::LocalRead(name) = &node.kind {
            if name.starts_with(PLACEHOLDER_PREFIX) && !names.contains(name) {
                names.push(name.clone());
            }
        }
    });
    names
}

impl ProblemBuilder {
    pub(super) fn new(name: String) -> Self {
        Self {
            problem: IrRandProblem {
                name,
                ..IrRandProblem::default()
            },
            leaves: Vec::new(),
            calls: Vec::new(),
            placeholders: HashMap::new(),
            var_nodes: Vec::new(),
            item: String::new(),
            pending_orders: Vec::new(),
            templates: HashMap::new(),
            decl_vars: HashMap::new(),
            unit: 0,
            leaf_first: 0,
            loops: 0,
            units: None,
        }
    }

    /// Start translating the constraints of unit `unit`.
    pub(super) fn begin_unit(&mut self, unit: u32) {
        self.unit = unit;
        self.leaf_first = self.leaves.len();
    }

    /// A template node and the placeholder that stands for it.
    pub(super) fn template(
        &mut self,
        op: IrRandOp,
        signed: bool,
        width: u32,
        operands: [u32; 3],
    ) -> u32 {
        let [a, b, c] = operands;
        self.push(op, signed, width, a, b, c)
    }

    /// The placeholder read standing for template node `node`.
    pub(super) fn template_placeholder(&mut self, node: u32) -> IrExpr {
        let name = format!("{TEMPLATE_PREFIX}{}", self.templates.len());
        self.templates.insert(name.clone(), node);
        let (_, width, signed) = self.ty(node);
        IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
    }

    /// Whether `expr` reads a template placeholder.
    pub(super) fn has_template(&self, expr: &IrExpr) -> bool {
        placeholders_in(expr)
            .iter()
            .any(|name| self.templates.contains_key(name))
    }

    /// Append operand list `items` to the problem's args; returns its first.
    pub(super) fn push_args(&mut self, items: &[u32]) -> u32 {
        let first = self.problem.args.len() as u32;
        self.problem.args.extend_from_slice(items);
        first
    }

    pub(super) fn new_loop(&mut self) -> u32 {
        self.loops += 1;
        self.loops - 1
    }

    /// Width and signedness of node `node`.
    pub(super) fn node_type(&self, node: u32) -> (u32, bool) {
        let n = &self.problem.nodes[node as usize];
        (n.width, n.signed)
    }

    /// The 0-based position of declared index `index` of an array whose
    /// declared bounds are `left`/`right` (a fixed array) or none (dynamic
    /// arrays and queues are indexed by position), as a 64-bit signed node.
    pub(super) fn position(
        &mut self,
        kind: IrRandArrayKind,
        left: i32,
        right: i32,
        index: u32,
    ) -> Result<u32, String> {
        let (index, w, s) = self.ty(index);
        if kind != IrRandArrayKind::Fixed || (left <= right && left == 0) {
            return Ok(index);
        }
        if w > 64 {
            return Err("an array index wider than 64 bits is not supported in constraints".into());
        }
        let wide = self.coerce_own(index, w, s, 64);
        let base = self.constant_u64(i64::from(left) as u64, 64, true);
        Ok(if left <= right {
            self.push(IrRandOp::Sub, true, 64, wide, base, 0)
        } else {
            self.push(IrRandOp::Sub, true, 64, base, wide, 0)
        })
    }

    /// `left == right` in their common type (for `inside` an array).
    pub(super) fn equal(&mut self, left: u32, right: u32) -> u32 {
        self.compare(IrRandOp::Eq, left, right)
    }

    /// Add a candidate variable; returns its index and placeholder name.
    pub(super) fn add_var(&mut self, name: String, width: u32, signed: bool) -> (u32, String) {
        let index = self.problem.vars.len() as u32;
        let placeholder = format!("{PLACEHOLDER_PREFIX}{index}");
        self.problem.vars.push(IrRandVar {
            name,
            width,
            signed,
            domain: None,
            cyclic: false,
        });
        self.placeholders.insert(placeholder.clone(), index);
        self.var_nodes.push(None);
        (index, placeholder)
    }

    /// Restrict variable `var` to `values` while it is active (S29-D2).
    pub(super) fn set_domain(&mut self, var: u32, values: &[IrConst]) -> Result<(), String> {
        let (width, signed) = {
            let v = &self.problem.vars[var as usize];
            (v.width, v.signed)
        };
        let node = self.var_node(var);
        let mut any = None;
        for value in values {
            let constant = self.constant(value, width, signed)?;
            let equal = self.push(IrRandOp::Eq, false, 1, node, constant, 0);
            any = Some(match any {
                None => equal,
                Some(previous) => self.push(IrRandOp::LogOr, false, 1, previous, equal, 0),
            });
        }
        let domain = match any {
            Some(node) => node,
            None => self.constant_u64(0, 1, false),
        };
        self.problem.vars[var as usize].domain = Some(domain);
        Ok(())
    }

    pub(super) fn add_block(&mut self, name: String, root: u32) -> u32 {
        let index = self.problem.blocks.len() as u32;
        self.problem
            .blocks
            .push(crate::sim::ir::IrRandBlock { name, root });
        for (before, after) in std::mem::take(&mut self.pending_orders) {
            self.problem.orders.push(IrRandOrder {
                block: index,
                before,
                after,
            });
        }
        index
    }

    /// Mark variable `var` `randc` (18.4.2).
    pub(super) fn set_cyclic(&mut self, var: u32) {
        self.problem.vars[var as usize].cyclic = true;
    }

    /// The variable a lowered expression names when it is exactly one
    /// placeholder read.
    pub(super) fn placeholder_var(&self, expr: &IrExpr) -> Option<u32> {
        match &expr.kind {
            IrExprKind::LocalRead(name) => self.placeholders.get(name).copied(),
            _ => None,
        }
    }

    pub(super) fn is_cyclic(&self, var: u32) -> bool {
        self.problem.vars[var as usize].cyclic
    }

    /// Record `solve before before after` for the block being translated.
    pub(super) fn add_order(&mut self, before: u32, after: u32) {
        self.pending_orders.push((before, after));
    }

    /// `value dist { items }` under `guard` (18.5.4): a 1-bit node that holds
    /// when the value matches an item of nonzero weight, with the weights
    /// recorded for the sampler (S30-D1). Item values, bounds and weights
    /// that read random variables are solved first by the runtime (S30-D10).
    pub(super) fn dist(
        &mut self,
        value: &IrExpr,
        specs: &[DistSpec],
        guard: Option<u32>,
    ) -> Result<u32, String> {
        let value = self.expr(value)?;
        let mut bounds = Vec::with_capacity(specs.len());
        for spec in specs {
            bounds.push(match &spec.item {
                IrInsideItem::Value(item) => {
                    if let IrExprKind::Const(constant) = &item.kind {
                        if has_x_or_z(constant) {
                            return Err(
                                "a dist item value with X or Z bits is not supported".into()
                            );
                        }
                    }
                    let node = self.expr(item)?;
                    (Some(node), Some(node), true)
                }
                IrInsideItem::Range { low, high } => {
                    (Some(self.expr(low)?), Some(self.expr(high)?), false)
                }
                IrInsideItem::OpenRange { low, high } => (
                    low.as_ref().map(|low| self.expr(low)).transpose()?,
                    high.as_ref().map(|high| self.expr(high)).transpose()?,
                    false,
                ),
                _ => return Err("a dist item must be a value or a range (18.5.4)".into()),
            });
        }
        // The value and every bound compare in one type (11.4.13), which
        // also orders all items in one domain for the support size.
        let (_, mut width, mut signed) = self.ty(value);
        for (low, high, _) in &bounds {
            for node in [*low, *high].into_iter().flatten() {
                let (_, w, s) = self.ty(node);
                width = width.max(w);
                signed &= s;
            }
        }
        let words = width.div_ceil(64).max(1) as usize;
        let target = self.coerce(value, width, signed);
        let mut items = Vec::with_capacity(specs.len());
        for (spec, (low, high, single)) in specs.iter().zip(bounds) {
            let low = match low {
                Some(node) => self.coerce(node, width, signed),
                None => {
                    // `$` as a lower bound: the type's minimum.
                    let mut min = vec![0u64; words];
                    if signed {
                        min[((width - 1) / 64) as usize] |= 1u64 << ((width - 1) % 64);
                    }
                    self.constant_words(&min, width, signed)
                }
            };
            let high = match high {
                Some(node) if !single => self.coerce(node, width, signed),
                Some(_) => low,
                None => {
                    let mut max = vec![u64::MAX; words];
                    if signed {
                        max[((width - 1) / 64) as usize] &= !(1u64 << ((width - 1) % 64));
                    }
                    self.constant_words(&max, width, signed)
                }
            };
            let matches = if single {
                self.push(IrRandOp::Eq, false, 1, target, low, 0)
            } else {
                let ge = self.push(IrRandOp::Ge, false, 1, target, low, 0);
                let le = self.push(IrRandOp::Le, false, 1, target, high, 0);
                self.push(IrRandOp::LogAnd, false, 1, ge, le, 0)
            };
            let weight = spec
                .weight
                .as_ref()
                .map(|weight| {
                    if weight.is_real() {
                        return Err("a dist weight must be integral (18.5.4)".to_owned());
                    }
                    self.expr(weight)
                })
                .transpose()?;
            items.push(IrRandDistItem {
                matches,
                lo: low,
                hi: high,
                weight,
                per_range: spec.per_range,
                signed,
            });
        }
        let index = self.problem.dists.len() as u32;
        self.problem.dists.push(IrRandDist {
            expr: value,
            guard,
            items,
        });
        Ok(self.push(IrRandOp::Dist, false, 1, index, 0, 0))
    }

    fn push(&mut self, op: IrRandOp, signed: bool, width: u32, a: u32, b: u32, c: u32) -> u32 {
        let node = IrRandNode {
            op,
            signed,
            width,
            a,
            b,
            c,
        };
        self.problem.nodes.push(node);
        (self.problem.nodes.len() - 1) as u32
    }

    fn ty(&self, node: u32) -> Typed {
        let n = &self.problem.nodes[node as usize];
        (node, n.width, n.signed)
    }

    fn var_node(&mut self, var: u32) -> u32 {
        if let Some(node) = self.var_nodes[var as usize] {
            return node;
        }
        let (width, signed) = {
            let v = &self.problem.vars[var as usize];
            (v.width, v.signed)
        };
        let node = self.push(IrRandOp::Var, signed, width, var, 0, 0);
        self.var_nodes[var as usize] = Some(node);
        node
    }

    fn constant_words(&mut self, words: &[u64], width: u32, signed: bool) -> u32 {
        let first = self.problem.consts.len() as u32;
        let count = width.div_ceil(64).max(1) as usize;
        for index in 0..count {
            let mut word = words.get(index).copied().unwrap_or(0);
            if index + 1 == count && !width.is_multiple_of(64) {
                word &= (1u64 << (width % 64)) - 1;
            }
            self.problem.consts.push(word);
        }
        self.push(IrRandOp::Const, signed, width, first, 0, 0)
    }

    pub(super) fn constant_u64(&mut self, value: u64, width: u32, signed: bool) -> u32 {
        self.constant_words(&[value], width, signed)
    }

    /// A known constant converted to `(width, signed)` by its own signedness.
    fn constant(&mut self, value: &IrConst, width: u32, signed: bool) -> Result<u32, String> {
        if has_x_or_z(value) || value.real_value().is_some() {
            return Err("an enum value with X or Z bits cannot constrain a random variable".into());
        }
        let node = self.constant_words(value.bits(), value.width(), value.signed());
        Ok(self.convert(node, width, signed))
    }

    /// `node` as `(width, signed)`, extending by the TARGET signedness (the
    /// facade's operand coercion and `sv4_resize`).
    fn coerce(&mut self, node: u32, width: u32, signed: bool) -> u32 {
        let (mut node, w, s) = self.ty(node);
        if s != signed {
            node = self.push(IrRandOp::Resize, signed, w, node, 0, 0);
        }
        if w != width {
            node = self.push(IrRandOp::Resize, signed, width, node, 0, 0);
        }
        node
    }

    /// `node` as `(width, signed)`, extending by the SOURCE signedness
    /// (`sv4_cast`, assignment-like conversion).
    fn convert(&mut self, node: u32, width: u32, signed: bool) -> u32 {
        let (mut node, w, s) = self.ty(node);
        if w != width {
            node = self.push(IrRandOp::Resize, s, width, node, 0, 0);
        }
        if s != signed {
            node = self.push(IrRandOp::Resize, signed, width, node, 0, 0);
        }
        node
    }

    /// One-bit truth of `node` (nonzero).
    pub(super) fn truth(&mut self, node: u32) -> u32 {
        let (node, w, _) = self.ty(node);
        if w == 1 {
            return self.coerce(node, 1, false);
        }
        self.push(IrRandOp::RedOr, false, 1, node, 0, 0)
    }

    pub(super) fn all(&mut self, items: &[u32]) -> u32 {
        match items {
            [] => self.constant_u64(1, 1, false),
            [only] => *only,
            _ => {
                let first = self.problem.args.len() as u32;
                self.problem.args.extend_from_slice(items);
                self.push(IrRandOp::All, false, 1, 0, first, items.len() as u32)
            }
        }
    }

    pub(super) fn logical(&mut self, op: IrRandOp, a: u32, b: u32) -> u32 {
        let a = self.truth(a);
        let b = self.truth(b);
        self.push(op, false, 1, a, b, 0)
    }

    pub(super) fn not(&mut self, a: u32) -> u32 {
        let a = self.truth(a);
        self.push(IrRandOp::LogNot, false, 1, a, 0, 0)
    }

    fn leaf(&mut self, expr: &IrExpr) -> Result<u32, String> {
        if expr.is_real() {
            return Err("real values are not supported in constraints".into());
        }
        if expr.fill.is_some() {
            return Err("an unsized fill literal in a constraint has no width".into());
        }
        if let IrExprKind::Const(constant) = &expr.kind {
            if !has_x_or_z(constant) && constant.real_value().is_none() {
                let node =
                    self.constant_words(constant.bits(), constant.width(), constant.signed());
                return Ok(self.coerce(node, expr.width, expr.signed));
            }
        }
        let first = self.leaf_first;
        let index = match self.leaves[first..]
            .iter()
            .position(|leaf| leaf.template == *expr)
        {
            Some(index) => first + index,
            None => {
                self.leaves.push(LeafPlan {
                    template: expr.clone(),
                });
                self.problem.leaves.push(IrRandLeaf {
                    name: format!("state value {}", self.item),
                    width: expr.width,
                    signed: expr.signed,
                });
                self.leaves.len() - 1
            }
        };
        Ok(self.push(IrRandOp::State, expr.signed, expr.width, index as u32, 0, 0))
    }

    /// Translate one lowered constraint expression.
    pub(super) fn expr(&mut self, expr: &IrExpr) -> Result<u32, String> {
        // `&&`, `||` and `!` keep their structure even over state values:
        // each operand is its own guard subexpression (18.5.12), so a null
        // handle in one operand cannot hide a deciding value in another.
        match &expr.kind {
            IrExprKind::Bin {
                op: op @ (IrBinOp::LogAnd | IrBinOp::LogOr | IrBinOp::LogImpl),
                a,
                b,
            } => return self.binary(*op, a, b),
            IrExprKind::Un {
                op: IrUnOp::LogNot,
                a,
            } => {
                let a = self.expr(a)?;
                return Ok(self.not(a));
            }
            _ => {}
        }
        if !has_placeholder(expr) {
            return self.leaf(expr);
        }
        if expr.is_real() {
            return Err("real values are not supported in constraints".into());
        }
        match &expr.kind {
            IrExprKind::LocalRead(name) => {
                if let Some(node) = self.templates.get(name) {
                    return Ok(*node);
                }
                let var = *self
                    .placeholders
                    .get(name)
                    .ok_or_else(|| "unknown random variable placeholder".to_owned())?;
                Ok(self.var_node(var))
            }
            IrExprKind::Bin {
                op: IrBinOp::Pow,
                a,
                b,
            } => {
                // `a ** b`: the result takes a's context type; b is
                // self-determined (11.4.3, Table 11-4). A zero base with a
                // negative exponent is an evaluation error (S30-D4).
                let base = self.expr(a)?;
                let base = self.coerce(base, expr.width, expr.signed);
                let exponent = self.expr(b)?;
                Ok(self.push(IrRandOp::Pow, expr.signed, expr.width, base, exponent, 0))
            }
            IrExprKind::Bin { op, a, b } => self.binary(*op, a, b),
            IrExprKind::Un { op, a } => {
                let a = self.expr(a)?;
                let (a, w, s) = self.ty(a);
                Ok(match op {
                    IrUnOp::Neg => self.push(IrRandOp::Neg, s, w, a, 0, 0),
                    IrUnOp::BitNeg => self.push(IrRandOp::Not, s, w, a, 0, 0),
                    IrUnOp::LogNot => self.not(a),
                    IrUnOp::RedAnd => self.push(IrRandOp::RedAnd, false, 1, a, 0, 0),
                    IrUnOp::RedNand => self.push(IrRandOp::RedNand, false, 1, a, 0, 0),
                    IrUnOp::RedOr => self.push(IrRandOp::RedOr, false, 1, a, 0, 0),
                    IrUnOp::RedNor => self.push(IrRandOp::RedNor, false, 1, a, 0, 0),
                    IrUnOp::RedXor => self.push(IrRandOp::RedXor, false, 1, a, 0, 0),
                    IrUnOp::RedXNor => self.push(IrRandOp::RedXnor, false, 1, a, 0, 0),
                })
            }
            IrExprKind::Mux { sel, a, b } => {
                let sel = self.expr(sel)?;
                let sel = self.truth(sel);
                let a = self.expr(a)?;
                let b = self.expr(b)?;
                let (_, wa, sa) = self.ty(a);
                let (_, wb, sb) = self.ty(b);
                let (w, s) = (wa.max(wb), sa && sb);
                let a = self.coerce(a, w, s);
                let b = self.coerce(b, w, s);
                Ok(self.push(IrRandOp::Cond, s, w, sel, a, b))
            }
            IrExprKind::Concat { parts } => self.concat(parts),
            IrExprKind::Replicate { count, parts } => {
                let inner = self.concat(parts)?;
                let (inner, w, _) = self.ty(inner);
                let count = u32::try_from(*count)
                    .ok()
                    .filter(|count| *count > 0)
                    .ok_or_else(|| "replication count is out of range".to_owned())?;
                let width = w
                    .checked_mul(count)
                    .ok_or_else(|| "replication is too wide".to_owned())?;
                Ok(self.push(IrRandOp::Repl, false, width, inner, count, 0))
            }
            IrExprKind::BitSel { base, idx } => {
                let base = self.expr(base)?;
                self.select(base, idx, 1, false)
            }
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                neg,
                ..
            } => {
                let base = self.expr(base)?;
                self.select(base, base_idx, expr.width, *neg)
            }
            IrExprKind::PartSel { base, left, right } => {
                let base = self.expr(base)?;
                self.part_select(base, *left, *right)
            }
            IrExprKind::Inside { value, items } => self.inside(value, items),
            IrExprKind::Resize { a } | IrExprKind::CastToPacked { a } => {
                if a.is_real() {
                    return Err("real values are not supported in constraints".into());
                }
                let a = self.expr(a)?;
                Ok(self.coerce(a, expr.width, expr.signed))
            }
            IrExprKind::Convert { a } => {
                let a = self.expr(a)?;
                Ok(self.convert(a, expr.width, expr.signed))
            }
            IrExprKind::ToTwoState { a } => {
                let a = self.expr(a)?;
                Ok(self.coerce(a, expr.width, expr.signed))
            }
            IrExprKind::CallFn(_) => self.call(expr),
            _ => {
                Err("this operator is not supported on random variables in constraints".to_owned())
            }
        }
    }

    fn concat(&mut self, parts: &[IrExpr]) -> Result<u32, String> {
        let mut nodes = Vec::with_capacity(parts.len());
        let mut width = 0u32;
        for part in parts {
            let node = self.expr(part)?;
            width = width
                .checked_add(self.ty(node).1)
                .ok_or_else(|| "concatenation is too wide".to_owned())?;
            nodes.push(node);
        }
        if nodes.is_empty() {
            return Err("empty concatenation".into());
        }
        let first = self.problem.args.len() as u32;
        self.problem.args.extend_from_slice(&nodes);
        Ok(self.push(IrRandOp::Concat, false, width, 0, first, nodes.len() as u32))
    }

    fn binary(&mut self, op: IrBinOp, a: &IrExpr, b: &IrExpr) -> Result<u32, String> {
        match op {
            IrBinOp::LogAnd => {
                let (a, b) = (self.expr(a)?, self.expr(b)?);
                return Ok(self.logical(IrRandOp::LogAnd, a, b));
            }
            IrBinOp::LogOr => {
                let (a, b) = (self.expr(a)?, self.expr(b)?);
                return Ok(self.logical(IrRandOp::LogOr, a, b));
            }
            IrBinOp::LogImpl => {
                let (a, b) = (self.expr(a)?, self.expr(b)?);
                return Ok(self.logical(IrRandOp::Implies, a, b));
            }
            IrBinOp::LogEquiv => {
                let (a, b) = (self.expr(a)?, self.expr(b)?);
                return Ok(self.logical(IrRandOp::Equiv, a, b));
            }
            IrBinOp::Pow => unreachable!("`**` is translated with its result type"),
            _ => {}
        }
        let left = self.expr(a)?;
        match op {
            IrBinOp::Shl | IrBinOp::Ashl | IrBinOp::Shr | IrBinOp::Ashr => {
                let amount = self.expr(b)?;
                let (left, w, s) = self.ty(left);
                let code = match op {
                    IrBinOp::Shl | IrBinOp::Ashl => IrRandOp::Shl,
                    IrBinOp::Shr => IrRandOp::Shr,
                    _ => IrRandOp::Ashr,
                };
                return Ok(self.push(code, s, w, left, amount, 0));
            }
            IrBinOp::WildEq | IrBinOp::WildNeq => {
                let equal = self.wild_equal(left, b)?;
                return Ok(if op == IrBinOp::WildNeq {
                    self.not(equal)
                } else {
                    equal
                });
            }
            _ => {}
        }
        let right = self.expr(b)?;
        let (_, wa, sa) = self.ty(left);
        let (_, wb, sb) = self.ty(right);
        let (w, s) = (wa.max(wb), sa && sb);
        let left = self.coerce(left, w, s);
        let right = self.coerce(right, w, s);
        let (code, compare) = match op {
            IrBinOp::Add => (IrRandOp::Add, false),
            IrBinOp::Sub => (IrRandOp::Sub, false),
            IrBinOp::Mul => (IrRandOp::Mul, false),
            IrBinOp::Div => (IrRandOp::Div, false),
            IrBinOp::Mod => (IrRandOp::Mod, false),
            IrBinOp::BitAnd => (IrRandOp::And, false),
            IrBinOp::BitOr => (IrRandOp::Or, false),
            IrBinOp::BitXor => (IrRandOp::Xor, false),
            IrBinOp::BitXNor => (IrRandOp::Xnor, false),
            IrBinOp::Eq | IrBinOp::CaseEq => (IrRandOp::Eq, true),
            IrBinOp::Neq | IrBinOp::CaseNeq => (IrRandOp::Ne, true),
            IrBinOp::Lt => (IrRandOp::Lt, true),
            IrBinOp::Le => (IrRandOp::Le, true),
            IrBinOp::Gt => (IrRandOp::Gt, true),
            IrBinOp::Ge => (IrRandOp::Ge, true),
            _ => return Err("this operator is not supported in constraints".into()),
        };
        Ok(if compare {
            self.push(code, false, 1, left, right, 0)
        } else {
            self.push(code, s, w, left, right, 0)
        })
    }

    /// `left ==? right` (and inside value items): X/Z bits of a constant
    /// right operand are wildcards (11.4.6, 11.4.13).
    fn wild_equal(&mut self, left: u32, right: &IrExpr) -> Result<u32, String> {
        if let IrExprKind::Const(constant) = &right.kind {
            if has_x_or_z(constant) && constant.real_value().is_none() {
                let (_, wa, sa) = self.ty(left);
                let (w, s) = (wa.max(constant.width()), sa && constant.signed());
                // Extend the constant (and its wildcard mask) by the common
                // signedness, like the facade's sv4_wild_eq.
                let width = constant.width();
                let words = width.div_ceil(64).max(1) as usize;
                let mut care = vec![0u64; words];
                let mut bits = vec![0u64; words];
                for index in 0..words {
                    let wild = constant.x_mask().get(index).copied().unwrap_or(0)
                        | constant.z_mask().get(index).copied().unwrap_or(0);
                    care[index] = !wild;
                    bits[index] = constant.bits().get(index).copied().unwrap_or(0) & !wild;
                }
                let care = self.constant_words(&care, width, s);
                let bits = self.constant_words(&bits, width, s);
                let care = self.coerce(care, w, s);
                let bits = self.coerce(bits, w, s);
                let left = self.coerce(left, w, s);
                let masked = self.push(IrRandOp::And, s, w, left, care, 0);
                let expected = self.push(IrRandOp::And, s, w, bits, care, 0);
                return Ok(self.push(IrRandOp::Eq, false, 1, masked, expected, 0));
            }
        }
        let right = self.expr(right)?;
        let (_, wa, sa) = self.ty(left);
        let (_, wb, sb) = self.ty(right);
        let (w, s) = (wa.max(wb), sa && sb);
        let left = self.coerce(left, w, s);
        let right = self.coerce(right, w, s);
        Ok(self.push(IrRandOp::Eq, false, 1, left, right, 0))
    }

    fn compare(&mut self, op: IrRandOp, left: u32, right: u32) -> u32 {
        let (_, wa, sa) = self.ty(left);
        let (_, wb, sb) = self.ty(right);
        let (w, s) = (wa.max(wb), sa && sb);
        let left = self.coerce(left, w, s);
        let right = self.coerce(right, w, s);
        self.push(op, false, 1, left, right, 0)
    }

    fn inside(&mut self, value: &IrExpr, items: &[IrInsideItem]) -> Result<u32, String> {
        let value = self.expr(value)?;
        self.inside_node(value, items)
    }

    /// `value inside { items }` for a translated value; no items is false.
    pub(super) fn inside_node(
        &mut self,
        value: u32,
        items: &[IrInsideItem],
    ) -> Result<u32, String> {
        let mut any = None;
        for item in items {
            let matched =
                match item {
                    IrInsideItem::Value(item) => self.wild_equal(value, item)?,
                    IrInsideItem::Range { low, high } => {
                        let low = self.expr(low)?;
                        let high = self.expr(high)?;
                        let ge = self.compare(IrRandOp::Ge, value, low);
                        let le = self.compare(IrRandOp::Le, value, high);
                        self.push(IrRandOp::LogAnd, false, 1, ge, le, 0)
                    }
                    IrInsideItem::OpenRange { low, high } => {
                        let mut both = self.constant_u64(1, 1, false);
                        if let Some(low) = low {
                            let low = self.expr(low)?;
                            let ge = self.compare(IrRandOp::Ge, value, low);
                            both = self.push(IrRandOp::LogAnd, false, 1, both, ge, 0);
                        }
                        if let Some(high) = high {
                            let high = self.expr(high)?;
                            let le = self.compare(IrRandOp::Le, value, high);
                            both = self.push(IrRandOp::LogAnd, false, 1, both, le, 0);
                        }
                        both
                    }
                    _ => return Err(
                        "`inside` an array is not supported in constraints (planned for SIM-030)"
                            .into(),
                    ),
                };
            any = Some(match any {
                None => matched,
                Some(previous) => self.push(IrRandOp::LogOr, false, 1, previous, matched, 0),
            });
        }
        Ok(match any {
            Some(node) => node,
            None => self.constant_u64(0, 1, false),
        })
    }

    /// `base[index +: width]` (or `-:`); a bit outside the base is an
    /// evaluation error, as its X value would be (S29-D1). A random index
    /// excludes the assignments that select outside the base (S30-D4).
    fn select(&mut self, base: u32, index: &IrExpr, width: u32, neg: bool) -> Result<u32, String> {
        let (base, base_width, _) = self.ty(base);
        if let IrExprKind::Const(constant) = &index.kind {
            if !has_x_or_z(constant) && constant.width() <= 64 {
                let raw = constant.bits().first().copied().unwrap_or(0);
                let value = if constant.signed() && constant.width() < 64 {
                    let shift = 64 - constant.width();
                    ((raw << shift) as i64) >> shift
                } else if constant.signed() {
                    raw as i64
                } else if raw > i64::MAX as u64 {
                    -1
                } else {
                    raw as i64
                };
                let low = if neg {
                    value.saturating_sub(i64::from(width) - 1)
                } else {
                    value
                };
                if !(neg && value < 0)
                    && low >= 0
                    && (low as u64) + u64::from(width) <= u64::from(base_width)
                {
                    return Ok(self.push(IrRandOp::Slice, false, width, base, low as u32, 0));
                }
            }
        }
        let index = self.expr(index)?;
        let index = if neg {
            let (index, w, s) = self.ty(index);
            if w > 64 {
                return Err(
                    "an indexed part-select index wider than 64 bits is not supported in constraints"
                        .into(),
                );
            }
            let wide = self.coerce_own(index, w, s, 64);
            let offset = self.constant_u64(u64::from(width) - 1, 64, true);
            self.push(IrRandOp::Sub, true, 64, wide, offset, 0)
        } else {
            index
        };
        Ok(self.push(IrRandOp::DynSlice, false, width, base, index, 0))
    }

    /// Extend `node` by its own signedness to `width`, then read it signed.
    fn coerce_own(&mut self, node: u32, w: u32, s: bool, width: u32) -> u32 {
        let mut node = node;
        if w != width {
            node = self.push(IrRandOp::Resize, s, width, node, 0, 0);
        }
        if !s {
            node = self.push(IrRandOp::Resize, true, width, node, 0, 0);
        }
        node
    }

    /// `base[left:right]` in physical bit positions; `left < right` reads
    /// the bits in reverse order (`sv4_part_select`).
    fn part_select(&mut self, base: u32, left: i64, right: i64) -> Result<u32, String> {
        let (base, base_width, _) = self.ty(base);
        let in_range = |bit: i64| bit >= 0 && bit < i64::from(base_width);
        if left >= right {
            let width = u32::try_from(left - right + 1)
                .map_err(|_| "part-select is too wide".to_owned())?;
            if in_range(left) && in_range(right) {
                return Ok(self.push(IrRandOp::Slice, false, width, base, right as u32, 0));
            }
            let index = self.constant_u64(right as u64, 64, true);
            return Ok(self.push(IrRandOp::DynSlice, false, width, base, index, 0));
        }
        let mut bits = Vec::new();
        for bit in left..=right {
            bits.push(if in_range(bit) {
                self.push(IrRandOp::Slice, false, 1, base, bit as u32, 0)
            } else {
                let index = self.constant_u64(bit as u64, 64, true);
                self.push(IrRandOp::DynSlice, false, 1, base, index, 0)
            });
        }
        let first = self.problem.args.len() as u32;
        let width = bits.len() as u32;
        self.problem.args.extend_from_slice(&bits);
        Ok(self.push(IrRandOp::Concat, false, width, 0, first, width))
    }

    fn call(&mut self, expr: &IrExpr) -> Result<u32, String> {
        if expr.width == 0 {
            return Err("a real-valued function is not supported in constraints".into());
        }
        if self.unit != 0 {
            return Err(
                "a function call in a constraint of an object reached through a `rand` handle is not supported"
                    .into(),
            );
        }
        if self.has_template(expr) {
            return Err(
                "a function call argument that reads an array element, array size, loop index or another object's variable is not supported in constraints"
                    .into(),
            );
        }
        let mut args = Vec::new();
        for name in placeholders_in(expr) {
            let var = *self
                .placeholders
                .get(&name)
                .ok_or_else(|| "unknown random variable placeholder".to_owned())?;
            args.push(self.var_node(var));
        }
        let index = self.calls.len() as u32;
        self.calls.push(CallPlan {
            template: expr.clone(),
        });
        self.problem.calls.push(IrRandCall {
            name: format!("function call {}", self.item),
            width: expr.width,
            signed: expr.signed,
            args,
        });
        Ok(self.push(IrRandOp::Call, expr.signed, expr.width, index, 0, 0))
    }
}
