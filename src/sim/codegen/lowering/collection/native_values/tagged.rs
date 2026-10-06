//! Tagged unions with string, real or handle members (SIM-007).
//!
//! Such a union has no packed payload, so module and static storage keep a
//! four-state tag signal and every member's own leaves
//! (`collect_native_tagged_union`). Every member access checks the tag
//! first, exactly like a packed tagged union: an inactive read reports a
//! source-addressed runtime error and yields the member's default, an
//! inactive write reports the error and stores nothing (SV 7.3.2, 11.9).
//! Matching reads the tag and the selected member's leaves in place.
//! Subroutine storage holds the same tag and member leaves in one native
//! value (`native_tagged_element`), checked the same way. Elements of
//! queues, dynamic, associative and fixed arrays are such values inside
//! their container; their tag and members are read in place through
//! element accesses, which re-evaluate the element's locator, so a checked
//! element access needs a locator free of side effects.

use super::*;
use crate::sim::ir::{IrPackedSelect, IrTaggedMemberGuard, IrTaggedSelectStep};

/// The storage of a tagged union with native members: the leaves of a
/// module or static declaration, a native subroutine value, or the
/// container element that an element select (`q[i]`, `a["k"]`) names.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::sim::codegen) enum NativeTaggedRoot {
    Module(NodeId),
    Value(usize),
    Element(NodeId),
}

/// One member access below a native tagged union variable.
#[derive(Clone)]
pub(in crate::sim::codegen) struct NativeTaggedMember {
    pub(in crate::sim::codegen) root: NativeTaggedRoot,
    pub(in crate::sim::codegen) index: u32,
    pub(in crate::sim::codegen) name: String,
    pub(in crate::sim::codegen) tag_bits: u32,
}

