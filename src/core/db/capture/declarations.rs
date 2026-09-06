use std::collections::HashSet;
use std::os::raw::c_int;

use crate::core::elab::{self, Val};
use crate::core::model::TypeInfo;
use crate::ffi::vpi::{self, OwnedHandle, ValueData, VpiHandle};

use super::super::capture;
use super::super::database::*;
use super::super::*;

pub(in crate::core::db) fn direction(handle: VpiHandle) -> Direction {
    Direction::from_raw(vpi::get(vpi::vpiDirection, handle))
}

pub(in crate::core::db) fn net_type(handle: VpiHandle) -> NetType {
    NetType::from_raw(vpi::get(vpi::vpiNetType, handle))
}

impl Builder {
    pub(in crate::core::db) fn enclosing_typedef_info(
        &mut self,
        context: VpiHandle,
        name: &str,
    ) -> Option<(TypeInfo, bool)> {
        let mut scope = child(vpi::vpiParent, context);
        let mut hops = 0usize;
        loop {
            let current = scope.as_ref()?;
            for typedef in iter(vpi::vpiTypedef, current.raw()) {
                if vpi::obj_name(typedef.raw()) == name {
                    let ty = self.typespec_info(typedef.raw());
                    let two_state = self.typespec_two_state(typedef.raw(), 0);
                    return Some((ty, two_state));
                }
            }
            if hops == 32 {
                return None;
            }
            scope = current.child(vpi::vpiParent);
            hops += 1;
        }
    }

