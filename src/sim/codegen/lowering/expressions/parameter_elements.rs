//! Reads of parameters of fixed unpacked types, whole or selected.
//!
//! The value is the one the frontend elaborated, whatever expression or
//! override gave it. A selection with constant indices is a constant; a
//! run-time index selects from the packed image of the smallest part of the
//! value that the constant indices before it leave.

use super::*;
use crate::core::db::ElementValue;
use crate::sim::codegen::lowering::collection::native_values::{LeafReads, LeafValue};
use crate::sim::codegen::lowering::collection::{
    element_descriptor, image_path_descriptor, image_width,
};

/// Bits of a real leaf in the image of a parameter part.
const REAL_IMAGE_WIDTH: Option<u64> = Some(64);

enum SelectPart {
    Member(String),
    Index(NodeId),
}

/// The part of a parameter that constant selectors reach.
enum ConstantPart<'e> {
    Value(&'e ElementValue),
    /// A nonexistent element, which reads as the default of its type.
    Nonexistent,
}

impl<'a> Codegen<'a> {
    /// The parameter of a fixed unpacked type that `node` names.
    fn unpacked_parameter(&self, node: NodeId) -> Option<NodeId> {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => (*target)?,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                return self.unpacked_parameter((*refs.last()?)?)
            }
            _ => node,
        };
        (matches!(self.kind(target), NodeKind::Param { .. })
            && self.db.parameter_elements(target).is_some()
            && self.query_descriptor(target).is_some_and(|descriptor| {
                matches!(
                    &descriptor.shape,
                    TypeShape::FixedArray { .. }
                        | TypeShape::Aggregate(crate::core::db::AggregateLayout {
                            kind: AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion,
                            ..
                        })
                )
            }))
        .then_some(target)
    }

    /// The parameter of a fixed unpacked type that `node` reads, and the
    /// selectors it applies.
    fn parameter_select(&self, node: NodeId) -> Option<(NodeId, Vec<SelectPart>)> {
        if let Some(parameter) = self.unpacked_parameter(node) {
            return Some((parameter, Vec::new()));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                // The owned path retains the exact aggregate owner even when
                // Slang represents its member array as a detached node.
                let (parameter, mut parts) = match self.db.array_select_path(node) {
                    Some((root, members)) => {
                        let (parameter, mut parts) = self.parameter_select(root)?;
                        parts.extend(members.iter().cloned().map(SelectPart::Member));
                        (parameter, parts)
                    }
                    None => self.parameter_select(*base)?,
                };
                parts.extend(indices.iter().copied().map(SelectPart::Index));
                Some((parameter, parts))
            }
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let first = refs.iter().position(|target| {
                    target.is_some_and(|t| self.parameter_select(t).is_some())
                })?;
                let (parameter, mut selected) = self.parameter_select(refs[first]?)?;
                selected.extend(parts[first + 1..].iter().cloned().map(SelectPart::Member));
                Some((parameter, selected))
            }
            _ => None,
        }
    }

    /// Follow the leading constant selectors of `parts` through the value
    /// of `parameter`: the part reached, its type and the selectors left.
    fn constant_parameter_part<'p>(
        &self,
        parameter: NodeId,
        mut parts: &'p [SelectPart],
    ) -> Option<(ConstantPart<'_>, TypeDescriptor, &'p [SelectPart])> {
        let mut value = ConstantPart::Value(self.db.parameter_elements(parameter)?);
        let mut descriptor = self.query_descriptor(parameter)?.clone();
        while let Some((part, rest)) = parts.split_first() {
            match (part, &descriptor.shape) {
                (SelectPart::Index(index), TypeShape::FixedArray { dimensions, .. }) => {
                    let Ok(index) = self.eval_bound_i128(*index) else {
                        break;
                    };
                    let position = element_position(index, *dimensions.first()?);
                    value = match (value, position) {
                        (ConstantPart::Value(ElementValue::Elements(elements)), Some(position)) => {
                            ConstantPart::Value(elements.get(position)?)
                        }
                        (ConstantPart::Nonexistent, _) | (_, None) => ConstantPart::Nonexistent,
                        _ => return None,
                    };
                    descriptor = element_descriptor(&descriptor)?;
                }
                (SelectPart::Member(name), TypeShape::Aggregate(layout)) => {
                    let position = layout
                        .members
                        .iter()
                        .position(|member| member.name == *name)?;
                    value = match value {
                        ConstantPart::Value(ElementValue::Elements(members)) => {
                            ConstantPart::Value(members.get(position)?)
                        }
                        ConstantPart::Value(ElementValue::Union { member, value })
                            if *member == position =>
                        {
                            ConstantPart::Value(value)
                        }
                        ConstantPart::Nonexistent => ConstantPart::Nonexistent,
                        // A packed member, or a union member other than the
                        // one that holds the value, is a part of the image.
                        ConstantPart::Value(_) => break,
                    };
                    descriptor = layout.members[position].descriptor.clone();
                }
                // A select of a packed element.
                _ => break,
            }
            parts = rest;
        }
        Some((value, descriptor, parts))
    }

    /// Lower a read of a parameter of a fixed unpacked type, or of a part of
    /// one. `None` when `node` is no such read or its value has no packed or
    /// real form.
    pub(super) fn lower_parameter_elements(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some((parameter, parts)) = self.parameter_select(node) else {
            return Ok(None);
        };
        let Some((value, descriptor, rest)) = self.constant_parameter_part(parameter, &parts)
        else {
            return Ok(None);
        };
        if rest.is_empty() {
            return constant_part_value(&value, &descriptor);
        }
        if let (ConstantPart::Value(value), Some((SelectPart::Index(index), inner))) =
            (&value, rest.split_first())
        {
            // One run-time index: only the parts that its values select.
            if let Some((left, right, leaves, leaf)) =
                self.projected_elements(value, &descriptor, inner)
            {
                let Some(width) = image_width(&leaf, REAL_IMAGE_WIDTH) else {
                    return Ok(None);
                };
                let mut bits = Vec::new();
                for value in leaves {
                    let Some(part) = image(value, &leaf) else {
                        return Ok(None);
                    };
                    bits.extend(part);
                }
                let base = constant_image(&elab::Value::from_bits(bits, false))?;
                let bounds = (i128::from(left), i128::from(right));
                let selected = self.select_element(path, base, *index, bounds, width)?;
                return image_value(selected, &leaf, width, leaf.info.signed).map(Some);
            }
        }
        // Several run-time indices, and selects of packed elements.
        let Some(mut width) = image_width(&descriptor, REAL_IMAGE_WIDTH) else {
            return Ok(None);
        };
        let Some(bits) = part_image(&value, &descriptor) else {
            return Ok(None);
        };
        let mut selected = constant_image(&elab::Value::from_bits(bits, false))?;
        let mut descriptor = descriptor;
        let mut ranges: Option<Vec<(i128, i128)>> = None;
        for part in rest {
            let index = match part {
                SelectPart::Member(name) => {
                    let member = AggregatePathPart::Member(name.clone());
                    let Some((member, offset)) =
                        image_path_descriptor(&descriptor, &[member], REAL_IMAGE_WIDTH)
                    else {
                        return Ok(None);
                    };
                    let Some(member_width) = image_width(&member, REAL_IMAGE_WIDTH) else {
                        return Ok(None);
                    };
                    selected = part_select(selected, offset, member_width);
                    width = member_width;
                    descriptor = member;
                    continue;
                }
                SelectPart::Index(index) => *index,
            };
            let (left, right) = match (&descriptor.shape, &mut ranges) {
                (TypeShape::FixedArray { dimensions, .. }, _) => match dimensions.first() {
                    Some((left, right)) => (i128::from(*left), i128::from(*right)),
                    None => return Ok(None),
                },
                (TypeShape::PackedAtom { ranges: packed }, ranges) => {
                    let ranges = ranges.get_or_insert_with(|| {
                        packed
                            .iter()
                            .map(|range| (range.left, range.right))
                            .collect()
                    });
                    if ranges.is_empty() {
                        return Ok(None);
                    }
                    ranges.remove(0)
                }
                _ => return Ok(None),
            };
            let Some(element_width) = u32::try_from(left.abs_diff(right) + 1)
                .ok()
                .filter(|count| width.is_multiple_of(*count))
                .map(|count| width / count)
            else {
                return Ok(None);
            };
            selected = self.select_element(path, selected, index, (left, right), element_width)?;
            width = element_width;
            if matches!(descriptor.shape, TypeShape::FixedArray { .. }) {
                let Some(element) = element_descriptor(&descriptor) else {
                    return Ok(None);
                };
                descriptor = element;
            }
        }
        let signed = descriptor.info.signed && ranges.is_none();
        image_value(selected, &descriptor, width, signed).map(Some)
    }

    /// What every element of the array `value` holds at the constant
    /// selectors `inner`: the bounds of the array, the parts and their type.
    fn projected_elements<'e>(
        &self,
        value: &'e ElementValue,
        descriptor: &TypeDescriptor,
        inner: &[SelectPart],
    ) -> Option<(i32, i32, Vec<&'e ElementValue>, TypeDescriptor)> {
        let (ElementValue::Elements(elements), TypeShape::FixedArray { dimensions, .. }) =
            (value, &descriptor.shape)
        else {
            return None;
        };
        let (left, right) = *dimensions.first()?;
        let mut leaf = element_descriptor(descriptor)?;
        let mut leaves = elements.iter().collect::<Vec<_>>();
        for part in inner {
            let (position, next) = match (part, &leaf.shape) {
                (SelectPart::Index(index), TypeShape::FixedArray { dimensions, .. }) => {
                    let index = self.eval_bound_i128(*index).ok()?;
                    let position = element_position(index, *dimensions.first()?)?;
                    (position, element_descriptor(&leaf)?)
                }
                (SelectPart::Member(name), TypeShape::Aggregate(layout)) => {
                    let position = layout
                        .members
                        .iter()
                        .position(|member| member.name == *name)?;
                    (position, layout.members[position].descriptor.clone())
                }
                _ => return None,
            };
            leaves = leaves
                .into_iter()
                .map(|value| match value {
                    ElementValue::Elements(values) => values.get(position),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            leaf = next;
        }
        Some((left, right, leaves, leaf))
    }

    /// Element `index` of `base`, whose elements of `width` bits lie from
    /// `left` in its most significant bits to `right`; X for an index that
    /// is unknown or out of range (SV 7.4.6, 11.5.1).
    fn select_element(
        &mut self,
        path: &str,
        base: IrExpr,
        index: NodeId,
        (left, right): (i128, i128),
        width: u32,
    ) -> Result<IrExpr, String> {
        let index = self.lower_expr(path, index)?;
        if index.is_real() || index.width == 0 {
            return Err(format!(
                "fixed-array parameter index must be integral in `{path}`"
            ));
        }
        let arithmetic_width = index
            .width
            .max(i64::BITS)
            .checked_add(u32::BITS + 2)
            .filter(|width| *width <= LLG_MAX_WIDTH)
            .ok_or_else(|| {
                "fixed-array parameter index arithmetic exceeds the supported width".to_owned()
            })?;
        let constant =
            |value: i128| IrExpr::convert_to(lhs_integer_expr(value), arithmetic_width, true);
        let index = IrExpr::convert_to(index, arithmetic_width, true);
        // Elements to the right of the selected one.
        let remaining = if left >= right {
            bin_expr(IrBinOp::Sub, index, constant(right))
        } else {
            bin_expr(IrBinOp::Sub, constant(right), index)
        };
        Ok(IrExpr::new(
            IrExprKind::IdxPartSel {
                base: Box::new(base),
                base_idx: Box::new(bin_expr(
                    IrBinOp::Mul,
                    remaining,
                    constant(i128::from(width)),
                )),
                width_expr: Box::new(lhs_integer_expr(i128::from(width))),
                neg: false,
            },
            width,
            false,
            None,
        ))
    }

    /// The type of the part of `parameter` that `parts` select.
    fn parameter_part_type(
        &self,
        parameter: NodeId,
        parts: &[SelectPart],
    ) -> Option<TypeDescriptor> {
        let mut descriptor = self.query_descriptor(parameter)?.clone();
        for part in parts {
            descriptor = match (part, &descriptor.shape) {
                (SelectPart::Index(_), TypeShape::FixedArray { .. }) => {
                    element_descriptor(&descriptor)?
                }
                (SelectPart::Member(name), TypeShape::Aggregate(layout)) => layout
                    .members
                    .iter()
                    .find(|member| member.name == *name)?
                    .descriptor
                    .clone(),
                _ => return None,
            };
        }
        Some(descriptor)
    }

    /// The parameter of a fixed unpacked type and the selectors with which
    /// `node` reads a string inside it.
    fn parameter_string_select(&self, node: NodeId) -> Option<(NodeId, Vec<SelectPart>)> {
        self.parameter_select(node).filter(|(parameter, parts)| {
            self.parameter_part_type(*parameter, parts)
                .is_some_and(|descriptor| descriptor.shape == TypeShape::String)
        })
    }

    /// Whether `node` reads a string inside a parameter of a fixed unpacked
    /// type.
    pub(in super::super) fn is_parameter_string(&self, node: NodeId) -> bool {
        self.parameter_string_select(node).is_some()
    }

    /// Lower a string inside a parameter of a fixed unpacked type.
    pub(in super::super) fn lower_parameter_string(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStringExpr>, String> {
        let Some((parameter, parts)) = self.parameter_string_select(node) else {
            return Ok(None);
        };
        self.parameter_string(path, parameter, &parts).map(Some)
    }

    /// Lower a character of a string inside a parameter of a fixed unpacked
    /// type (SV 6.16).
    pub(in super::super) fn lower_parameter_character(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrObjectQuery>, String> {
        let Some((parameter, mut parts)) = self.parameter_select(node) else {
            return Ok(None);
        };
        let Some(SelectPart::Index(character)) = parts.pop() else {
            return Ok(None);
        };
        if self
            .parameter_part_type(parameter, &parts)
            .is_none_or(|descriptor| descriptor.shape != TypeShape::String)
        {
            return Ok(None);
        }
        let text = self.parameter_string(path, parameter, &parts)?;
        let character = self.lower_container_index(path, character)?;
        Ok(Some(IrObjectQuery::StringGetc(text, Box::new(character))))
    }

    /// The leaves of the part of a parameter of a fixed unpacked type that
    /// constant selectors in `node` reach.
    pub(in super::super) fn parameter_leaf_reads(
        &self,
        node: NodeId,
    ) -> Result<Option<LeafReads>, String> {
        let Some((parameter, parts)) = self.parameter_select(node) else {
            return Ok(None);
        };
        let Some((ConstantPart::Value(value), descriptor, [])) =
            self.constant_parameter_part(parameter, &parts)
        else {
            return Ok(None);
        };
        let mut reads = Vec::new();
        Ok(constant_leaves(value, &descriptor, &mut Vec::new(), &mut reads)?.map(|()| reads))
    }

    /// The leaves of an unpacked structure with a string or real member that
    /// constant selectors in `node` reach in a parameter. One without them is
    /// read as one packed value.
    pub(in super::super) fn parameter_record_reads(
        &self,
        node: NodeId,
    ) -> Result<Option<LeafReads>, String> {
        if self.query_descriptor(node).is_none_or(|descriptor| {
            !matches!(
                &descriptor.shape,
                TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct
            ) || image_width(descriptor, None).is_some()
        }) {
            return Ok(None);
        }
        self.parameter_leaf_reads(node)
    }

    /// The string that `parts` select in `parameter`.
    fn parameter_string(
        &mut self,
        path: &str,
        parameter: NodeId,
        parts: &[SelectPart],
    ) -> Result<IrStringExpr, String> {
        let unsupported = |cg: &Self| {
            format!(
                "this selection of the strings of parameter `{}` in `{path}` is not supported",
                cg.node(parameter).name
            )
        };
        let Some((value, descriptor, rest)) = self.constant_parameter_part(parameter, parts) else {
            return Err(unsupported(self));
        };
        let ConstantPart::Value(value) = value else {
            return Ok(IrStringExpr::Literal(Vec::new()));
        };
        if rest.is_empty() {
            return string_bytes(value).map(IrStringExpr::Literal);
        }
        let mut choices = Vec::new();
        self.string_choices(value, &descriptor, rest, &mut Vec::new(), &mut choices)
            .ok_or_else(|| unsupported(self))?;
        let index_constant =
            |index: i64| IrExpr::convert_to(lhs_integer_expr(i128::from(index)), i64::BITS, true);
        // A string that no element holds is the default.
        let members = choices
            .into_iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(indices, name)| crate::sim::ir::IrEnumMember {
                value: joined_indices(indices.into_iter().map(index_constant).collect()),
                name,
            })
            .collect::<Vec<_>>();
        let mut indices = Vec::new();
        for part in rest {
            let SelectPart::Index(index) = part else {
                continue;
            };
            if self.eval_bound_i128(*index).is_ok() {
                continue;
            }
            let index = self.lower_expr(path, *index)?;
            if index.is_real() || index.width == 0 || index.width > i64::BITS {
                return Err(format!(
                    "string parameter index must be integral and at most 64 bits wide in `{path}`"
                ));
            }
            indices.push(IrExpr::convert_to(index, i64::BITS, true));
        }
        if members.is_empty() {
            return Ok(IrStringExpr::Literal(Vec::new()));
        }
        Ok(IrStringExpr::EnumName {
            receiver: Box::new(joined_indices(indices)),
            members,
        })
    }

    /// The strings that `parts` select in `value`, with the values that the
    /// run-time indices among them then have. `None` for a part that is no
    /// element or member of a value.
    fn string_choices(
        &self,
        value: &ElementValue,
        descriptor: &TypeDescriptor,
        parts: &[SelectPart],
        indices: &mut Vec<i64>,
        choices: &mut Vec<(Vec<i64>, Vec<u8>)>,
    ) -> Option<()> {
        let Some((part, rest)) = parts.split_first() else {
            choices.push((indices.clone(), string_bytes(value).ok()?));
            return Some(());
        };
        let ElementValue::Elements(elements) = value else {
            return None;
        };
        match (part, &descriptor.shape) {
            (SelectPart::Index(index), TypeShape::FixedArray { dimensions, .. }) => {
                let bounds = *dimensions.first()?;
                let element = element_descriptor(descriptor)?;
                if let Ok(index) = self.eval_bound_i128(*index) {
                    let Some(position) = element_position(index, bounds) else {
                        return Some(());
                    };
                    let value = elements.get(position)?;
                    return self.string_choices(value, &element, rest, indices, choices);
                }
                let (left, right) = (i64::from(bounds.0), i64::from(bounds.1));
                let step = if left >= right { -1 } else { 1 };
                for (value, offset) in elements.iter().zip(0i64..) {
                    indices.push(left + step * offset);
                    self.string_choices(value, &element, rest, indices, choices)?;
                    indices.pop();
                }
                Some(())
            }
            (SelectPart::Member(name), TypeShape::Aggregate(layout)) => {
                let position = layout
                    .members
                    .iter()
                    .position(|member| member.name == *name)?;
                let member = &layout.members[position].descriptor;
                self.string_choices(elements.get(position)?, member, rest, indices, choices)
            }
            _ => None,
        }
    }
}

/// The run-time indices of a selection as one value.
fn joined_indices(mut indices: Vec<IrExpr>) -> IrExpr {
    if indices.len() == 1 {
        return indices.remove(0);
    }
    let width = indices.iter().map(|index| index.width).sum();
    IrExpr::new(IrExprKind::Concat { parts: indices }, width, false, None)
}

/// The position from the left bound of element `index`, if it exists.
fn element_position(index: i128, (left, right): (i32, i32)) -> Option<usize> {
    (i128::from(left.min(right))..=i128::from(left.max(right)))
        .contains(&index)
        .then(|| index.abs_diff(i128::from(left)))
        .and_then(|position| usize::try_from(position).ok())
}

fn string_bytes(value: &ElementValue) -> Result<Vec<u8>, String> {
    match value {
        ElementValue::Singular(Val::Str(text)) => decode_verilog_string(text),
        _ => Err("string parameter element has no captured string value".to_owned()),
    }
}

/// The value of type `descriptor` that `selected` is the image of.
fn image_value(
    selected: IrExpr,
    descriptor: &TypeDescriptor,
    width: u32,
    signed: bool,
) -> Result<IrExpr, String> {
    match descriptor.shape {
        TypeShape::Real { .. } => Ok(IrExpr::new(
            IrExprKind::SysFunc(Box::new(IrSysFunc::BitsToReal(Box::new(selected)))),
            0,
            true,
            None,
        )),
        _ => ir_to_storage(selected, width, signed, descriptor.two_state),
    }
}

fn constant_image(value: &elab::Value) -> Result<IrExpr, String> {
    let constant = val_to_const(value)?;
    let width = constant.width;
    Ok(IrExpr::new(IrExprKind::Const(constant), width, false, None))
}

fn part_select(base: IrExpr, offset: u32, width: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::PartSel {
            base: Box::new(base),
            left: i64::from(offset + width - 1),
            right: i64::from(offset),
        },
        width,
        false,
        None,
    )
}

/// The packed or real value of a parameter part, if it has one.
fn constant_part_value(
    part: &ConstantPart<'_>,
    descriptor: &TypeDescriptor,
) -> Result<Option<IrExpr>, String> {
    match (&descriptor.shape, part) {
        (TypeShape::Real { .. }, ConstantPart::Value(ElementValue::Singular(value))) => {
            Ok(match value {
                Val::Real(value) => Some(real_literal_expr(*value)),
                _ => None,
            })
        }
        (TypeShape::Real { .. }, ConstantPart::Nonexistent) => Ok(Some(real_literal_expr(0.0))),
        _ => {
            let Some(width) = image_width(descriptor, None) else {
                return Ok(None);
            };
            let Some(bits) = part_image(part, descriptor) else {
                return Ok(None);
            };
            let value = constant_image(&elab::Value::from_bits(bits, false))?;
            ir_to_storage(value, width, descriptor.info.signed, descriptor.two_state).map(Some)
        }
    }
}

/// The leaves of `value`, a part of a parameter with type `descriptor`, at
/// their paths below `path`. `None` for a value of another type.
fn constant_leaves(
    value: &ElementValue,
    descriptor: &TypeDescriptor,
    path: &mut Vec<AggregatePathPart>,
    reads: &mut LeafReads,
) -> Result<Option<()>, String> {
    let mut below = |part, value, descriptor: &TypeDescriptor| {
        path.push(part);
        let found = constant_leaves(value, descriptor, path, reads);
        path.pop();
        found
    };
    match (value, &descriptor.shape) {
        (ElementValue::Elements(elements), TypeShape::FixedArray { dimensions, .. }) => {
            let (Some((left, right)), Some(element)) =
                (dimensions.first(), element_descriptor(descriptor))
            else {
                return Ok(None);
            };
            let step = if left <= right { 1 } else { -1 };
            for (value, offset) in elements.iter().zip(0i32..) {
                let index = AggregatePathPart::Index(left + step * offset);
                if below(index, value, &element)?.is_none() {
                    return Ok(None);
                }
            }
            Ok(Some(()))
        }
        (
            ElementValue::Elements(members),
            TypeShape::Aggregate(crate::core::db::AggregateLayout {
                kind: AggregateKind::UnpackedStruct,
                members: layout,
                ..
            }),
        ) => {
            for (value, member) in members.iter().zip(layout) {
                let part = AggregatePathPart::Member(member.name.clone());
                if below(part, value, &member.descriptor)?.is_none() {
                    return Ok(None);
                }
            }
            Ok(Some(()))
        }
        (ElementValue::Singular(_), TypeShape::String) => {
            let text = IrStringExpr::Literal(string_bytes(value)?);
            reads.push((path.clone(), LeafValue::String(text)));
            Ok(Some(()))
        }
        (ElementValue::Singular(_), shape) => {
            let leaf = constant_part_value(&ConstantPart::Value(value), descriptor)?;
            let Some(leaf) = leaf else {
                return Ok(None);
            };
            reads.push((
                path.clone(),
                match shape {
                    TypeShape::Real { .. } => LeafValue::Real(leaf),
                    _ => LeafValue::Packed(leaf),
                },
            ));
            Ok(Some(()))
        }
        _ => Ok(None),
    }
}

/// The packed image of a parameter part, most significant bit first, in the
/// layout of [`image_path_descriptor`]. `None` for a part with a string.
fn part_image(part: &ConstantPart<'_>, descriptor: &TypeDescriptor) -> Option<Vec<Bit>> {
    match part {
        ConstantPart::Value(value) => image(value, descriptor),
        ConstantPart::Nonexistent => {
            let width = image_width(descriptor, REAL_IMAGE_WIDTH)?;
            Some(vec![Bit::X; width as usize])
        }
    }
}

fn image(value: &ElementValue, descriptor: &TypeDescriptor) -> Option<Vec<Bit>> {
    let width = image_width(descriptor, REAL_IMAGE_WIDTH)? as usize;
    match (value, &descriptor.shape) {
        (ElementValue::Singular(Val::Bits(value)), _) => {
            (value.width() == width).then(|| value.bits.clone())
        }
        (ElementValue::Singular(Val::Real(value)), TypeShape::Real { .. }) => {
            let bits = value.to_bits();
            Some(
                (0..u64::BITS)
                    .rev()
                    .map(|bit| {
                        if bits >> bit & 1 == 1 {
                            Bit::One
                        } else {
                            Bit::Zero
                        }
                    })
                    .collect(),
            )
        }
        (ElementValue::Elements(elements), TypeShape::FixedArray { .. }) => {
            let element = element_descriptor(descriptor)?;
            let mut bits = Vec::with_capacity(width);
            for value in elements {
                bits.extend(image(value, &element)?);
            }
            (bits.len() == width).then_some(bits)
        }
        (ElementValue::Elements(members), TypeShape::Aggregate(layout)) => {
            let mut bits = Vec::with_capacity(width);
            for (value, member) in members.iter().zip(&layout.members) {
                bits.extend(image(value, &member.descriptor)?);
            }
            (bits.len() == width).then_some(bits)
        }
        (ElementValue::Union { member, value }, TypeShape::Aggregate(layout)) => {
            let member = layout.members.get(*member)?;
            let path = [AggregatePathPart::Member(member.name.clone())];
            let (member, offset) = image_path_descriptor(descriptor, &path, REAL_IMAGE_WIDTH)?;
            let fill = if descriptor.two_state {
                Bit::Zero
            } else {
                Bit::X
            };
            let part = image(value, &member)?;
            let end = width - offset as usize;
            let mut bits = vec![fill; width];
            bits[end - part.len()..end].copy_from_slice(&part);
            Some(bits)
        }
        _ => None,
    }
}