impl Codegen<'_> {
    /// The union variable `node` names directly, when it is a tagged union
    /// with native members.
    pub(in crate::sim::codegen) fn native_tagged_root(
        &self,
        node: NodeId,
    ) -> Option<NativeTaggedRoot> {
        let node = self.p30_unwrap_cast(node);
        if let Ok(Some((value, path))) = self.native_path_of(node) {
            return (path.is_empty()
                && self
                    .native_tagged_layout(NativeTaggedRoot::Value(value))
                    .is_some())
            .then_some(NativeTaggedRoot::Value(value));
        }
        if self.record_element_of(node).is_some() {
            let root = NativeTaggedRoot::Element(node);
            return self.native_tagged_layout(root).is_some().then_some(root);
        }
        let (root, info) = self.unpacked_aggregate_info(node)?;
        (info.kind == AggregateKind::TaggedUnion && !info.columns)
            .then_some(NativeTaggedRoot::Module(root))
    }

    pub(in crate::sim::codegen) fn native_tagged_layout(
        &self,
        root: NativeTaggedRoot,
    ) -> Option<AggregateLayout> {
        let element;
        let descriptor = match root {
            NativeTaggedRoot::Module(root) => self.query_descriptor(root)?,
            NativeTaggedRoot::Value(value) => &self.native_layout_of_value(value).ok()?.descriptor,
            NativeTaggedRoot::Element(node) => {
                let selection = self.record_element_of(node)?;
                element =
                    self.container_element_descriptor(selection.container, selection.depth())?;
                &element
            }
        };
        match &descriptor.shape {
            TypeShape::Aggregate(layout) if layout.kind == AggregateKind::TaggedUnion => {
                Some(layout.clone())
            }
            _ => None,
        }
    }

    /// The member leaf at `path` of a tagged union's storage.
    fn native_tagged_leaf(
        &self,
        root: NativeTaggedRoot,
        path: &[AggregatePathPart],
    ) -> Option<NativeEndpointLeaf> {
        match root {
            NativeTaggedRoot::Module(root) => self
                .unpacked_aggregates
                .get(&root)?
                .leaves
                .iter()
                .find(|leaf| leaf.path == path)
                .map(|leaf| NativeEndpointLeaf::Module(Box::new(leaf.clone()))),
            NativeTaggedRoot::Value(value) => self
                .native_layout_of_value(value)
                .ok()?
                .leaves
                .iter()
                .find(|leaf| leaf.path == path)
                .map(|leaf| NativeEndpointLeaf::Value(value, leaf.clone())),
            // Element leaves are read through fresh accesses
            // (`native_tagged_leaf_read`), never as endpoints.
            NativeTaggedRoot::Element(_) => None,
        }
    }

    /// A read of the member leaf at `leaf_path` of a tagged union's storage.
    fn native_tagged_leaf_read(
        &mut self,
        path: &str,
        root: NativeTaggedRoot,
        leaf_path: &[AggregatePathPart],
    ) -> Result<Option<LeafValue>, String> {
        if let NativeTaggedRoot::Element(element) = root {
            if !self.side_effect_free(element) {
                return Err(format!(
                    "a tagged union element accessed in `{path}` must be selected without side effects"
                ));
            }
            return self.element_path_read(path, element, leaf_path);
        }
        self.native_tagged_leaf(root, leaf_path)
            .map(|leaf| self.endpoint_leaf_read(&leaf))
            .transpose()
    }

    /// The storage of the members below `prefix` as a transfer endpoint.
    fn native_tagged_endpoint(
        &self,
        root: NativeTaggedRoot,
        prefix: Vec<AggregatePathPart>,
        descriptor: &TypeDescriptor,
    ) -> Option<NativeEndpoint> {
        Some(match root {
            NativeTaggedRoot::Module(root) => {
                NativeEndpoint::Module(Box::new(AggregateSelection {
                    root,
                    prefix,
                    descriptor: descriptor.clone(),
                    storage: self.unpacked_aggregates.get(&root)?.clone(),
                }))
            }
            NativeTaggedRoot::Value(value) => NativeEndpoint::Value { value, prefix },
            NativeTaggedRoot::Element(_) => return None,
        })
    }

    /// The union member that a member selection below a native tagged union
    /// passes through (`u.m`, `u.m.s`, `u.m[2]`).
    pub(in crate::sim::codegen) fn native_tagged_member(
        &self,
        node: NodeId,
    ) -> Option<NativeTaggedMember> {
        let (root, path) = match self.native_path_of(node) {
            Ok(Some((value, path))) => (NativeTaggedRoot::Value(value), path),
            Ok(None) => {
                if let Some((element, _, path)) = self.element_member_parts(node) {
                    (NativeTaggedRoot::Element(element), path)
                } else {
                    let (root, path) = self.unpacked_path_for_expr(node)?;
                    let info = self.unpacked_aggregates.get(&root)?;
                    if info.kind != AggregateKind::TaggedUnion || info.columns {
                        return None;
                    }
                    (NativeTaggedRoot::Module(root), path)
                }
            }
            Err(_) => return None,
        };
        let AggregatePathPart::Member(name) = path.first()? else {
            return None;
        };
        let layout = self.native_tagged_layout(root)?;
        let index = layout
            .members
            .iter()
            .position(|member| member.name == *name)?;
        Some(NativeTaggedMember {
            root,
            index: u32::try_from(index).ok()?,
            name: name.clone(),
            tag_bits: layout.tag_bits()?,
        })
    }

    /// The tagged member that `node` reads or writes: a member selection or
    /// a bit, part or element select of one. Nodes whose checked lowering is
    /// in progress are excluded.
    pub(in crate::sim::codegen) fn native_tagged_access(
        &self,
        node: NodeId,
    ) -> Option<NativeTaggedMember> {
        if self.native_tagged_bypass.contains(&node) {
            return None;
        }
        if let Some(member) = self.native_tagged_member(node) {
            return Some(member);
        }
        match self.kind(node) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.native_tagged_access(*base),
            _ => None,
        }
    }

    /// Lower `node` with the ordinary lowering `lower`, then check its tag.
    pub(in crate::sim::codegen) fn checked_native_tagged<T>(
        &mut self,
        node: NodeId,
        lower: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        self.native_tagged_bypass.insert(node);
        let lowered = lower(self);
        self.native_tagged_bypass.remove(&node);
        lowered
    }

    /// Checked packed or real read of a native tagged member (or a select
    /// inside one).
    pub(in crate::sim::codegen) fn native_tagged_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some(member) = self.native_tagged_access(node) else {
            return Ok(None);
        };
        let value = self.checked_native_tagged(node, |cg| cg.lower_expr(path, node))?;
        Ok(Some(if value.is_real() {
            IrExpr::new(
                IrExprKind::Mux {
                    sel: Box::new(self.native_tagged_check(path, &member, node)?),
                    a: Box::new(value),
                    b: Box::new(real_literal_expr(0.0)),
                },
                0,
                false,
                None,
            )
        } else {
            self.native_tagged_packed_read(path, &member, value, node)?
        }))
    }

    /// Checked string read of a native tagged member: the empty string
    /// after an inactive-member error.
    pub(in crate::sim::codegen) fn native_tagged_string(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStringExpr>, String> {
        let Some(member) = self.native_tagged_access(node) else {
            return Ok(None);
        };
        let value = self.checked_native_tagged(node, |cg| cg.lower_string(path, node))?;
        Ok(Some(IrStringExpr::Conditional {
            predicate: Box::new(self.native_tagged_check(path, &member, node)?),
            then: Box::new(value),
            otherwise: Box::new(IrStringExpr::Literal(Vec::new())),
        }))
    }

    /// Checked handle read of a native tagged member: null after an
    /// inactive-member error.
    pub(in crate::sim::codegen) fn native_tagged_chandle(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        let Some(member) = self.native_tagged_access(node) else {
            return Ok(None);
        };
        let value = self.checked_native_tagged(node, |cg| cg.lower_chandle(path, node))?;
        Ok(Some(IrChandleExpr::Conditional {
            predicate: Box::new(self.native_tagged_check(path, &member, node)?),
            then: Box::new(value),
            otherwise: Box::new(IrChandleExpr::Null),
        }))
    }

    fn native_tag_leaf(&self, root: NativeTaggedRoot) -> Option<NativeEndpointLeaf> {
        self.native_tagged_leaf(
            root,
            &[AggregatePathPart::Member(NATIVE_TAG_MEMBER.to_owned())],
        )
    }

    /// The tag of a native tagged union; `None` for a one-member union,
    /// whose only member is always active.
    pub(in crate::sim::codegen) fn native_tag_read(
        &mut self,
        path: &str,
        root: NativeTaggedRoot,
    ) -> Result<Option<IrExpr>, String> {
        match self.native_tagged_leaf_read(
            path,
            root,
            &[AggregatePathPart::Member(NATIVE_TAG_MEMBER.to_owned())],
        )? {
            None => Ok(None),
            Some(LeafValue::Packed(tag)) => Ok(Some(tag)),
            Some(_) => Err("tagged union tag is not a packed leaf".to_owned()),
        }
    }

    fn native_tag_guard(member: &NativeTaggedMember) -> IrTaggedMemberGuard {
        IrTaggedMemberGuard {
            member_index: member.index,
            tag_width: member.tag_bits,
            member_name: member.name.clone(),
        }
    }

    /// A checked packed read of `value` (a member leaf of `member`): the
    /// value while the member is active, otherwise X and a runtime error.
    pub(in crate::sim::codegen) fn native_tagged_packed_read(
        &mut self,
        path: &str,
        member: &NativeTaggedMember,
        value: IrExpr,
        site: NodeId,
    ) -> Result<IrExpr, String> {
        let Some(tag) = self.native_tag_read(path, member.root)? else {
            return Ok(value);
        };
        let (width, signed) = (value.width, value.signed);
        Ok(IrExpr::new(
            IrExprKind::TaggedSelect {
                base: Box::new(IrExpr::new(
                    IrExprKind::Concat {
                        parts: vec![tag, value],
                    },
                    member.tag_bits + width,
                    false,
                    None,
                )),
                steps: vec![IrTaggedSelectStep {
                    selection: IrPackedSelect {
                        base: lhs_integer_expr(0),
                        width,
                    },
                    two_state: false,
                    guard: Some(Self::native_tag_guard(member)),
                }],
                location: self.source_location(site),
            },
            width,
            signed,
            None,
        ))
    }

    /// One-bit known test that `member` is active. A false test has
    /// already reported the inactive-member runtime error.
    pub(in crate::sim::codegen) fn native_tagged_check(
        &mut self,
        path: &str,
        member: &NativeTaggedMember,
        site: NodeId,
    ) -> Result<IrExpr, String> {
        let one = const_bits_expr(1, true);
        let checked = self.native_tagged_packed_read(path, member, one.clone(), site)?;
        Ok(cmp_expr_ir(IrBinOp::CaseEq, checked, one))
    }

    /// `u = tagged m value` (or `tagged m` for a void member) into a native
    /// tagged union variable: the tag names `m` and `m` receives the value;
    /// the other members' storage is unobservable until retagged.
    pub(in crate::sim::codegen) fn lower_native_tagged_construct(
        &mut self,
        path: &str,
        root: NativeTaggedRoot,
        rhs: NodeId,
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        let NodeKind::Expr(ExprKind::TaggedUnion { member, value }) =
            self.kind(self.p30_unwrap_cast(rhs))
        else {
            return Ok(None);
        };
        let (member_name, value) = (member.clone(), *value);
        if nba {
            return Err(format!(
                "nonblocking assignment to a tagged union with string, real or handle members in `{path}` is not supported"
            ));
        }
        let layout = self
            .native_tagged_layout(root)
            .ok_or_else(|| format!("tagged union in `{path}` has no layout"))?;
        let index = layout
            .members
            .iter()
            .position(|member| member.name == member_name)
            .ok_or_else(|| format!("tagged union has no member `{member_name}` in `{path}`"))?;
        let member = layout.members[index].clone();
        let mut statements = Vec::new();
        // The value is evaluated before the tag changes, so a source that
        // reads the union observes its previous member.
        if let Some(value) = value {
            let prefix = vec![AggregatePathPart::Member(member_name.clone())];
            let target = self
                .native_tagged_endpoint(root, prefix.clone(), &member.descriptor)
                .ok_or_else(|| format!("tagged union in `{path}` has no storage"))?;
            let leaves = self.endpoint_leaves(&target)?;
            match leaves.as_slice() {
                [(
                    relative,
                    leaf @ (NativeEndpointLeaf::Module(_) | NativeEndpointLeaf::Value(..)),
                )] if relative.is_empty() => {
                    let leaf = leaf.clone();
                    let ty = leaf_field_type(&member.descriptor).ok_or_else(|| {
                        format!("tagged member `{member_name}` in `{path}` has no scalar type")
                    })?;
                    let source = self.native_leaf_source(path, ty, value)?;
                    statements.push(self.endpoint_leaf_write(path, &leaf, source, false)?);
                }
                _ => {
                    // Any record source (pattern, call, variable) is built in
                    // a temporary of the member type, then copied leaf by leaf.
                    let temporary = self.native_temporary(value)?;
                    let source = NativeEndpoint::Value {
                        value: temporary,
                        prefix: Vec::new(),
                    };
                    statements.push(IrStmt::NativeValueDeclare(temporary));
                    statements.push(self.native_assign_into(
                        path,
                        &source,
                        &member.descriptor,
                        value,
                        false,
                    )?);
                    statements.push(self.native_transfer(path, &target, &source, false)?);
                }
            }
        }
        // Inactive members return to their default-uninitialized values, as
        // for column-layout unions, so whole-value copies and equality never
        // observe a previous member's storage.
        for (other, member) in layout.members.iter().enumerate() {
            if other == index || is_void_member(member) {
                continue;
            }
            let target = self
                .native_tagged_endpoint(
                    root,
                    vec![AggregatePathPart::Member(member.name.clone())],
                    &member.descriptor,
                )
                .ok_or_else(|| format!("tagged union in `{path}` has no storage"))?;
            for (_, leaf) in self.endpoint_leaves(&target)? {
                match leaf {
                    NativeEndpointLeaf::Container(container) => {
                        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Delete(
                            container,
                        ))));
                        if matches!(
                            self.model.containers[container].kind,
                            IrContainerKind::Associative { .. }
                        ) {
                            statements.push(IrStmt::Container(Box::new(
                                IrContainerStmt::ResetDefault(container),
                            )));
                        }
                    }
                    leaf => statements.push(self.native_leaf_reset(path, &leaf, false)?),
                }
            }
        }
        if let Some(tag) = self.native_tag_leaf(root) {
            let tag_bits = layout
                .tag_bits()
                .ok_or_else(|| format!("tagged union in `{path}` has no tag"))?;
            let lhs = match &tag {
                NativeEndpointLeaf::Module(leaf) => self.aggregate_leaf_lhs(leaf)?,
                NativeEndpointLeaf::Value(value, leaf) => self.native_leaf_lhs(*value, leaf),
                NativeEndpointLeaf::Container(_) => {
                    return Err("tagged union tag is not a packed leaf".to_owned())
                }
            };
            let expected = IrConst::packed(
                vec![u64::try_from(index).map_err(|_| "tagged member index overflows")?],
                vec![0],
                vec![0],
                tag_bits,
                false,
                None,
            )
            .map_err(|error| error.to_string())?;
            statements.push(IrStmt::Assign {
                lhs,
                rhs: IrExpr::new(IrExprKind::Const(expected), tag_bits, false, None),
                nba: false,
            });
        }
        Ok(Some(IrStmt::Block(statements)))
    }

    /// Guard a write below a native tagged union member: the write happens
    /// only while the member is active, otherwise the runtime error is
    /// reported and nothing is stored.
    pub(in crate::sim::codegen) fn native_tagged_guarded_write(
        &mut self,
        path: &str,
        lhs: NodeId,
        write: IrStmt,
    ) -> Result<IrStmt, String> {
        let Some(member) = self.native_tagged_access(lhs) else {
            return Ok(write);
        };
        Ok(IrStmt::If {
            cond: self.native_tagged_check(path, &member, lhs)?,
            then_: vec![write],
            els: None,
            check: IrUniquePriorityCheck::None,
        })
    }

    /// Pattern match against a native tagged union variable `source`:
    /// `tagged m`, `tagged m .*`, `tagged m .v` and packed constant or
    /// structure payloads of packed members (SV 12.6).
    pub(in crate::sim::codegen) fn native_tagged_pattern(
        &mut self,
        path: &str,
        root: NativeTaggedRoot,
        member_name: &str,
        payload: Option<NodeId>,
    ) -> Result<NativeTaggedPattern, String> {
        let layout = self
            .native_tagged_layout(root)
            .ok_or_else(|| format!("tagged union in `{path}` has no layout"))?;
        let index = layout
            .members
            .iter()
            .position(|member| member.name == member_name)
            .ok_or_else(|| format!("tagged union has no member `{member_name}` in `{path}`"))?;
        let member = NativeTaggedMember {
            root,
            index: u32::try_from(index).map_err(|_| "tagged member index overflows")?,
            name: member_name.to_owned(),
            tag_bits: layout
                .tag_bits()
                .ok_or_else(|| format!("tagged union in `{path}` has no tag"))?,
        };
        let descriptor = layout.members[index].descriptor.clone();
        let value = self.native_tagged_leaf_read(
            path,
            root,
            &[AggregatePathPart::Member(member_name.to_owned())],
        )?;
        Ok(NativeTaggedPattern {
            tag: self.native_tag_read(path, root)?,
            tag_bits: member.tag_bits,
            index: member.index,
            descriptor,
            value: value.map(LeafValue::into_leaf_expr).transpose()?,
            payload,
        })
    }
}

