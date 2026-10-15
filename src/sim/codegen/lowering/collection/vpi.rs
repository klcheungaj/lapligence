//! Owned VPI catalog capture (IEEE 1800-2009 clauses 36-38, Annex K/M).
//!
//! Every object the generated model exposes through `vpi_handle_by_name`,
//! `vpi_iterate` and the call-argument relations is captured here from the
//! owned database while lowering still has the frontend identities. The
//! runtime never reaches back into frontend data: names, ranges, directions
//! and parameter values are copied into [`IrVpiObject`].

use super::*;
use crate::core::db::{ConstantSource, ConstantType, Direction, NetType};
use crate::sim::ir::{
    IrVpiArgRef, IrVpiCompileArg, IrVpiCompileCall, IrVpiConstant, IrVpiDirection, IrVpiObject,
    IrVpiObjectKind, IrVpiVarKind,
};

const SEPARATOR: &str = "\u{1f}";

impl Codegen<'_> {
    /// The unit-separated hierarchical path of a scope or declaration, with
    /// generate blocks named per SV 27.4 and named statement blocks included.
    fn vpi_path(&self, id: NodeId) -> Vec<String> {
        match self.kind(id) {
            NodeKind::ModuleInst { .. } | NodeKind::GenScope | NodeKind::GenScopeArray => self
                .db
                .hierarchical_name_parts(id)
                .into_iter()
                .map(str::to_owned)
                .collect(),
            _ => {
                let node = self.node(id);
                let mut path = node
                    .parent
                    .map_or_else(Vec::new, |parent| self.vpi_path(parent));
                let named = matches!(
                    self.kind(id),
                    NodeKind::Stmt(StmtKind::Begin | StmtKind::Fork { .. })
                        | NodeKind::Var { .. }
                        | NodeKind::Net { .. }
                        | NodeKind::Array { .. }
                        | NodeKind::Param { .. }
                        | NodeKind::Port { .. }
                ) && !node.name.is_empty();
                if named {
                    path.push(node.name.clone());
                }
                path
            }
        }
    }

    fn vpi_full_name(&self, id: NodeId) -> String {
        self.vpi_path(id).join(SEPARATOR)
    }

    /// Declared `[left:right]` of the outermost packed dimension, or the
    /// implicit `[width-1:0]` of a vector without a declared range.
    fn vpi_packed_range(&self, id: NodeId, width: u32) -> Option<(i64, i64)> {
        let declared = self.db.packed_dimensions(id).and_then(|ranges| {
            ranges.first().and_then(|range| {
                Some((
                    i64::try_from(range.left).ok()?,
                    i64::try_from(range.right).ok()?,
                ))
            })
        });
        declared.or_else(|| (width > 1).then(|| (i64::from(width) - 1, 0)))
    }

    fn vpi_packed_dimension_count(&self, id: NodeId) -> usize {
        self.db.packed_dimensions(id).map_or(0, <[_]>::len)
    }

    /// Capture the source-owned hierarchy and storage identities used by the
    /// generated VPI catalog.  This runs after collection has assigned stable
    /// IR indices but before optimization can prune any otherwise dead signal.
    pub(in super::super) fn populate_vpi_metadata(
        &self,
        model: &mut IrModel,
        tops: &[NodeId],
    ) -> Result<(), String> {
        use std::collections::BTreeMap;

        fn reachable(db: &Db, node: NodeId, tops: &[NodeId]) -> bool {
            let mut current = Some(node);
            while let Some(id) = current {
                if tops.contains(&id) {
                    return true;
                }
                current = db.node(id).parent;
            }
            false
        }

        /// Whether `id` is a named statement block of a process (not the
        /// frontend's declaration scope of the same name, nor an instance
        /// body), so every named block is captured exactly once.
        fn statement_block(db: &Db, id: NodeId) -> bool {
            let mut current = db.node(id).parent;
            while let Some(parent) = current {
                match db.node(parent).kind {
                    NodeKind::Process { .. } => return true,
                    NodeKind::Stmt(_) => current = db.node(parent).parent,
                    _ => return false,
                }
            }
            false
        }

        let mut objects = BTreeMap::<String, IrVpiObject>::new();
        let mut insert = |object: IrVpiObject| {
            objects.entry(object.full_name.clone()).or_insert(object);
        };
        let static_locals = self
            .proc_local_instances
            .iter()
            .filter_map(|((_, node), info)| info.static_signal.as_ref().map(|s| (*node, s.ir)))
            .collect::<HashMap<_, _>>();

        for id in self.db.node_ids() {
            if !reachable(self.db, id, tops) {
                continue;
            }
            let node = self.node(id);
            let source = |object: IrVpiObject| IrVpiObject {
                file: node.file().map(str::to_owned),
                line: node.line,
                time_unit_fs: self.timescale_of_node(id).unit_fs,
                ..object
            };
            match self.kind(id) {
                NodeKind::ModuleInst {
                    def_name,
                    is_top,
                    is_interface,
                    ..
                } => {
                    let path = self.vpi_path(id);
                    let Some(name) = path.last().cloned() else {
                        continue;
                    };
                    insert(source(IrVpiObject {
                        full_name: path.join(SEPARATOR),
                        name,
                        definition_name: Some(def_name.clone()),
                        kind: if *is_interface {
                            IrVpiObjectKind::Interface
                        } else {
                            IrVpiObjectKind::Module
                        },
                        top: *is_top,
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::GenScope if !node.name.is_empty() => {
                    let path = self.vpi_path(id);
                    insert(source(IrVpiObject {
                        full_name: path.join(SEPARATOR),
                        name: path.last().cloned().unwrap_or_default(),
                        kind: IrVpiObjectKind::GenScope,
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::Stmt(kind @ (StmtKind::Begin | StmtKind::Fork { .. }))
                    if !node.name.is_empty() && statement_block(self.db, id) =>
                {
                    insert(source(IrVpiObject {
                        full_name: self.vpi_full_name(id),
                        name: node.name.clone(),
                        kind: if matches!(kind, StmtKind::Fork { .. }) {
                            IrVpiObjectKind::NamedFork
                        } else {
                            IrVpiObjectKind::NamedBegin
                        },
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::Net { ty, .. } | NodeKind::Var { ty }
                    if self.sig_globals.contains_key(&id) || static_locals.contains_key(&id) =>
                {
                    let ir = match self.sig_globals.get(&id) {
                        Some(info) => info.ir,
                        None => static_locals[&id],
                    };
                    let Some(signal) = model.signals.get(ir) else {
                        return Err(format!(
                            "VPI signal metadata references missing IR signal {ir} for `{}`",
                            node.full_name
                        ));
                    };
                    let (width, signed, real, two_state) = match signal.ty {
                        IrType::Packed {
                            width,
                            signed,
                            two_state,
                        } => (width, signed, false, two_state),
                        IrType::Real { .. } => (0, false, true, false),
                    };
                    let declared_net = matches!(self.kind(id), NodeKind::Net { .. });
                    let net =
                        declared_net || signal.net_driver.is_some() || !signal.net_alias.is_empty();
                    let kind = if real {
                        IrVpiObjectKind::RealVar
                    } else if net {
                        IrVpiObjectKind::Net
                    } else {
                        IrVpiObjectKind::Reg
                    };
                    let net_type = match self.kind(id) {
                        NodeKind::Net { net_type, .. } => Some(vpi_net_type(*net_type)),
                        _ => None,
                    };
                    insert(source(IrVpiObject {
                        full_name: self.vpi_full_name(id),
                        name: node.name.clone(),
                        kind,
                        width,
                        signed,
                        real,
                        net,
                        signal: Some(ir),
                        var_kind: IrVpiVarKind::from_type_kind(
                            &ty.kind,
                            self.vpi_packed_dimension_count(id),
                        ),
                        packed_range: (!real).then(|| self.vpi_packed_range(id, width)).flatten(),
                        net_type: net.then(|| net_type.unwrap_or("vpiWire")),
                        two_state,
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::Array { ty } => {
                    let Some(info) = self.array_globals.get(&id) else {
                        continue;
                    };
                    let Some(array) = model.arrays.get(info.ir) else {
                        return Err(format!(
                            "VPI array metadata references missing IR array {} for `{}`",
                            info.ir, node.full_name
                        ));
                    };
                    // Descriptor-backed and lexical storage has no stable
                    // dense element cells to expose.
                    if array.hdl_name.is_empty() || array.activation || array.descriptor {
                        continue;
                    }
                    let net = array.net.is_some();
                    insert(source(IrVpiObject {
                        full_name: self.vpi_full_name(id),
                        name: node.name.clone(),
                        kind: if net {
                            IrVpiObjectKind::NetArray
                        } else {
                            IrVpiObjectKind::RegArray
                        },
                        width: array.elem_width,
                        signed: array.signed,
                        real: array.real,
                        net,
                        array: Some(info.ir),
                        var_kind: IrVpiVarKind::from_type_kind(
                            &ty.kind,
                            self.vpi_packed_dimension_count(id),
                        ),
                        packed_range: (!array.real)
                            .then(|| self.vpi_packed_range(id, array.elem_width))
                            .flatten(),
                        net_type: net.then_some("vpiWire"),
                        two_state: array.two_state,
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::Param { ty, value, local } => {
                    let Some(value) = value else {
                        continue;
                    };
                    let (constant, width, signed, real) = match value {
                        Val::Bits(bits) => {
                            let digits = bits
                                .bits
                                .iter()
                                .map(|bit| match bit {
                                    Bit::Zero => '0',
                                    Bit::One => '1',
                                    Bit::X => 'x',
                                    Bit::Z => 'z',
                                })
                                .collect::<String>();
                            let width = u32::try_from(digits.len()).unwrap_or(u32::MAX);
                            if width == 0 || width > LLG_MAX_WIDTH {
                                continue;
                            }
                            (IrVpiConstant::Bits(digits), width, bits.signed, false)
                        }
                        Val::Real(real) => (IrVpiConstant::Real(*real), 0, false, true),
                        Val::Str(text) => {
                            if text.contains('\0') {
                                continue;
                            }
                            let width = u32::try_from(text.len().saturating_mul(8))
                                .unwrap_or(u32::MAX)
                                .min(LLG_MAX_WIDTH);
                            (IrVpiConstant::Text(text.clone()), width, false, false)
                        }
                    };
                    insert(source(IrVpiObject {
                        full_name: self.vpi_full_name(id),
                        name: node.name.clone(),
                        kind: IrVpiObjectKind::Parameter,
                        width,
                        signed,
                        real,
                        var_kind: IrVpiVarKind::from_type_kind(
                            &ty.kind,
                            self.vpi_packed_dimension_count(id),
                        ),
                        packed_range: (!real && matches!(value, Val::Bits(_)))
                            .then(|| self.vpi_packed_range(id, width))
                            .flatten(),
                        local_param: *local,
                        constant: Some(constant),
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::Port {
                    direction, low, ty, ..
                } => {
                    let Some(parent) = node.parent else {
                        continue;
                    };
                    let port_index = self
                        .node(parent)
                        .children
                        .iter()
                        .take_while(|child| **child != id)
                        .filter(|child| matches!(self.kind(**child), NodeKind::Port { .. }))
                        .count();
                    let width = ty.width.unwrap_or(0);
                    insert(source(IrVpiObject {
                        // A port and the declaration it connects share one
                        // name; the port lives in its own namespace.
                        full_name: format!("{}{SEPARATOR}\u{1e}port", self.vpi_full_name(id)),
                        name: node.name.clone(),
                        kind: IrVpiObjectKind::Port,
                        width,
                        signed: ty.signed,
                        direction: match direction {
                            Direction::Input => IrVpiDirection::Input,
                            Direction::Output => IrVpiDirection::Output,
                            Direction::Inout => IrVpiDirection::Inout,
                            _ => IrVpiDirection::None,
                        },
                        port_index: u32::try_from(port_index).unwrap_or(u32::MAX),
                        port_target: low.map(|low| self.vpi_full_name(low)),
                        ..IrVpiObject::default()
                    }));
                }
                _ => {}
            }
        }
        model.vpi_objects = objects.into_values().collect();
        Ok(())
    }

    /// Record the compile-time descriptor of one VPI system call site and
    /// return its stable site index.
    pub(in super::super) fn push_vpi_compile_call(
        &mut self,
        name: &str,
        call: NodeId,
        arg_nodes: &[NodeId],
        args: &[IrExpr],
    ) -> usize {
        let descriptors = arg_nodes
            .iter()
            .zip(args)
            .map(|(node, arg)| IrVpiCompileArg {
                width: arg.width,
                signed: arg.signed,
                real: arg.is_real(),
                reference: self.vpi_argument_reference(*node, arg),
            })
            .collect();
        let mut scope = self.node(call).parent;
        while let Some(id) = scope {
            let node = self.node(id);
            let is_scope = match self.kind(id) {
                NodeKind::ModuleInst { .. } => true,
                NodeKind::GenScope => !node.name.is_empty(),
                NodeKind::Stmt(StmtKind::Begin | StmtKind::Fork { .. }) => !node.name.is_empty(),
                _ => false,
            };
            if is_scope {
                break;
            }
            scope = node.parent;
        }
        let site = self.model.vpi_compile_calls.len();
        let mut descriptor = IrVpiCompileCall::new(name.to_owned(), descriptors);
        descriptor.time_unit_fs = self.timescale_of_node(call).unit_fs;
        descriptor.scope = scope.map(|id| self.vpi_full_name(id));
        descriptor.file = self.node(call).file().map(str::to_owned);
        descriptor.line = self.node(call).line;
        self.model.vpi_compile_calls.push(descriptor);
        site
    }

    /// Classify one VPI call argument by the source expression it names.
    /// Only selections fixed at elaboration become object references; any
    /// other expression is a read-only snapshot, as for a C caller of a
    /// system function given an expression actual. The source node, not the
    /// lowered value, decides: formal-type conversions wrap the lowered form.
    pub(in super::super) fn vpi_argument_reference(
        &self,
        node: NodeId,
        lowered: &IrExpr,
    ) -> IrVpiArgRef {
        let node = self.vpi_strip_conversions(node);
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type, source, ..
            }) => IrVpiArgRef::Constant(vpi_const_type(*const_type, source)),
            NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. }) => {
                let target = self.vpi_reference_target(node);
                // A parameter actual is the catalog vpiParameter object.
                if let Some(parameter) = target.filter(|target| {
                    matches!(self.kind(*target), NodeKind::Param { value: Some(_), .. })
                }) {
                    return IrVpiArgRef::Object(self.vpi_full_name(parameter));
                }
                let visible = target.filter(|target| {
                    self.sig_globals.contains_key(target) || self.array_globals.contains_key(target)
                });
                match (visible, &lowered.kind) {
                    (Some(target), _) if !self.array_globals.contains_key(&target) => {
                        IrVpiArgRef::Object(self.vpi_full_name(target))
                    }
                    (_, IrExprKind::SigRead(signal)) => self
                        .sig_global_for_ir(*signal)
                        .map(|id| self.vpi_full_name(id))
                        .map_or(IrVpiArgRef::Expression, IrVpiArgRef::Object),
                    _ => IrVpiArgRef::Expression,
                }
            }
            _ => self.vpi_select_reference(node),
        }
    }

    /// A literal under its conversion to the formal type. The owned database
    /// does not mark implicit conversions apart from explicit casts, so a
    /// converted variable stays an expression (a read-only snapshot) rather
    /// than becoming a writable object the application did not pass.
    fn vpi_strip_conversions(&self, node: NodeId) -> NodeId {
        let mut inner = node;
        while let NodeKind::Expr(ExprKind::Cast {
            operand,
            size_cast: false,
            ..
        }) = self.kind(inner)
        {
            inner = *operand;
        }
        if matches!(self.kind(inner), NodeKind::Expr(ExprKind::Constant { .. })) {
            inner
        } else {
            node
        }
    }

    /// An index fixed at elaboration: a literal or a parameter reference.
    fn vpi_constant_index(&self, index: NodeId) -> Option<i64> {
        match self.kind(index) {
            NodeKind::Expr(ExprKind::Constant { value, size, .. }) => {
                let constant = read_const_from(value, *size).ok()?;
                if constant.real.is_some()
                    || constant.x.iter().chain(&constant.z).any(|word| *word != 0)
                    || constant.width > 63
                {
                    return None;
                }
                let word = constant.bits.first().copied().unwrap_or(0);
                let value = if constant.signed && constant.width > 0 && constant.width < 64 {
                    let shift = 64 - constant.width;
                    ((word << shift) as i64) >> shift
                } else {
                    i64::try_from(word).ok()?
                };
                Some(value)
            }
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => match self.kind(*target) {
                NodeKind::Param {
                    value: Some(Val::Bits(bits)),
                    ..
                } => bits.to_i64(),
                _ => None,
            },
            _ => None,
        }
    }

    /// The declaration an argument names: through a reference expression,
    /// or the declaration itself when a select's base points at it directly.
    fn vpi_reference_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
            NodeKind::Expr(_) => None,
            _ => Some(node),
        }
    }

    /// A constant bit/part-select of a whole variable or net, or a
    /// constant-index element of a catalog array.
    fn vpi_select_reference(&self, node: NodeId) -> IrVpiArgRef {
        let constant_index = |index: NodeId| self.vpi_constant_index(index);
        let single = match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                Some((*base, indices[0]))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => Some((*base, *index)),
            _ => None,
        };
        match self.kind(node) {
            _ if single.is_some() => {
                let Some((base, index)) = single else {
                    return IrVpiArgRef::Expression;
                };
                let Some(target) = self.vpi_reference_target(base) else {
                    return IrVpiArgRef::Expression;
                };
                let Some(index) = constant_index(index) else {
                    return IrVpiArgRef::Expression;
                };
                if let Some(array) = self
                    .array_globals
                    .get(&target)
                    .and_then(|info| self.model.arrays.get(info.ir))
                {
                    let Some(&(left, right)) = array.dims.first() else {
                        return IrVpiArgRef::Expression;
                    };
                    if array.dims.len() != 1 {
                        return IrVpiArgRef::Expression;
                    }
                    let (low, high) = (i64::from(left.min(right)), i64::from(left.max(right)));
                    if index < low || index > high {
                        return IrVpiArgRef::Expression;
                    }
                    let offset = if left <= right {
                        index - i64::from(left)
                    } else {
                        i64::from(left) - index
                    };
                    return IrVpiArgRef::Element {
                        object: self.vpi_full_name(target),
                        index: offset.unsigned_abs(),
                    };
                }
                let Some(info) = self.sig_globals.get(&target) else {
                    return IrVpiArgRef::Expression;
                };
                if self.vpi_packed_dimension_count(target) > 1 {
                    return IrVpiArgRef::Expression;
                }
                let Some((left, right)) = self.vpi_packed_range(target, info.width) else {
                    return IrVpiArgRef::Expression;
                };
                match packed_offset(left, right, index) {
                    Some(lsb) => IrVpiArgRef::Select {
                        object: self.vpi_full_name(target),
                        lsb,
                        width: 1,
                        left: index,
                        right: index,
                    },
                    None => IrVpiArgRef::Expression,
                }
            }
            NodeKind::Expr(ExprKind::PartSelect {
                base, left, right, ..
            }) => {
                let Some(target) = self.vpi_reference_target(*base) else {
                    return IrVpiArgRef::Expression;
                };
                let Some(info) = self.sig_globals.get(&target) else {
                    return IrVpiArgRef::Expression;
                };
                if self.vpi_packed_dimension_count(target) > 1 {
                    return IrVpiArgRef::Expression;
                }
                let (Some(select_left), Some(select_right)) =
                    (constant_index(*left), constant_index(*right))
                else {
                    return IrVpiArgRef::Expression;
                };
                let Some((declared_left, declared_right)) =
                    self.vpi_packed_range(target, info.width)
                else {
                    return IrVpiArgRef::Expression;
                };
                let (Some(a), Some(b)) = (
                    packed_offset(declared_left, declared_right, select_left),
                    packed_offset(declared_left, declared_right, select_right),
                ) else {
                    return IrVpiArgRef::Expression;
                };
                IrVpiArgRef::Select {
                    object: self.vpi_full_name(target),
                    lsb: a.min(b),
                    width: a.abs_diff(b) + 1,
                    left: select_left,
                    right: select_right,
                }
            }
            _ => IrVpiArgRef::Expression,
        }
    }
}

/// `vpiConstType` code name of a literal argument. Integral literals keep
/// their radix only in the source spelling (`8'hff`, `3'b1x0`, `12`).
fn vpi_const_type(const_type: ConstantType, source: &ConstantSource) -> &'static str {
    match const_type {
        ConstantType::String => "vpiStringConst",
        ConstantType::Real => "vpiRealConst",
        ConstantType::Time => "vpiTimeConst",
        _ => {
            let ConstantSource::Exact(spelling) = source else {
                return "vpiDecConst";
            };
            let Some(quote) = spelling.find('\'') else {
                return "vpiDecConst";
            };
            let radix = spelling[quote + 1..]
                .trim_start_matches(['s', 'S'])
                .chars()
                .next()
                .map(|radix| radix.to_ascii_lowercase());
            match radix {
                Some('b') => "vpiBinaryConst",
                Some('o') => "vpiOctConst",
                Some('h') => "vpiHexConst",
                Some('d') => "vpiDecConst",
                // Unbased unsized fills ('0, '1, 'x, 'z) are bit patterns.
                _ => "vpiBinaryConst",
            }
        }
    }
}

/// Storage offset of index `index` in a packed range `[left:right]`.
fn packed_offset(left: i64, right: i64, index: i64) -> Option<u32> {
    let (low, high) = (left.min(right), left.max(right));
    if index < low || index > high {
        return None;
    }
    let offset = if left >= right {
        index - right
    } else {
        right - index
    };
    u32::try_from(offset).ok()
}

fn vpi_net_type(net_type: NetType) -> &'static str {
    match net_type {
        NetType::Wand => "vpiWand",
        NetType::Wor => "vpiWor",
        NetType::Tri => "vpiTri",
        NetType::Tri0 => "vpiTri0",
        NetType::Tri1 => "vpiTri1",
        NetType::TriReg => "vpiTriReg",
        NetType::TriAnd => "vpiTriAnd",
        NetType::TriOr => "vpiTriOr",
        NetType::Supply1 => "vpiSupply1",
        NetType::Supply0 => "vpiSupply0",
        NetType::Uwire => "vpiUwire",
        _ => "vpiWire",
    }
}

#[cfg(test)]
mod tests {
    use super::{packed_offset, vpi_const_type};
    use crate::core::db::{ConstantSource, ConstantType};

    #[test]
    fn literal_radix_selects_the_constant_type() {
        let exact = |text: &str| ConstantSource::Exact(text.to_owned());
        let integer = ConstantType::Integer;
        assert_eq!(vpi_const_type(integer, &exact("3'b1x0")), "vpiBinaryConst");
        assert_eq!(vpi_const_type(integer, &exact("12'sd10")), "vpiDecConst");
        assert_eq!(vpi_const_type(integer, &exact("8'HfF")), "vpiHexConst");
        assert_eq!(vpi_const_type(integer, &exact("'o17")), "vpiOctConst");
        assert_eq!(vpi_const_type(integer, &exact("'1")), "vpiBinaryConst");
        assert_eq!(vpi_const_type(integer, &exact("42")), "vpiDecConst");
        assert_eq!(
            vpi_const_type(integer, &ConstantSource::Unavailable),
            "vpiDecConst"
        );
        assert_eq!(
            vpi_const_type(ConstantType::String, &exact("\"t\"")),
            "vpiStringConst"
        );
        assert_eq!(
            vpi_const_type(ConstantType::Real, &exact("1.5")),
            "vpiRealConst"
        );
    }

    #[test]
    fn packed_offsets_follow_declared_direction() {
        assert_eq!(packed_offset(7, 0, 7), Some(7));
        assert_eq!(packed_offset(7, 0, 0), Some(0));
        assert_eq!(packed_offset(5, 2, 2), Some(0));
        assert_eq!(packed_offset(0, 7, 0), Some(7));
        assert_eq!(packed_offset(0, 7, 7), Some(0));
        assert_eq!(packed_offset(7, 0, 8), None);
        assert_eq!(packed_offset(5, 2, 1), None);
    }
}
