//! Owned-database collection, storage allocation, wiring, and process setup.

use super::*;
use crate::core::db::ConcurrentAssertionKind;
use crate::sim::ir::{
    IrChandleExpr, IrClass, IrClassField, IrClassFieldType, IrContainerElement, IrContainerMember,
    IrObjectStmt, IrObjectType, IrStringExpr, IrVirtualInterface, IrVirtualInterfaceInstance,
    IrVirtualInterfaceMember, IrVirtualInterfaceMethod,
};
use std::collections::BTreeSet;

mod aggregates;
mod arguments;
mod call_contracts;
mod calls;
mod captures;
mod classes;
mod constants;
mod dependencies;
mod design;
mod events;
mod function_bodies;
mod gates;
mod initialization;
mod locals;
mod lvalues;
mod names;
mod nets;
mod packed_elements;
pub(super) mod packed_formals;
mod ports;
mod processes;
mod signatures;
mod virtual_interfaces;

type VirtualInterfaceMemberEntries = Vec<(String, SignalInfo)>;
type VirtualInterfaceMethodEntries = Vec<(String, usize)>;
type VirtualInterfaceMethodInfo = (usize, usize, NodeId, NodeId, NodeId);

fn lower_container_element(descriptor: &TypeDescriptor) -> Result<IrContainerElement, String> {
    let packed = || {
        descriptor
            .info
            .width
            .filter(|width| *width != 0)
            .map(|width| IrContainerElement::Packed {
                width,
                signed: descriptor.info.signed,
                two_state: descriptor.info.kind == "bit"
                    || matches!(
                        descriptor.info.kind.as_str(),
                        "int" | "longint" | "byte" | "shortint"
                    ),
            })
    };
    match &descriptor.shape {
        TypeShape::PackedAtom { .. } => packed().ok_or_else(|| {
            format!(
                "container element `{}` has no representable packed width",
                descriptor.name
            )
        }),
        TypeShape::Real { shortreal } => Ok(IrContainerElement::Real {
            shortreal: *shortreal,
        }),
        TypeShape::String => Ok(IrContainerElement::String),
        TypeShape::Aggregate(layout)
            if matches!(
                layout.kind,
                AggregateKind::PackedStruct | AggregateKind::PackedUnion
            ) && descriptor.info.width.is_some() =>
        {
            packed().ok_or_else(|| {
                format!(
                    "container element `{}` has no representable packed width",
                    descriptor.name
                )
            })
        }
        TypeShape::Aggregate(layout) => Ok(IrContainerElement::Aggregate {
            type_id: descriptor.id.0,
            members: layout
                .members
                .iter()
                .map(|member| {
                    Ok(IrContainerMember {
                        name: member.name.clone(),
                        element: Box::new(lower_container_element(&member.descriptor)?),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        }),
        TypeShape::FixedArray {
            dimensions,
            element,
        } => Ok(IrContainerElement::FixedArray {
            dimensions: dimensions.clone(),
            element: Box::new(lower_container_element(element)?),
        }),
        TypeShape::Container { kind, element } => Ok(IrContainerElement::Container {
            type_id: element.id.0,
            kind: kind.clone(),
            element: Box::new(lower_container_element(element)?),
        }),
        TypeShape::Opaque { kind } if kind == "Chandle" || kind == "VirtualInterface" => {
            Ok(IrContainerElement::Chandle)
        }
        TypeShape::Opaque { kind } if kind == "Event" => Ok(IrContainerElement::Event),
        TypeShape::Opaque { kind } => Ok(IrContainerElement::Opaque {
            type_id: descriptor.id.0,
            kind: kind.clone(),
        }),
    }
}

pub(super) fn aggregate_path_suffix(path: &[AggregatePathPart]) -> String {
    path.iter()
        .map(|part| match part {
            AggregatePathPart::Member(name) => name.clone(),
            AggregatePathPart::Index(index) => format!("i{index}"),
        })
        .collect::<Vec<_>>()
        .join("__")
}

pub(super) fn aggregate_path_key(path: &[AggregatePathPart]) -> String {
    path.iter()
        .map(|part| match part {
            AggregatePathPart::Member(name) => format!("m:{name}"),
            AggregatePathPart::Index(index) => format!("i:{index}"),
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn leaf_member(base: &AggregateMember, descriptor: &TypeDescriptor) -> AggregateMember {
    let packed_ranges = match &descriptor.shape {
        TypeShape::PackedAtom { ranges } => ranges.clone(),
        _ => base.packed_ranges.clone(),
    };
    AggregateMember {
        initializer: base.initializer.clone(),
        name: base.name.clone(),
        ty: descriptor.info.clone(),
        two_state: base.two_state,
        packed_ranges,
        aggregate: None,
        descriptor: descriptor.clone(),
    }
}

fn port_array_index_vectors(dims: &[(i32, i32)]) -> Vec<Vec<i32>> {
    fn visit(
        dims: &[(i32, i32)],
        dimension: usize,
        current: &mut Vec<i32>,
        values: &mut Vec<Vec<i32>>,
    ) {
        if dimension == dims.len() {
            values.push(current.clone());
            return;
        }
        let (left, right) = dims[dimension];
        let step = if left <= right { 1 } else { -1 };
        let mut index = left;
        loop {
            current.push(index);
            visit(dims, dimension + 1, current, values);
            current.pop();
            if index == right {
                break;
            }
            index = index.saturating_add(step);
        }
    }

    let mut values = Vec::new();
    visit(dims, 0, &mut Vec::new(), &mut values);
    values
}

/// Return the canonical scalar spelling used in a generated DPI-C prototype.
/// Packed vectors wider than one bit, 4-state integer atoms beyond scalar
/// `logic`/`reg`, and all aggregates remain reserved for later work.
fn dpi_type_key(ty: &TypeInfo, width: u32, two_state: bool) -> Result<String, String> {
    let key = match ty.kind.as_str() {
        "bit" if width == 1 && two_state => "svBit".to_owned(),
        "logic" | "reg" if width == 1 && !two_state => "svLogic".to_owned(),
        "byte" if width == 8 && two_state => {
            if ty.signed { "int8_t" } else { "uint8_t" }.to_owned()
        }
        "shortint" if width == 16 && two_state => {
            if ty.signed { "int16_t" } else { "uint16_t" }.to_owned()
        }
        "int" if width == 32 && two_state => {
            if ty.signed { "int32_t" } else { "uint32_t" }.to_owned()
        }
        "longint" if width == 64 && two_state => {
            if ty.signed { "int64_t" } else { "uint64_t" }.to_owned()
        }
        "real" if width == 0 => "real".to_owned(),
        "shortreal" if width == 0 => "shortreal".to_owned(),
        "chandle" if width == 0 => "chandle".to_owned(),
        "string" if width == 0 => "string".to_owned(),
        _ => {
            return Err(format!(
                "DPI-C type `{}` ({} bits, {}) is outside the supported scalar ABI",
                ty.render(),
                width,
                if two_state { "2-state" } else { "4-state" }
            ));
        }
    };
    Ok(key)
}

#[derive(Default)]
struct ProcessContractScan {
    event_controls: Vec<NodeId>,
    fork_controls: Vec<NodeId>,
    blocking_timing_controls: Vec<NodeId>,
    disallowed_assignments: Vec<NodeId>,
    event_triggers: Vec<NodeId>,
}

#[derive(Clone)]
struct ProcessWriter {
    node: NodeId,
    label: String,
    writes: HashSet<IrDependency>,
}

fn materialize_decl_cast_value(
    value: elab::Value,
    width: usize,
    signed: bool,
    two_state: bool,
) -> elab::Value {
    let mut value = value.cast(width, signed);
    // A cast returns the value held by a temporary of the target type (IEEE
    // 1800-2009 §6.24.1). Its target width has therefore materialized an
    // unbased unsized fill; do not let an enclosing declaration assignment
    // refill to a second, wider destination.
    value.fill = None;
    if two_state {
        for bit in &mut value.bits {
            if matches!(*bit, Bit::X | Bit::Z) {
                *bit = Bit::Zero;
            }
        }
    }
    value
}

fn aggregate_member_matches_type_key(
    member: &AggregateMember,
    _key: &str,
    key_type: Option<&AssignmentPatternKeyType>,
) -> bool {
    let Some(key_type) = key_type else {
        return false;
    };
    pattern_key_matches_descriptor(
        key_type,
        &member.descriptor,
        member.two_state,
        Some(&member.packed_ranges),
    )
}

/// Match a resolved assignment-pattern type key against a complete recursive
/// descriptor. Frontend type identity is authoritative; display names and
/// structural similarity cannot make two nominal types compatible.
pub(super) fn pattern_key_matches_descriptor(
    key_type: &AssignmentPatternKeyType,
    descriptor: &TypeDescriptor,
    two_state: bool,
    packed_ranges: Option<&[crate::core::db::PackedRange]>,
) -> bool {
    if key_type.type_id != descriptor.id {
        return false;
    }
    let nominal = |kind: &str| matches!(kind, "struct" | "union" | "enum" | "class");
    if nominal(&key_type.ty.kind) || nominal(&descriptor.info.kind) {
        return key_type.ty.kind == descriptor.info.kind && key_type.two_state == two_state;
    }
    if key_type.ty.kind == "array" || descriptor.info.kind == "array" {
        return key_type.ty.kind == descriptor.info.kind;
    }
    if key_type.ty.kind == "real" || descriptor.info.kind == "real" {
        return key_type.ty.kind == descriptor.info.kind;
    }
    if key_type.ty.kind == "string" || descriptor.info.kind == "string" {
        return key_type.ty.kind == descriptor.info.kind;
    }
    if !is_integral_pattern_key_kind(&key_type.ty.kind)
        || !is_integral_pattern_key_kind(&descriptor.info.kind)
        || key_type.ty.signed != descriptor.info.signed
        || key_type.two_state != two_state
    {
        return false;
    }
    let descriptor_ranges = match &descriptor.shape {
        TypeShape::PackedAtom { ranges } => ranges.as_slice(),
        _ => &[],
    };
    let effective_ranges = |ranges: &[crate::core::db::PackedRange], width: Option<u32>| {
        if !ranges.is_empty() {
            return ranges.to_vec();
        }
        width
            .and_then(|width| width.checked_sub(1))
            .map(|left| {
                vec![crate::core::db::PackedRange {
                    left: i128::from(left),
                    right: 0,
                }]
            })
            .unwrap_or_default()
    };
    effective_ranges(&key_type.packed_ranges, key_type.ty.width)
        == effective_ranges(
            packed_ranges.unwrap_or(descriptor_ranges),
            descriptor.info.width,
        )
}

pub(super) fn pattern_key_types_equal(
    left: &AssignmentPatternKeyType,
    right: &AssignmentPatternKeyType,
) -> bool {
    left.type_id == right.type_id
        && left.two_state == right.two_state
        && left.ty.kind == right.ty.kind
        && left.ty.width == right.ty.width
        && left.ty.signed == right.ty.signed
        && left.packed_ranges == right.packed_ranges
}

pub(super) fn pattern_key_matches_type_descriptor(
    key_type: &AssignmentPatternKeyType,
    descriptor: &TypeDescriptor,
    two_state: bool,
) -> bool {
    pattern_key_matches_descriptor(key_type, descriptor, two_state, None)
}

fn is_integral_pattern_key_kind(kind: &str) -> bool {
    matches!(
        kind,
        "bit" | "logic" | "reg" | "byte" | "shortint" | "int" | "longint" | "integer" | "time"
    )
}

fn materialize_parameter_value(value: &Val) -> Val {
    match value {
        Val::Bits(value) => {
            let mut value = value.clone();
            // A parameter reference denotes its declared/inferred finite
            // value (IEEE 1800-2009 §6.20.2). A frontend can retain the
            // initializer's unbased fill marker after it has already resized
            // the payload, so preserve the elaborated width/bits/signedness
            // but clear that stale contextual marker.
            value.fill = None;
            Val::Bits(value)
        }
        Val::Str(value) => Val::Str(value.clone()),
        Val::Real(value) => Val::Real(*value),
    }
}

/// Convert semantic drive strengths to the ordered IEEE 1800-2009
/// Table 28-7 scale used by the generated runtime. Charge strengths are valid
/// for trireg storage, not continuous-assignment drive strengths.
fn continuous_assignment_strengths(
    strength0: Strength,
    strength1: Strength,
    net_name: &str,
) -> Result<(u8, u8), String> {
    fn level(strength: Strength, net_name: &str) -> Result<u8, String> {
        match strength {
            Strength::Unspecified | Strength::Strong => Ok(6),
            Strength::Supply => Ok(7),
            Strength::Pull => Ok(5),
            Strength::Weak => Ok(3),
            Strength::HighZ => Ok(0),
            Strength::Large | Strength::Medium | Strength::Small => Err(format!(
                "charge strength on continuous assignment to net `{net_name}` is not supported"
            )),
            Strength::Unsupported => Err(format!(
                "unsupported drive strength on continuous assignment to net `{net_name}`"
            )),
        }
    }

    let zero = level(strength0, net_name)?;
    let one = level(strength1, net_name)?;
    if zero == 0 && one == 0 {
        return Err(format!(
            "continuous assignment to net `{net_name}` specifies high impedance for both logic values"
        ));
    }
    Ok((zero, one))
}

/// Apply the language restriction that an explicit continuous-assignment
/// strength belongs only to a scalar net.  The resolved runtime still carries
/// one strength pair per structural driver, so scalar wired and collapsed
/// groups use the same path as ordinary wires.
fn continuous_assignment_strengths_for_width(
    strength0: Strength,
    strength1: Strength,
    net_name: &str,
    width: u32,
) -> Result<(u8, u8), String> {
    if (strength0 != Strength::Unspecified || strength1 != Strength::Unspecified) && width != 1 {
        return Err(format!(
            "drive strength on non-scalar net `{net_name}` is not permitted by IEEE 1800-2009 10.3.4"
        ));
    }
    continuous_assignment_strengths(strength0, strength1, net_name)
}

/// Return the effective drive pair for a primitive output.  Ordinary gate
/// outputs default to strong/strong, while pullup/pulldown primitives are
/// pull-strength sources.  Explicit gate strengths retain their asymmetric
/// endpoints and flow into the canonical structural-driver slot.
fn gate_driver_strengths(
    prim_type: PrimitiveType,
    strength0: Strength,
    strength1: Strength,
    net_name: &str,
) -> Result<(u8, u8), String> {
    let (strength0, strength1) =
        if strength0 == Strength::Unspecified && strength1 == Strength::Unspecified {
            match prim_type {
                PrimitiveType::Pullup => (Strength::HighZ, Strength::Pull),
                PrimitiveType::Pulldown => (Strength::Pull, Strength::HighZ),
                _ => (Strength::Strong, Strength::Strong),
            }
        } else {
            (strength0, strength1)
        };
    continuous_assignment_strengths(strength0, strength1, net_name)
}

/// Return the effective drive pair for an output port. Port strengths use the
/// same endpoint legality as continuous drivers; an omitted declaration is
/// the ordinary strong/strong source. Keeping this conversion beside the
/// gate/continuous helpers gives collapsed groups one resolver contract for
/// every structural source.
fn port_driver_strengths(
    strength0: Strength,
    strength1: Strength,
    net_name: &str,
) -> Result<(u8, u8), String> {
    continuous_assignment_strengths(strength0, strength1, net_name)
}

#[cfg(test)]
mod tests {
    use super::{materialize_decl_cast_value, materialize_parameter_value};
    use crate::core::elab::{Bit, Val, Value};

    #[test]
    fn declaration_cast_materializes_fill_before_outer_assignment() {
        let mut fill = Value::from_bits(vec![Bit::One], false);
        fill.fill = Some(Bit::One);

        let cast = materialize_decl_cast_value(fill, 1, false, false);
        assert_eq!(cast.fill, None);
        assert_eq!(cast.cast(8, false).to_u128(), Some(1));
    }

    #[test]
    fn parameter_value_drops_initializer_fill_after_declared_resize() {
        let mut elaborated = Value::from_u64(1, 8, false);
        elaborated.fill = Some(Bit::One);

        let Val::Bits(materialized) = materialize_parameter_value(&Val::Bits(elaborated)) else {
            panic!("packed parameter must remain packed");
        };
        assert_eq!(materialized.width(), 8);
        assert!(!materialized.signed);
        assert_eq!(materialized.fill, None);
        assert_eq!(materialized.to_u128(), Some(1));
    }
}