/// Whether `path` names the tag leaf of a native tagged union.
pub(in crate::sim::codegen) fn is_native_tag_path(path: &[AggregatePathPart]) -> bool {
    matches!(path, [AggregatePathPart::Member(name)] if name == NATIVE_TAG_MEMBER)
}

/// Equality of two tagged unions of one type from the equalities of their
/// paired leaves (`$tag` first, then each member's leaves): the tags match
/// and the active member's leaves match. Storage of inactive members is
/// unobservable (SV 7.3.2), so a member's leaves only count while its tag
/// is set; `===` compares the tag exactly.
pub(in crate::sim::codegen) fn native_tagged_equality(
    layout: &AggregateLayout,
    case: bool,
    leaves: Vec<(Vec<AggregatePathPart>, IrExpr)>,
    tags: Option<(IrExpr, IrExpr)>,
) -> Result<IrExpr, String> {
    let and = |previous: Option<IrExpr>, next: IrExpr| {
        Some(match previous {
            Some(previous) => cmp_expr_ir(IrBinOp::LogAnd, previous, next),
            None => next,
        })
    };
    let Some((left_tag, right_tag)) = tags else {
        // A one-member union has no tag: its member is always active.
        return leaves
            .into_iter()
            .fold(None, |equality, (_, leaf)| and(equality, leaf))
            .ok_or_else(|| "tagged union equality has no leaves".to_owned());
    };
    let tag_bits = layout.tag_bits().ok_or("tagged union has no tag")?;
    let (equal, differ) = if case {
        (IrBinOp::CaseEq, IrBinOp::CaseNeq)
    } else {
        (IrBinOp::Eq, IrBinOp::Neq)
    };
    let mut equality = Some(cmp_expr_ir(equal, left_tag.clone(), right_tag));
    for (index, member) in layout.members.iter().enumerate() {
        let member_equal = leaves
            .iter()
            .filter(|(path, _)| {
                matches!(path.first(), Some(AggregatePathPart::Member(name)) if *name == member.name)
            })
            .fold(None, |equality, (_, leaf)| and(equality, leaf.clone()));
        let Some(member_equal) = member_equal else {
            continue;
        };
        let index = IrConst::packed(
            vec![u64::try_from(index).map_err(|_| "tagged member index overflows")?],
            vec![0],
            vec![0],
            tag_bits,
            false,
            None,
        )
        .map_err(|error| error.to_string())?;
        let inactive = cmp_expr_ir(
            differ,
            left_tag.clone(),
            IrExpr::new(IrExprKind::Const(index), tag_bits, false, None),
        );
        equality = and(
            equality,
            cmp_expr_ir(IrBinOp::LogOr, inactive, member_equal),
        );
    }
    equality.ok_or_else(|| "tagged union equality has no leaves".to_owned())
}

