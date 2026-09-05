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
        self.index_node(h, &props, id);
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
        self.index_node(h, &props, id);
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
        let ty = match el {
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
        let id = self.register(parent, &props, NodeKind::Array { ty });
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
        self.arrays.insert(id, ArrayMeta { dims, init });
        Ok(id)
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
        let ty = match child(vpi::vpiTypespec, h) {
            Some(ts) => self.typespec_info(ts.raw()),
            None => match child(vpi::vpiTypedef, h) {
                Some(ts) => self.typespec_info(ts.raw()),
                None => TypeInfo::default(),
            },
        };
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
            _ => ty,
        }
    }

    pub(in crate::core::db) fn typespec_info(&mut self, ts: VpiHandle) -> TypeInfo {
        let mut visited: HashSet<(i32, String)> = HashSet::new();
        let mut current: Option<OwnedHandle> = None;
        loop {
            let cur = current.as_ref().map_or(ts, OwnedHandle::raw);
            if vpi::obj_type(cur) != vpi::vpiRefTypespec {
                return self.concrete_typespec(cur);
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

    pub(in crate::core::db) fn concrete_typespec(&mut self, ts: VpiHandle) -> TypeInfo {
        let t = vpi::obj_type(ts);
        let signed = vpi::get(vpi::vpiSigned, ts) != 0;
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
            vpi::vpiEnumTypespec => TypeInfo {
                kind: "enum".to_string(),
                width: child(vpi::vpiBaseTypespec, ts)
                    .and_then(|b| self.typespec_info(b.raw()).width),
                signed,
                type_name,
            },
            vpi::vpiStructTypespec => TypeInfo {
                kind: "struct".to_string(),
                width: None,
                signed: false,
                type_name,
            },
            vpi::vpiUnionTypespec => TypeInfo {
                kind: "union".to_string(),
                width: None,
                signed: false,
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