    pub(in crate::core::db) fn walk_enum_const(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let value = elab::read_value(h).ok();
        let id = self.register(parent, &props, NodeKind::EnumConst { value });
        self.index_node(h, &props, id);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_port(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let direction = capture::declarations::direction(h);
        // The high connection is kept as raw presence facts alongside the
        // resolved target: `high: None` alone cannot distinguish an omitted
        // port from `.p()` (explicit-empty marker) or an expression
        // connection (see [`NodeKind::Port`]).
        let high_conn = child(vpi::vpiHighConn, h);
        let high_present = high_conn.is_some();
        let high_open = high_conn
            .as_ref()
            .is_some_and(|c| Self::is_empty_conn_marker(c.raw()));
        let high = high_conn.as_ref().and_then(|c| self.resolve_ref(c.raw()));
        let low = self.conn_ref(h, vpi::vpiLowConn);
        let id = self.register(
            parent,
            &props,
            NodeKind::Port {
                direction,
                high,
                low,
                high_expr: None,
                high_present,
                high_open,
            },
        );
        // Keep the full high-side expression owned by the port.  This is a
        // separate view from `high`: direct refs still resolve to their
        // declaration target, while operations and other expressions retain
        // every operand for consumers that need parent-side reads.
        let high_expr = match high_conn.as_ref() {
            Some(conn) if !high_open => Some(self.walk_node(conn.raw(), Some(id))?),
            _ => None,
        };
        self.set_kind(
            id,
            NodeKind::Port {
                direction,
                high,
                low,
                high_expr,
                high_present,
                high_open,
            },
        );
        let mut kids: Vec<NodeId> = Vec::new();
        // Interface-typed ports record the connected actual interface instance
        // and the (possibly empty) modport name as a child node.
        if let Some((actual, modport)) = self.interface_conn(h) {
            let cid = self.register(
                Some(id),
                &CommonProps::default(),
                NodeKind::IfaceConn { actual, modport },
            );
            kids.push(cid);
        }
        // Keep the connection expression in the child list as well as in the
        // port variant so the owned tree remains reachable.  In particular,
        // codegen uses a captured array-select child to address unpacked array
        // elements in port links.
        if let Some(expr) = high_expr {
            kids.push(expr);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    pub(in crate::core::db) fn conn_ref(&self, h: VpiHandle, rel: c_int) -> Option<NodeId> {
        match child(rel, h) {
            Some(c) => self.resolve_ref(c.raw()),
            None => None,
        }
    }

    pub(in crate::core::db) fn is_empty_conn_marker(h: VpiHandle) -> bool {
        if vpi::obj_type(h) != vpi::vpiOperation {
            return false;
        }
        if vpi::get(vpi::vpiOpType, h) != vpi::vpiNullOp {
            return false;
        }
        match vpi::iterate(vpi::vpiOperand, h) {
            Some(it) => it.count() == 0,
            None => true,
        }
    }

    pub(in crate::core::db) fn interface_conn(&self, h: VpiHandle) -> Option<(NodeId, String)> {
        let hc = child(vpi::vpiHighConn, h)?;
        match vpi::obj_type(hc.raw()) {
            // `u_bus.master`: the first ref resolves to the actual interface
            // instance, the last ref's name is the modport name.  A
            // hier_path's `vpiActual` is 1-to-many (one ref_obj per element).
            vpi::vpiHierPath => {
                let mut names: Vec<String> = Vec::new();
                let mut actual: Option<NodeId> = None;
                for a in iter(vpi::vpiActual, hc.raw()) {
                    let a = a.raw();
                    let n = vpi::obj_name(a);
                    if !n.is_empty() {
                        names.push(n);
                    }
                    if actual.is_none() {
                        actual = self.resolve_ref(a).filter(|t| {
                            matches!(
                                self.nodes[t.0 as usize].kind,
                                NodeKind::ModuleInst {
                                    is_interface: true,
                                    ..
                                }
                            )
                        });
                    }
                }
                Some((actual?, names.last().cloned().unwrap_or_default()))
            }
            // `u_bus` (bare interface): the ref's actual is the interface.
            vpi::vpiRefObj => {
                let actual = self.resolve_ref(hc.raw())?;
                matches!(
                    self.nodes[actual.0 as usize].kind,
                    NodeKind::ModuleInst {
                        is_interface: true,
                        ..
                    }
                )
                .then_some((actual, String::new()))
            }
            _ => None,
        }
    }

    pub(in crate::core::db) fn walk_modport(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::ModPort);
        // Modports have no `vpiFullName`; index them under the owning
        // interface's full name plus `.` plus the modport name so port
        // `low`/`vpiActual` refs into them resolve (see `resolve_ref`).
        if let Some(iface) = child(vpi::vpiInterface, h) {
            let full = vpi::obj_full_name(iface.raw());
            if !full.is_empty() && !props.name.is_empty() {
                self.index
                    .insert((vpi::vpiModport, format!("{full}.{}", props.name)), id);
            }
        }
        let mut kids: Vec<NodeId> = Vec::new();
        for io in iter(vpi::vpiIODecl, h) {
            kids.push(self.walk_io_decl(io.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_io_decl(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let direction = capture::declarations::direction(h);
        // On per-port copies the io_decl's `vpiExpr` is the copy's own var;
        // on the actual interface instance there is no expr.
        let expr = child(vpi::vpiExpr, h).and_then(|e| {
            self.resolve_ref(e.raw())
                .or_else(|| self.resolve_direct(e.raw()))
        });
        let id = self.register(parent, &props, NodeKind::IoDecl { direction, expr });
        Ok(id)
    }

    pub(in crate::core::db) fn walk_net(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let ty = self.type_info_of(h);
        let net_type = capture::declarations::net_type(h);
        let id = self.register(parent, &props, NodeKind::Net { ty, net_type });
        if self.object_two_state(h) {
            self.two_state_types.insert(id);
        }
        self.index_node(h, &props, id);
        if let Some(typespec) = child(vpi::vpiTypespec, h).or_else(|| child(vpi::vpiTypedef, h)) {
            if let Some((layout, packed_members)) = self.aggregate_layout(typespec.raw(), h) {
                self.aggregate_layouts.insert(id, layout);
                if let Some(members) = packed_members {
                    self.packed_members.insert(id, members);
                }
            }
            if let Some(dimensions) = self.contextual_packed_ranges(typespec.raw(), h) {
                self.packed_dimensions.insert(id, dimensions);
            }
        }
        Ok(id)
    }

    pub(in crate::core::db) fn walk_var(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        // Array objects reached outside `walk_module_inst` (function-body
        // locals, block locals) keep the Array kind so consumers skip them
        // the same way they always did.
        if is_array_type(vpi::obj_type(h)) {
            return self.walk_array(h, parent, false);
        }
        let props = self.common(h);
        let ty = self.type_info_of(h);
        let id = self.register(parent, &props, NodeKind::Var { ty });
        let lifetime = self.variable_lifetime_qualifier(h, parent);
        self.var_lifetime_qualifiers.insert(id, lifetime);
        if self.object_two_state(h) {
            self.two_state_types.insert(id);
        }
        self.index_node(h, &props, id);
        if let Some(typespec) = child(vpi::vpiTypespec, h).or_else(|| child(vpi::vpiTypedef, h)) {
            if let Some((layout, packed_members)) = self.aggregate_layout(typespec.raw(), h) {
                self.aggregate_layouts.insert(id, layout);
                if let Some(members) = packed_members {
                    self.packed_members.insert(id, members);
                }
            }
            if let Some(dimensions) = self.contextual_packed_ranges(typespec.raw(), h) {
                self.packed_dimensions.insert(id, dimensions);
            }
        }
        // Declaration initializer (`logic l = 1'b0;`, `int x = 5;`): the
        // value lives on the var's `vpiExpr` child (a constant or a constant
        // expression), unlike `reg`/`wire` initializers which Surelog models
        // as `vpiNetDeclAssign` continuous assignments (see
        // `walk_cont_assign`).  Walked as the var's only child so its
        // operands are captured; the simulator codegen reads it from
        // `Db::vars_init`.
        if let Some(e) = child(vpi::vpiExpr, h) {
            let eid = self.walk_node(e.raw(), Some(id))?;
            self.set_children(id, vec![eid]);
            self.vars_init.insert(id, eid);
        }
        Ok(id)
    }

    pub(in crate::core::db) fn walk_named_event(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::NamedEvent);
        self.index_node(h, &props, id);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_array(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
        is_net: bool,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        // Element type: `vpiReg` (array_var/reg_array) or `vpiNet` (array_net)
        // child's typespec — UHDM exposes those as 1-to-many vectors, so
        // iterate and take the first element.  Fall back to the array's own
        // typespec when absent.
        let el = iter(if is_net { vpi::vpiNet } else { vpi::vpiReg }, h)
            .into_iter()
            .next();
        let net_type = el
            .as_ref()
            .filter(|_| is_net)
            .map(|e| capture::declarations::net_type(e.raw()));
        let element_handle = el.as_ref().map(|element| element.raw()).unwrap_or(h);
        let ty = match &el {
            Some(e) => self.type_info_of(e.raw()),
            None => self.type_info_of(h),
        };
        let mut dims: Vec<Option<(i32, i32)>> = Vec::new();
        for r in iter(vpi::vpiRange, h) {
            let r = r.raw();
            let left = self.range_bound(vpi::vpiLeftRange, r);
            let right = self.range_bound(vpi::vpiRightRange, r);
            dims.push(match (left, right) {
                (Some(l), Some(rr)) => Some((l as i32, rr as i32)),
                _ => None,
            });
        }
        let array_type = vpi::get(vpi::vpiArrayType, h);
        let queue_maximum_elements = if array_type == vpi::vpiQueueArray {
            iter(vpi::vpiRange, h).into_iter().next().and_then(|range| {
                self.range_bound(vpi::vpiRightRange, range.raw())
                    .and_then(|right| u64::try_from(right).ok())
                    .and_then(|upper| upper.checked_add(1))
            })
        } else {
            None
        };
        let kind = match array_type {
            vpi::vpiDynamicArray => ArrayKind::Dynamic,
            vpi::vpiAssocArray => ArrayKind::Associative(self.associative_index(h)),
            vpi::vpiQueueArray => ArrayKind::Queue {
                maximum_elements: queue_maximum_elements,
            },
            _ => ArrayKind::Static,
        };
        let id = self.register(parent, &props, NodeKind::Array { ty });
        if self.object_two_state(element_handle) {
            self.two_state_types.insert(id);
        }
        self.index_node(h, &props, id);
        // Declaration initializer (`= '{…}`): an assignment-pattern operation
        // under `vpiExpr`.  Walked as the array's only child so its constant
        // operands are captured; the simulator codegen reads them from
        // `ArrayMeta.init`.
        let mut kids: Vec<NodeId> = Vec::new();
        let init = match child(vpi::vpiExpr, h) {
            Some(e) => {
                let eid = self.walk_node(e.raw(), Some(id))?;
                kids.push(eid);
                Some(eid)
            }
            None => None,
        };
        self.set_children(id, kids);
        self.arrays.insert(
            id,
            ArrayMeta {
                kind,
                dims,
                init,
                net_type,
            },
        );
        Ok(id)
    }

    fn associative_index(&mut self, array: VpiHandle) -> AssociativeIndex {
        let Some(root) = child(vpi::vpiTypespec, array) else {
            return AssociativeIndex::Wildcard;
        };
        let mut current = root;
        for _ in 0..=32 {
            if vpi::obj_type(current.raw()) != vpi::vpiRefTypespec {
                let Some(index) = current.child(vpi::vpiIndexTypespec) else {
                    return AssociativeIndex::Wildcard;
                };
                let info = self.typespec_info(index.raw());
                return match info.kind.as_str() {
                    "string" => AssociativeIndex::String,
                    "logic" | "bit" | "int" | "integer" | "longint" | "byte" | "shortint"
                    | "time" | "enum" => info
                        .width
                        .map(|width| AssociativeIndex::Integral {
                            width,
                            signed: info.signed,
                            two_state: self.typespec_two_state(index.raw(), 0),
                        })
                        .unwrap_or_else(|| AssociativeIndex::Unsupported(info.kind.clone())),
                    kind => AssociativeIndex::Unsupported(kind.to_string()),
                };
            }
            let Some(next) = current.child(vpi::vpiActual) else {
                return AssociativeIndex::Wildcard;
            };
            current = next;
        }
        AssociativeIndex::Unsupported("recursive index typespec".to_string())
    }

    pub(in crate::core::db) fn walk_task_func(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let is_task = vpi::obj_type(h) == vpi::vpiTask;
        let automatic = vpi::get(vpi::vpiAutomatic, h) != 0;
        let id = self.register(
            parent,
            &props,
            NodeKind::FuncTask {
                is_task,
                automatic,
                ret: None,
            },
        );
        self.index_node(h, &props, id);

        let mut kids: Vec<NodeId> = Vec::new();
        // Return variable: the implicit function-name variable.  Void
        // functions (`function void`) and tasks have no `vpiReturn`.
        let mut ret_ty: Option<TypeInfo> = None;
        if let Some(rv) = child(vpi::vpiReturn, h) {
            let ty = self.type_info_of(rv.raw());
            // Key the return var by the function name; its own (buggy)
            // full name equals the function's full name.
            let rprops = CommonProps {
                name: props.name.clone(),
                full_name: props.full_name.clone(),
                file: props.file.clone(),
                line: props.line,
                col: props.col,
                end_line: props.end_line,
                end_col: props.end_col,
            };
            let rv_id = self.register(Some(id), &rprops, NodeKind::Var { ty: ty.clone() });
            if self.object_two_state(rv.raw()) {
                self.two_state_types.insert(rv_id);
            }
            self.index
                .insert((vpi::obj_type(rv.raw()), props.full_name.clone()), rv_id);
            kids.push(rv_id);
            ret_ty = Some(ty);
        }
        // Formal arguments, in declaration order.
        for io in iter(vpi::vpiIODecl, h) {
            let io = io.raw();
            let direction = capture::declarations::direction(io);
            let ty = self.type_info_of(io);
            let aprops = self.common(io);
            let aid = self.register(
                Some(id),
                &aprops,
                NodeKind::FuncArg {
                    direction,
                    ty,
                    default: None,
                },
            );
            if self.object_two_state(io) {
                self.two_state_types.insert(aid);
            }
            // The default-value expression (`input int a = 7`) is captured
            // as the FuncArg's only child.
            if let Some(d) = child(vpi::vpiExpr, io) {
                let did = self.walk_node(d.raw(), Some(aid))?;
                self.set_children(aid, vec![did]);
                if let NodeKind::FuncArg { default, .. } = &mut self.nodes[aid.0 as usize].kind {
                    *default = Some(did);
                }
            }
            kids.push(aid);
        }
        // Body statement.  Surelog emits no `vpiStmt` for an empty
        // function/task body (`function void vf(...); endfunction`), so a
        // placeholder empty statement is registered to keep a body child
        // present (codegen picks the last statement child of the function).
        match child(vpi::vpiStmt, h) {
            Some(stmt) => kids.push(self.walk_node(stmt.raw(), Some(id))?),
            None => kids.push(self.register(
                Some(id),
                &CommonProps::default(),
                NodeKind::Stmt(StmtKind::Empty),
            )),
        }
        self.set_children(id, kids);
        if let NodeKind::FuncTask { ret, .. } = &mut self.nodes[id.0 as usize].kind {
            *ret = ret_ty;
        }
        Ok(id)
    }

    pub(in crate::core::db) fn walk_param(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
        value: Option<Val>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let ty = self.type_info_of(h);
        let local = vpi::get(vpi::vpiLocalParam, h) != 0;
        let id = self.register(parent, &props, NodeKind::Param { ty, value, local });
        self.index_node(h, &props, id);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_param_assign(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let overridden = vpi::get(vpi::vpiOverriden, h) != 0;
        let id = self.register(parent, &props, NodeKind::ParamAssign { overridden });
        let mut kids: Vec<NodeId> = Vec::new();
        if let Some(lhs) = child(vpi::vpiLhs, h) {
            kids.push(self.walk_node(lhs.raw(), Some(id))?);
        }
        if let Some(rhs) = child(vpi::vpiRhs, h) {
            kids.push(self.walk_node(rhs.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    pub(in crate::core::db) fn type_info_of(&mut self, h: VpiHandle) -> TypeInfo {
        let typespec = child(vpi::vpiTypespec, h).or_else(|| child(vpi::vpiTypedef, h));
        let mut ty = match &typespec {
            Some(ts) => self.typespec_info(ts.raw()),
            None => TypeInfo::default(),
        };
        ty.signed |= vpi::get(vpi::vpiSigned, h) != 0;
        if ty.width.is_none() {
            if let Some(typespec) = &typespec {
                ty.width = self.contextual_typespec_width(typespec.raw(), h);
            }
        }
        if ty.kind != "other" {
            return ty;
        }
        // Surelog v1.86 exposes a shortreal variable as vpiShortRealVar but
        // does not attach a vpiTypespec handle. Preserve the scalar type from
        // the object discriminator instead of degrading it to `other`.
        match vpi::obj_type(h) {
            vpi::vpiRealVar => TypeInfo {
                kind: "real".to_string(),
                width: None,
                signed: true,
                type_name: None,
            },
            vpi::vpiShortRealVar => TypeInfo {
                kind: "shortreal".to_string(),
                width: None,
                signed: true,
                type_name: None,
            },
            vpi::vpiChandleVar => TypeInfo {
                kind: "chandle".to_string(),
                width: None,
                signed: false,
                type_name: None,
            },
            _ => ty,
        }
    }

    pub(in crate::core::db) fn object_two_state(&mut self, object: VpiHandle) -> bool {
        child(vpi::vpiTypespec, object)
            .or_else(|| child(vpi::vpiTypedef, object))
            .is_some_and(|typespec| self.typespec_two_state(typespec.raw(), 0))
    }

    pub(in crate::core::db) fn typespec_two_state(
        &mut self,
        typespec: VpiHandle,
        hops: usize,
    ) -> bool {
        if hops > 32 {
            return false;
        }
        match vpi::obj_type(typespec) {
            vpi::vpiBitTypespec
            | vpi::vpiIntTypespec
            | vpi::vpiLongIntTypespec
            | vpi::vpiByteTypespec
            | vpi::vpiShortIntTypespec => true,
            vpi::vpiRefTypespec => child(vpi::vpiActual, typespec)
                .is_some_and(|actual| self.typespec_two_state(actual.raw(), hops + 1)),
            vpi::vpiEnumTypespec => child(vpi::vpiBaseTypespec, typespec)
                .is_some_and(|base| self.typespec_two_state(base.raw(), hops + 1)),
            vpi::vpiPackedArrayTypespec => child(vpi::vpiElemTypespec, typespec)
                .is_some_and(|element| self.typespec_two_state(element.raw(), hops + 1)),
            vpi::vpiStructTypespec | vpi::vpiUnionTypespec => {
                let members = iter(vpi::vpiTypespecMember, typespec);
                !members.is_empty()
                    && members.into_iter().all(|member| {
                        child(vpi::vpiTypespec, member.raw()).is_some_and(|member_type| {
                            self.typespec_two_state(member_type.raw(), hops + 1)
                        })
                    })
            }
            _ => false,
        }
    }

    pub(in crate::core::db) fn typespec_info(&mut self, ts: VpiHandle) -> TypeInfo {
        let mut visited: HashSet<(i32, String)> = HashSet::new();
        let mut current: Option<OwnedHandle> = None;
        let mut signed = false;
        loop {
            let cur = current.as_ref().map_or(ts, OwnedHandle::raw);
            signed |= vpi::get(vpi::vpiSigned, cur) != 0;
            if vpi::obj_type(cur) != vpi::vpiRefTypespec {
                let mut ty = self.concrete_typespec(cur);
                ty.signed |= signed;
                return ty;
            }
            let key = (vpi::obj_type(cur), vpi::obj_full_name(cur));
            if !visited.insert(key) {
                return TypeInfo::default();
            }
            let next = match current.as_ref() {
                Some(owner) => owner.child(vpi::vpiActual),
                None => child(vpi::vpiActual, ts),
            };
            match next {
                Some(actual) => current = Some(actual),
                None => return TypeInfo::default(),
            }
        }
    }

    /// Recover a parameter-dependent packed width using the expression's
    /// enclosing elaborated scope. Surelog retains typedef range expressions
    /// such as `WIDTH-1` even when it omits the cast operation's `vpiSize`.
    pub(in crate::core::db) fn contextual_typespec_width(
        &mut self,
        ts: VpiHandle,
        context: VpiHandle,
    ) -> Option<u32> {
        let mut current: Option<OwnedHandle> = None;
        let mut hops = 0usize;
        loop {
            let cur = current.as_ref().map_or(ts, OwnedHandle::raw);
            if vpi::obj_type(cur) != vpi::vpiRefTypespec {
                return match vpi::obj_type(cur) {
                    vpi::vpiLogicTypespec | vpi::vpiBitTypespec | vpi::vpiPackedArrayTypespec => {
                        self.contextual_range_width(cur, context)
                    }
                    vpi::vpiStructTypespec => {
                        if vpi::get(vpi::vpiPacked, cur) == 0 {
                            return None;
                        }
                        let mut total = 0u32;
                        let mut any = false;
                        for member in iter(vpi::vpiTypespecMember, cur) {
                            let member = child(vpi::vpiTypespec, member.raw())?;
                            total = total.checked_add(
                                self.contextual_typespec_width(member.raw(), context)?,
                            )?;
                            any = true;
                        }
                        any.then_some(total)
                    }
                    vpi::vpiUnionTypespec => {
                        if vpi::get(vpi::vpiPacked, cur) == 0 {
                            return None;
                        }
                        let mut width = 0u32;
                        let mut any = false;
                        for member in iter(vpi::vpiTypespecMember, cur) {
                            let member = child(vpi::vpiTypespec, member.raw())?;
                            width =
                                width.max(self.contextual_typespec_width(member.raw(), context)?);
                            any = true;
                        }
                        any.then_some(width)
                    }
                    vpi::vpiEnumTypespec => child(vpi::vpiBaseTypespec, cur)
                        .and_then(|base| self.contextual_typespec_width(base.raw(), context)),
                    _ => self.concrete_typespec(cur).width,
                };
            }
            if hops == 16 {
                return None;
            }
            current = match current.as_ref() {
                Some(owner) => owner.child(vpi::vpiActual),
                None => child(vpi::vpiActual, ts),
            };
            current.as_ref()?;
            hops += 1;
        }
    }

    fn aggregate_layout(
        &mut self,
        typespec: VpiHandle,
        context: VpiHandle,
    ) -> Option<(AggregateLayout, Option<Vec<PackedMember>>)> {
        let mut current: Option<OwnedHandle> = None;
        let mut hops = 0usize;
        loop {
            let cur = current.as_ref().map_or(typespec, OwnedHandle::raw);
            if vpi::obj_type(cur) != vpi::vpiRefTypespec {
                let is_union = vpi::obj_type(cur) == vpi::vpiUnionTypespec;
                if !is_union && vpi::obj_type(cur) != vpi::vpiStructTypespec {
                    return None;
                }
                let is_packed = vpi::get(vpi::vpiPacked, cur) != 0;
                let is_tagged = is_union && vpi::get(vpi::vpiTagged, cur) != 0;
                let kind = if is_tagged {
                    AggregateKind::TaggedUnion
                } else if is_union && is_packed {
                    AggregateKind::PackedUnion
                } else if is_union {
                    AggregateKind::UnpackedUnion
                } else if is_packed {
                    AggregateKind::PackedStruct
                } else {
                    AggregateKind::UnpackedStruct
                };
                let mut aggregate_members = Vec::new();
                for member in iter(vpi::vpiTypespecMember, cur) {
                    let name = vpi::obj_name(member.raw());
                    if name.is_empty() {
                        return None;
                    }
                    let member_type = child(vpi::vpiTypespec, member.raw())?;
                    let mut ty = self.typespec_info(member_type.raw());
                    if ty.width.is_none() {
                        ty.width = self.contextual_typespec_width(member_type.raw(), context);
                    }
                    let two_state = self.typespec_two_state(member_type.raw(), 0);
                    let aggregate = self
                        .aggregate_layout(member_type.raw(), context)
                        .map(|(layout, _)| Box::new(layout));
                    let packed_ranges =
                        match self.contextual_packed_ranges(member_type.raw(), context) {
                            Some(ranges) => ranges,
                            None => match ty.width.and_then(|width| width.checked_sub(1)) {
                                Some(left) => vec![PackedRange {
                                    left: i128::from(left),
                                    right: 0,
                                }],
                                None => Vec::new(),
                            },
                        };
                    aggregate_members.push(AggregateMember {
                        name,
                        ty,
                        two_state,
                        packed_ranges,
                        aggregate,
                    });
                }
                if aggregate_members.is_empty() {
                    return None;
                }
                let mut packed_members = if is_packed {
                    aggregate_members
                        .iter()
                        .map(|member| {
                            Some(PackedMember {
                                name: member.name.clone(),
                                lsb: 0,
                                width: member.ty.width?,
                                signed: member.ty.signed,
                                two_state: member.two_state,
                                packed_ranges: member.packed_ranges.clone(),
                            })
                        })
                        .collect::<Option<Vec<_>>>()
                } else {
                    None
                };
                if is_packed && !is_union {
                    let mut lsb = 0u32;
                    for member in packed_members.as_mut()?.iter_mut().rev() {
                        member.lsb = lsb;
                        lsb = lsb.checked_add(member.width)?;
                    }
                }
                return Some((
                    AggregateLayout {
                        kind,
                        type_identity: {
                            let full_name = vpi::obj_full_name(cur);
                            if !full_name.is_empty() {
                                Some(full_name)
                            } else {
                                let name = vpi::obj_name(cur);
                                (!name.is_empty()).then_some(name)
                            }
                        },
                        members: aggregate_members,
                    },
                    packed_members,
                ));
            }
            if hops == 16 {
                return None;
            }
            current = match current.as_ref() {
                Some(owner) => owner.child(vpi::vpiActual),
                None => child(vpi::vpiActual, typespec),
            };
            current.as_ref()?;
            hops += 1;
        }
    }

    fn contextual_packed_ranges(
        &mut self,
        typespec: VpiHandle,
        context: VpiHandle,
    ) -> Option<Vec<PackedRange>> {
        let mut current: Option<OwnedHandle> = None;
        let mut hops = 0usize;
        loop {
            let cur = current.as_ref().map_or(typespec, OwnedHandle::raw);
            if vpi::obj_type(cur) != vpi::vpiRefTypespec {
                if !matches!(
                    vpi::obj_type(cur),
                    vpi::vpiLogicTypespec | vpi::vpiBitTypespec | vpi::vpiPackedArrayTypespec
                ) {
                    return None;
                }
                let mut ranges = Vec::new();
                for range in iter(vpi::vpiRange, cur) {
                    let left = child(vpi::vpiLeftRange, range.raw())?;
                    let right = child(vpi::vpiRightRange, range.raw())?;
                    ranges.push(PackedRange {
                        left: self.contextual_bound_value(left.raw(), context)?,
                        right: self.contextual_bound_value(right.raw(), context)?,
                    });
                }
                if vpi::obj_type(cur) == vpi::vpiPackedArrayTypespec {
                    if let Some(element) = child(vpi::vpiElemTypespec, cur) {
                        if let Some(mut inner) =
                            self.contextual_packed_ranges(element.raw(), context)
                        {
                            ranges.append(&mut inner);
                        }
                    }
                }
                return (!ranges.is_empty()).then_some(ranges);
            }
            if hops == 16 {
                return None;
            }
            current = match current.as_ref() {
                Some(owner) => owner.child(vpi::vpiActual),
                None => child(vpi::vpiActual, typespec),
            };
            current.as_ref()?;
            hops += 1;
        }
    }

    fn contextual_range_width(&mut self, typespec: VpiHandle, context: VpiHandle) -> Option<u32> {
        let mut total = 1u128;
        let mut any = false;
        for range in iter(vpi::vpiRange, typespec) {
            any = true;
            let left = child(vpi::vpiLeftRange, range.raw())?;
            let right = child(vpi::vpiRightRange, range.raw())?;
            let left = self.contextual_bound_value(left.raw(), context)?;
            let right = self.contextual_bound_value(right.raw(), context)?;
            let width = left.abs_diff(right).checked_add(1)?;
            total = total.checked_mul(width)?;
        }
        if !any {
            return Some(1);
        }
        u32::try_from(total).ok()
    }

    fn contextual_bound_value(&mut self, bound: VpiHandle, context: VpiHandle) -> Option<i128> {
        match vpi::read_value(bound) {
            ValueData::Int(value) => return Some(value as i128),
            ValueData::UInt(value) => return Some(value as i128),
            ValueData::Scalar(value) => return Some(value as i128),
            _ => {}
        }
        self.eval_in_parent_scope(context, bound, 0)
            .and_then(|value| match value {
                Val::Bits(bits) if !bits.is_unknown() => {
                    if bits.signed {
                        bits.to_i128()
                    } else {
                        bits.to_u128().and_then(|value| i128::try_from(value).ok())
                    }
                }
                _ => None,
            })
    }

    fn eval_in_parent_scope(
        &mut self,
        object: VpiHandle,
        expression: VpiHandle,
        hops: usize,
    ) -> Option<Val> {
        if hops > 64 {
            return None;
        }
        if matches!(
            vpi::obj_type(object),
            vpi::vpiModule | vpi::vpiGenScope | vpi::vpiPackage
        ) {
            return self.resolver.eval_expr(object, expression).ok();
        }
        let parent = child(vpi::vpiParent, object)?;
        self.eval_in_parent_scope(parent.raw(), expression, hops + 1)
    }

    pub(in crate::core::db) fn concrete_typespec(&mut self, ts: VpiHandle) -> TypeInfo {
        let t = vpi::obj_type(ts);
        let mut signed = vpi::get(vpi::vpiSigned, ts) != 0;
        if matches!(t, vpi::vpiStructTypespec | vpi::vpiUnionTypespec) && !signed {
            if let Some(source) = self.declaration_source_prefix(ts) {
                let words = source
                    .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                    .filter(|word| !word.is_empty())
                    .collect::<Vec<_>>();
                signed = words
                    .windows(3)
                    .any(|words| matches!(words, ["struct" | "union", "packed", "signed"]));
            }
        }
        let type_name = {
            let n = vpi::obj_name(ts);
            if n.is_empty() {
                None
            } else {
                Some(n)
            }
        };
        match t {
            vpi::vpiIntTypespec => TypeInfo {
                kind: "int".to_string(),
                width: Some(32),
                signed,
                type_name: None,
            },
            vpi::vpiIntegerTypespec => TypeInfo {
                kind: "integer".to_string(),
                width: Some(32),
                signed,
                type_name: None,
            },
            vpi::vpiTimeTypespec => TypeInfo {
                kind: "time".to_string(),
                // LRM 1364-1995 §3.10.2 / 1364-2001 §3.11.2: `time` is at
                // least 64 bits; `$time` produces 64-bit values.
                width: Some(64),
                signed,
                type_name: None,
            },
            vpi::vpiLongIntTypespec => TypeInfo {
                kind: "longint".to_string(),
                width: Some(64),
                signed,
                type_name: None,
            },
            vpi::vpiByteTypespec => TypeInfo {
                kind: "byte".to_string(),
                width: Some(8),
                signed: true,
                type_name: None,
            },
            vpi::vpiShortIntTypespec => TypeInfo {
                kind: "shortint".to_string(),
                width: Some(16),
                signed: true,
                type_name: None,
            },
            vpi::vpiLogicTypespec => TypeInfo {
                kind: "logic".to_string(),
                width: self.range_width(ts),
                signed,
                type_name: None,
            },
            vpi::vpiBitTypespec => TypeInfo {
                kind: "bit".to_string(),
                width: self.range_width(ts),
                signed,
                type_name: None,
            },
            vpi::vpiEnumTypespec => {
                let base = child(vpi::vpiBaseTypespec, ts)
                    .map(|base| self.typespec_info(base.raw()))
                    .unwrap_or_default();
                TypeInfo {
                    kind: "enum".to_string(),
                    width: base.width,
                    signed: base.signed,
                    type_name,
                }
            }
            vpi::vpiStructTypespec => TypeInfo {
                kind: "struct".to_string(),
                width: None,
                signed,
                type_name,
            },
            vpi::vpiUnionTypespec => TypeInfo {
                kind: "union".to_string(),
                width: None,
                signed,
                type_name,
            },
            vpi::vpiStringTypespec => TypeInfo {
                kind: "string".to_string(),
                width: None,
                signed: false,
                type_name: None,
            },
            vpi::vpiRealTypespec => TypeInfo {
                kind: "real".to_string(),
                width: None,
                signed: true,
                type_name: None,
            },
            vpi::vpiShortRealTypespec => TypeInfo {
                kind: "shortreal".to_string(),
                width: None,
                signed: true,
                type_name: None,
            },
            vpi::vpiChandleTypespec => TypeInfo {
                kind: "chandle".to_string(),
                width: None,
                signed: false,
                type_name: None,
            },
            vpi::vpiClassTypespec => TypeInfo {
                kind: "class".to_string(),
                width: None,
                signed: false,
                type_name,
            },
            vpi::vpiArrayTypespec => {
                let element_kind =
                    child(vpi::vpiElemTypespec, ts).map(|elem| self.typespec_info(elem.raw()).kind);
                let kind = match element_kind.as_deref() {
                    Some("real") => "real_array",
                    Some("shortreal") => "shortreal_array",
                    _ => "array",
                };
                TypeInfo {
                    kind: kind.to_string(),
                    width: None,
                    signed: false,
                    type_name: None,
                }
            }
            vpi::vpiPackedArrayTypespec => TypeInfo {
                kind: "array".to_string(),
                width: None,
                signed: false,
                type_name: None,
            },
            _ => TypeInfo::default(),
        }
    }

    pub(in crate::core::db) fn range_width(&self, ts: VpiHandle) -> Option<u32> {
        let mut total: u64 = 1;
        let mut any = false;
        for r in iter(vpi::vpiRange, ts) {
            any = true;
            let l = self.range_bound(vpi::vpiLeftRange, r.raw())?;
            let rr = self.range_bound(vpi::vpiRightRange, r.raw())?;
            let dim = (l - rr).abs() + 1;
            total = total.saturating_mul(dim as u64);
        }
        if any {
            Some(total as u32)
        } else {
            Some(1)
        }
    }

    pub(in crate::core::db) fn range_bound(&self, rel: c_int, r: VpiHandle) -> Option<i128> {
        let b = child(rel, r)?;
        match vpi::read_value(b.raw()) {
            ValueData::Int(v) => Some(v as i128),
            ValueData::UInt(v) => Some(v as i128),
            ValueData::Scalar(v) => Some(v as i128),
            _ => None,
        }
    }
}