/// The scalar leaf type of a one-leaf union member.
fn leaf_field_type(descriptor: &TypeDescriptor) -> Option<IrClassFieldType> {
    match &descriptor.shape {
        TypeShape::String => Some(IrClassFieldType::String),
        TypeShape::Opaque { .. } => Some(IrClassFieldType::Chandle),
        TypeShape::Real { shortreal } => Some(IrClassFieldType::Real {
            shortreal: *shortreal,
        }),
        _ => Some(IrClassFieldType::Packed {
            width: Codegen::fixed_descriptor_width(descriptor)?,
            signed: descriptor.info.signed,
            two_state: descriptor.two_state,
        }),
    }
}

/// The parts of a pattern test against a native tagged union member.
pub(in crate::sim::codegen) struct NativeTaggedPattern {
    pub(in crate::sim::codegen) tag: Option<IrExpr>,
    pub(in crate::sim::codegen) tag_bits: u32,
    pub(in crate::sim::codegen) index: u32,
    pub(in crate::sim::codegen) descriptor: TypeDescriptor,
    /// The member's value when it is one leaf (packed, real, string or
    /// handle); `None` for void and record members.
    pub(in crate::sim::codegen) value: Option<IrNativeLeafExpr>,
    pub(in crate::sim::codegen) payload: Option<NodeId>,
}
