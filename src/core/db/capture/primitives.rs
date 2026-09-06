use crate::ffi::vpi::{self, VpiHandle};

use super::super::capture;
use super::super::database::*;
use super::super::{Direction, PrimitiveType, Strength};

pub(in crate::core::db) fn primitive_type(handle: VpiHandle) -> PrimitiveType {
    PrimitiveType::from_raw(vpi::get(vpi::vpiPrimType, handle))
}

pub(in crate::core::db) fn strengths(handle: VpiHandle) -> (Strength, Strength) {
    (
        Strength::from_raw(vpi::get(vpi::vpiStrength0, handle)),
        Strength::from_raw(vpi::get(vpi::vpiStrength1, handle)),
    )
}

pub(in crate::core::db) fn terminal_direction(handle: VpiHandle) -> Direction {
    Direction::from_raw(vpi::get(vpi::vpiDirection, handle))
}

impl Builder {
    pub(in crate::core::db) fn walk_cont_assign(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let net_decl = vpi::get(vpi::vpiNetDeclAssign, h) != 0;
        let (strength0, strength1) = capture::primitives::strengths(h);
        let id = self.register(
            parent,
            &props,
            NodeKind::ContAssign {
                net_decl,
                delay: None,
                strength0,
                strength1,
            },
        );
        let mut kids: Vec<NodeId> = Vec::new();
        if let Some(lhs) = child(vpi::vpiLhs, h) {
            // A net-declaration assignment's LHS is the declaration object
            // itself in Surelog v1.87, not necessarily a ref wrapper. Bind it
            // to the declaration already captured in the owning scope so the
            // owned DB preserves whether this targets a net, variable, or
            // unpacked array. The array full-name fallback also covers older
            // frontend shapes whose direct object identity differs.
            let full = vpi::obj_full_name(lhs.raw());
            let declaration = if net_decl {
                if is_array_type(vpi::obj_type(lhs.raw())) && !full.is_empty() {
                    self.array_by_fullname(&full)
                } else {
                    self.resolve_direct(lhs.raw())
                        .or_else(|| self.resolve_ref(lhs.raw()))
                }
            } else {
                None
            };
            let lhs_id = if let Some(target) = declaration {
                self.register(
                    Some(id),
                    &CommonProps::default(),
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }),
                )
            } else {
                self.walk_node(lhs.raw(), Some(id))?
            };
            kids.push(lhs_id);
        }
        if let Some(rhs) = child(vpi::vpiRhs, h) {
            kids.push(self.walk_node(rhs.raw(), Some(id))?);
        }
        // `assign #d lhs = rhs;` — the delay is a 1-to-1 `vpiDelay` child
        // expression (a plain constant or a parameter reference); walked as
        // the third child and recorded on the variant so codegen can fold it.
        let delay = match child(vpi::vpiDelay, h) {
            Some(d) => {
                let did = self.walk_node(d.raw(), Some(id))?;
                kids.push(did);
                Some(did)
            }
            None => None,
        };
        self.set_children(id, kids);
        self.set_kind(
            id,
            NodeKind::ContAssign {
                net_decl,
                delay,
                strength0,
                strength1,
            },
        );
        Ok(id)
    }

    pub(in crate::core::db) fn walk_primitive(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let class = match vpi::obj_type(h) {
            vpi::vpiSwitch => PrimClass::Switch,
            vpi::vpiUdp => PrimClass::Udp,
            _ => PrimClass::Gate,
        };
        let prim_type = capture::primitives::primitive_type(h);
        let (strength0, strength1) = capture::primitives::strengths(h);
        let id = self.register(
            parent,
            &props,
            NodeKind::Gate {
                class,
                prim_type,
                strength0,
                strength1,
                delay: None,
                terms: Vec::new(),
            },
        );
        let mut terms: Vec<GateTerm> = Vec::new();
        for t in iter(vpi::vpiPrimTerm, h) {
            let t = t.raw();
            let direction = capture::primitives::terminal_direction(t);
            let term_index = vpi::get(vpi::vpiTermIndex, t);
            let expr = child(vpi::vpiExpr, t).ok_or_else(|| {
                format!(
                    "primitive `{}` has a terminal without a connection",
                    props.name
                )
            })?;
            let eid = self.walk_node(expr.raw(), Some(id))?;
            terms.push(GateTerm {
                direction,
                term_index,
                expr: eid,
            });
        }
        terms.sort_by_key(|t| t.term_index);
        // `and #2 g(...)` — a 1-to-1 `vpiDelay` child expression; walked as
        // the node's only child and recorded on the variant so codegen can
        // fold it like a continuous-assignment delay.
        let delay = match child(vpi::vpiDelay, h) {
            Some(d) => {
                let did = self.walk_node(d.raw(), Some(id))?;
                self.set_children(id, vec![did]);
                Some(did)
            }
            None => None,
        };
        self.set_kind(
            id,
            NodeKind::Gate {
                class,
                prim_type,
                strength0,
                strength1,
                delay,
                terms,
            },
        );
        Ok(id)
    }

    pub(in crate::core::db) fn walk_primitive_array(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        Ok(self.register(
            parent,
            &props,
            NodeKind::Gate {
                class: PrimClass::Array,
                prim_type: capture::primitives::primitive_type(h),
                strength0: Strength::Unknown(0),
                strength1: Strength::Unknown(0),
                delay: None,
                terms: Vec::new(),
            },
        ))
    }
}
