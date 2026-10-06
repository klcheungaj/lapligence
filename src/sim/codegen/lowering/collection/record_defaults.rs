//! Member defaults of records with string, real, handle or container leaves.
//!
//! A record variable declared without an initializer starts from the
//! default member values of its type; an explicit declaration initializer
//! replaces them entirely (SV 7.2.2). The outermost member default on a
//! leaf's path wins, so a default on a nested record member overrides that
//! record type's own member defaults. Fixed records keep theirs in
//! `fixed_defaults.rs`; these leaves have no packed payload to carry them.
use super::fixed_values::{fixed_constant_slice, fixed_path_descriptor, fixed_width};
use super::*;

/// The default member value of one record leaf.
#[derive(Clone, Debug, PartialEq)]
pub(in super::super) enum LeafDefault {
    Packed(IrConst),
    Real(f64),
    String(Vec<u8>),
    /// A null handle: the storage default, nothing to write.
    Null,
}

impl Codegen<'_> {
    /// The member default that initializes the leaf at `path` of a record
    /// of type `root`, or `None` when no member on the path has one.
    /// Defaults that the leaf storage cannot represent reject.
    pub(in super::super) fn record_member_default(
        root: &TypeDescriptor,
        path: &[AggregatePathPart],
    ) -> Result<Option<LeafDefault>, String> {
        let mut descriptor = root.clone();
        for (position, part) in path.iter().enumerate() {
            let next = match (part, &descriptor.shape) {
                (AggregatePathPart::Member(name), TypeShape::Aggregate(layout)) => {
                    let Some(member) = layout.members.iter().find(|member| &member.name == name)
                    else {
                        return Ok(None);
                    };
                    if let Some(value) = &member.initializer {
                        return Self::member_default_leaf(member, value, &path[position + 1..])
                            .map(Some);
                    }
                    member.descriptor.clone()
                }
                (
                    AggregatePathPart::Index(_),
                    TypeShape::FixedArray {
                        dimensions,
                        element,
                    },
                ) => {
                    if dimensions.len() > 1 {
                        TypeDescriptor {
                            shape: TypeShape::FixedArray {
                                dimensions: dimensions[1..].to_vec(),
                                element: element.clone(),
                            },
                            ..descriptor.clone()
                        }
                    } else {
                        element.as_ref().clone()
                    }
                }
                _ => return Ok(None),
            };
            descriptor = next;
        }
        Ok(None)
    }

    /// The part of `member`'s default `value` at `rest` below the member.
    fn member_default_leaf(
        member: &AggregateMember,
        value: &ValueData,
        rest: &[AggregatePathPart],
    ) -> Result<LeafDefault, String> {
        let unsupported = || {
            format!(
                "member default of `{}` is not supported: the value has no representation for this member type",
                member.name
            )
        };
        let descriptor = &member.descriptor;
        if rest.is_empty() {
            match &descriptor.shape {
                TypeShape::Real { .. } => {
                    return match val_from_value_data(value, 64) {
                        Ok(Val::Real(value)) => Ok(LeafDefault::Real(value)),
                        _ => Err(unsupported()),
                    }
                }
                TypeShape::String => {
                    return match value {
                        ValueData::Bytes(bytes) => Ok(LeafDefault::String(bytes.clone())),
                        ValueData::Str(text) => Ok(LeafDefault::String(text.as_bytes().to_vec())),
                        _ => Err(unsupported()),
                    }
                }
                // The only constant handle value is `null`.
                TypeShape::Opaque { .. } => {
                    return match value {
                        ValueData::None => Ok(LeafDefault::Null),
                        _ => Err(unsupported()),
                    }
                }
                _ => {}
            }
        }
        let width = fixed_width(descriptor).ok_or_else(unsupported)?;
        let Ok(Val::Bits(bits)) = val_from_value_data(value, width as i32) else {
            return Err(unsupported());
        };
        let whole = val_to_const(&bits.cast(width as usize, descriptor.info.signed))?;
        if rest.is_empty() {
            return Ok(LeafDefault::Packed(whole));
        }
        let (leaf, offset) = fixed_path_descriptor(descriptor, rest).ok_or_else(unsupported)?;
        let leaf_width = fixed_width(&leaf).ok_or_else(unsupported)?;
        Ok(LeafDefault::Packed(fixed_constant_slice(
            &whole,
            offset,
            leaf_width,
            leaf.info.signed,
        )))
    }

    /// Initialize the leaf storage of record `node` (module or static block
    /// variable without a declaration initializer) from its member defaults.
    pub(super) fn apply_record_member_defaults(
        &mut self,
        path: &str,
        node: NodeId,
        descriptor: &TypeDescriptor,
        leaves: &[AggregateMemberInfo],
    ) -> Result<(), String> {
        if !matches!(self.kind(node), NodeKind::Var { .. })
            || self.db.var_initializer(node).is_some()
        {
            return Ok(());
        }
        for leaf in leaves {
            let default = Self::record_member_default(descriptor, &leaf.path).map_err(|error| {
                format!("{error} (record `{}` in `{path}`)", self.node(node).name)
            })?;
            let Some(default) = default else {
                continue;
            };
            match (default, &leaf.signal, leaf.object) {
                (LeafDefault::Packed(value), Some(signal), _) if !signal.real => {
                    self.model.signals[signal.ir].fixed_default = Some(value);
                }
                (LeafDefault::Real(value), Some(signal), _) if signal.real => {
                    self.var_inits
                        .push((signal.clone(), decl_value_to_const(Val::Real(value))?));
                }
                (LeafDefault::String(bytes), _, Some(object)) => {
                    self.model.objects[object].initial = Some(IrStringExpr::Literal(bytes));
                    self.reserve_initializer_order(node);
                    let identity = self.declaration_identity(node)?;
                    self.object_initializers.push((object, identity));
                }
                (LeafDefault::Null, _, Some(_)) => {}
                _ => {
                    return Err(format!(
                        "member default of `{}` in record `{}` in `{path}` is not supported",
                        aggregate_path_suffix(&leaf.path),
                        self.node(node).name
                    ))
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::{AggregateLayout, TypeId};
    use crate::core::model::TypeInfo;

    fn descriptor(id: u64, info: TypeInfo, shape: TypeShape) -> TypeDescriptor {
        TypeDescriptor {
            id: TypeId(id),
            two_state: false,
            name: format!("t{id}"),
            info,
            shape,
        }
    }

    fn int() -> TypeDescriptor {
        descriptor(
            1,
            TypeInfo {
                kind: "int".to_owned(),
                width: Some(32),
                signed: true,
                type_name: None,
            },
            TypeShape::PackedAtom { ranges: vec![] },
        )
    }

    fn member(
        name: &str,
        descriptor: TypeDescriptor,
        initializer: Option<ValueData>,
    ) -> AggregateMember {
        AggregateMember {
            initializer,
            name: name.to_owned(),
            ty: descriptor.info.clone(),
            two_state: false,
            packed_ranges: Vec::new(),
            aggregate: None,
            descriptor,
        }
    }

    fn record(id: u64, members: Vec<AggregateMember>) -> TypeDescriptor {
        descriptor(
            id,
            TypeInfo::default(),
            TypeShape::Aggregate(AggregateLayout {
                kind: AggregateKind::UnpackedStruct,
                type_identity: Some(format!("type#{id}")),
                type_id: Some(TypeId(id)),
                members,
            }),
        )
    }

    fn int_value(value: u64, width: u64) -> ValueData {
        ValueData::Vector {
            bit_width: width,
            is_signed: false,
            value_words: vec![value],
            unknown_words: vec![0],
        }
    }

    fn part(name: &str) -> AggregatePathPart {
        AggregatePathPart::Member(name.to_owned())
    }

    fn packed(default: Option<LeafDefault>) -> u64 {
        match default {
            Some(LeafDefault::Packed(value)) => value.bits[0],
            other => panic!("expected a packed default, got {other:?}"),
        }
    }

    #[test]
    fn outermost_member_default_wins_and_slices_array_elements() {
        let inner = record(10, vec![member("a", int(), Some(int_value(5, 32)))]);
        let array = descriptor(
            11,
            TypeInfo::default(),
            TypeShape::FixedArray {
                dimensions: vec![(0, 1)],
                element: Box::new(int()),
            },
        );
        let root = record(
            12,
            vec![
                member("plain", inner.clone(), None),
                // The member default on `over` (a = 9) replaces `in`'s own.
                member("over", inner, Some(int_value(9, 32))),
                // Element 0 is the most significant word of '{7, 8}.
                member("arr", array, Some(int_value((7 << 32) | 8, 64))),
                member(
                    "s",
                    descriptor(13, TypeInfo::default(), TypeShape::String),
                    Some(ValueData::Bytes(b"x".to_vec())),
                ),
                member("none", int(), None),
            ],
        );
        let default =
            |path: &[AggregatePathPart]| Codegen::record_member_default(&root, path).unwrap();
        assert_eq!(packed(default(&[part("plain"), part("a")])), 5);
        assert_eq!(packed(default(&[part("over"), part("a")])), 9);
        assert_eq!(
            packed(default(&[part("arr"), AggregatePathPart::Index(0)])),
            7
        );
        assert_eq!(
            packed(default(&[part("arr"), AggregatePathPart::Index(1)])),
            8
        );
        assert_eq!(
            default(&[part("s")]),
            Some(LeafDefault::String(b"x".to_vec()))
        );
        assert_eq!(default(&[part("none")]), None);
    }

    #[test]
    fn unrepresentable_member_defaults_reject() {
        let string = descriptor(20, TypeInfo::default(), TypeShape::String);
        let inner = record(21, vec![member("n", string, None)]);
        let root = record(22, vec![member("inner", inner, Some(ValueData::None))]);
        let error = Codegen::record_member_default(&root, &[part("inner"), part("n")])
            .expect_err("a record-valued default has no captured constant");
        assert!(
            error.contains("member default of `inner` is not supported"),
            "{error}"
        );
    }
}
