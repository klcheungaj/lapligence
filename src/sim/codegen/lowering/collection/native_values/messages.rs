//! Aggregate mailbox messages (SIM-017).
//!
//! A record, unpacked array, queue or dynamic-array message travels as one
//! descriptor-backed runtime value. The sender fills a lexical native value
//! of the message type through the ordinary SIM-003/SIM-006 transfers and
//! the runtime deep-copies it into the message; a receiver gets the message
//! into a lexical value of its destination's type and copies it out to the
//! destination afterwards. Identity handles inside the value are shared,
//! everything else is copied (SV 15.4, 7.5-7.10).
use super::*;

/// Statements around one mailbox operation on an aggregate message.
pub(in crate::sim::codegen) struct MailboxMessage {
    /// Run before the operation (declarations, source transfer, frozen
    /// destination selectors).
    pub(in crate::sim::codegen) before: Vec<IrStmt>,
    /// The lexical native value the message is copied from or into.
    pub(in crate::sim::codegen) value: usize,
    /// Run after a successful retrieval: the copy-out to the destination.
    pub(in crate::sim::codegen) after: Vec<IrStmt>,
}

impl Codegen<'_> {
    /// A lexical native value of type `descriptor` for the message operand
    /// `site`. Queue and dynamic-array parts (including a whole-container
    /// message) live in companion containers, as a native record's do.
    fn message_temporary(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        site: NodeId,
    ) -> Result<usize, String> {
        let element = lower_container_element(descriptor)?;
        validate_native_type(&element, "mailbox message").map_err(|error| {
            format!(
                "mailbox message of type `{}` in `{path}` is not supported: {error}",
                descriptor.name
            )
        })?;
        let mut leaves = NativeLeaves::default();
        collect_native_root_leaves(descriptor, &element, &mut leaves).map_err(|error| {
            format!(
                "mailbox message of type `{}` in `{path}` is not supported: {error}",
                descriptor.name
            )
        })?;
        // A message value nests a queue or dynamic array as a dynamic array
        // in its slot; the runtime has no nested associative form.
        if let Some(leaf) = leaves
            .containers
            .iter()
            .find(|leaf| matches!(leaf.kind, IrContainerKind::Associative { .. }))
        {
            let part = if leaf.path.is_empty() {
                "the message".to_owned()
            } else {
                format!("member `{}`", aggregate_path_suffix(&leaf.path))
            };
            return Err(format!(
                "mailbox message of type `{}` in `{path}` is not supported: {part} is an associative array, which has no nested value form (SIM-017)",
                descriptor.name
            ));
        }
        let ty = match self
            .model
            .native_types
            .iter()
            .position(|existing| *existing == element)
        {
            Some(ty) => ty,
            None => {
                self.model.native_types.push(element);
                self.model.native_types.len() - 1
            }
        };
        let layout = NativeLayout {
            ty,
            descriptor: descriptor.clone(),
            leaves: leaves.scalars,
            containers: leaves.containers,
        };
        let companions = self.native_companions(&layout, true);
        // `site` is an expression, never a declaration that native storage
        // collection could mistake for a record variable.
        self.native_layouts.insert(site, layout);
        let index = self.model.native_values.len();
        self.model.native_values.push(IrNativeValue {
            c_name: format!("S_llg_native_{index}"),
            ty,
            activation: true,
            companions,
            class_field: None,
            receiver: None,
        });
        self.native_value_layouts.insert(index, site);
        Ok(index)
    }

    /// Build the message sent by `put`/`try_put` from `source`.
    pub(in crate::sim::codegen) fn mailbox_message_source(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        source: NodeId,
    ) -> Result<MailboxMessage, String> {
        let value = self.message_temporary(path, descriptor, source)?;
        let mut before = vec![IrStmt::NativeValueDeclare(value)];
        if matches!(descriptor.shape, TypeShape::Container { .. }) {
            // A whole queue or dynamic array: any container source fills the
            // companion, which then becomes the value's nested array.
            let companion = self.model.native_values[value].companions[0];
            before.push(self.lower_container_into(path, source, companion, source)?);
        } else {
            let endpoint = NativeEndpoint::Value {
                value,
                prefix: Vec::new(),
            };
            let fill = self
                .native_assign_into(path, &endpoint, descriptor, source, false)
                .map_err(|error| Self::fixed_array_message_error(path, descriptor, error))?;
            before.push(fill);
        }
        before.extend(self.companions_to_element_items(value)?);
        Ok(MailboxMessage {
            before,
            value,
            after: Vec::new(),
        })
    }

    /// Receive a message for `get`/`peek`/`try_get`/`try_peek` into
    /// `destination`. Its selectors are frozen before the operation (the
    /// argument is a `ref`, Annex G.4); the copy-out runs after it.
    pub(in crate::sim::codegen) fn mailbox_message_destination(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        destination: NodeId,
    ) -> Result<MailboxMessage, String> {
        let value = self.message_temporary(path, descriptor, destination)?;
        let mut before = vec![IrStmt::NativeValueDeclare(value)];
        let mut after = self.element_items_to_companions(value)?;
        if matches!(descriptor.shape, TypeShape::Container { .. }) {
            let companion = self.model.native_values[value].companions[0];
            let target = self.container_of(destination).ok_or_else(|| {
                format!(
                    "mailbox destination of type `{}` in `{path}` must be a whole queue or dynamic-array variable",
                    descriptor.name
                )
            })?;
            after.push(IrStmt::Container(Box::new(IrContainerStmt::Copy {
                dst: target.ir,
                src: companion,
            })));
        } else if let Some(writeback) =
            self.container_record_writeback(path, destination, value, &mut before)?
        {
            after.push(writeback);
        } else {
            let (target, _) = self.native_endpoint(destination)?.ok_or_else(|| {
                Self::fixed_array_message_error(
                    path,
                    descriptor,
                    format!(
                        "mailbox destination of type `{}` in `{path}` is not a supported record variable",
                        descriptor.name
                    ),
                )
            })?;
            let source = NativeEndpoint::Value {
                value,
                prefix: Vec::new(),
            };
            after.push(self.native_transfer(path, &target, &source, false)?);
        }
        Ok(MailboxMessage {
            before,
            value,
            after,
        })
    }
}

impl Codegen<'_> {
    /// A whole unpacked-array variable has no leaf transfer to or from a
    /// message value; records (including array members) and patterns do.
    fn fixed_array_message_error(path: &str, descriptor: &TypeDescriptor, error: String) -> String {
        if matches!(descriptor.shape, TypeShape::FixedArray { .. }) {
            format!(
                "mailbox message of unpacked array type `{}` in `{path}` is supported only from an assignment pattern or into and out of record members; a whole array variable operand is not supported (SIM-017)",
                descriptor.name
            )
        } else {
            error
        }
    }

    /// The type of a mailbox operand that travels as an aggregate message:
    /// an unpacked record or union without a packed representation, an
    /// unpacked fixed array, or a queue, dynamic or associative array.
    pub(in crate::sim::codegen) fn mailbox_message_type(
        &self,
        node: NodeId,
    ) -> Option<TypeDescriptor> {
        let descriptor = self.query_descriptor(node)?;
        Self::is_mailbox_message_type(descriptor).then(|| descriptor.clone())
    }

    pub(in crate::sim::codegen) fn is_mailbox_message_type(descriptor: &TypeDescriptor) -> bool {
        match &descriptor.shape {
            TypeShape::Aggregate(layout) => {
                !matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) && !matches!(
                    lower_container_element(descriptor),
                    Ok(IrContainerElement::Packed { .. })
                )
            }
            TypeShape::FixedArray { .. } | TypeShape::Container { .. } => true,
            _ => false,
        }
    }

    /// The model-wide identity of an aggregate message type: equal for
    /// equivalent types (SV 6.22.2) and different otherwise. Unpacked
    /// structures, unions, enums and classes are nominal; arrays compare
    /// their kind, extents (not bounds), associative index and element; a
    /// queue's bound does not take part. Decided here once per site, so the
    /// runtime compares one integer.
    pub(in crate::sim::codegen) fn mailbox_message_key(&self, descriptor: &TypeDescriptor) -> u64 {
        let mut text = String::new();
        self.mailbox_type_text(descriptor, &mut text);
        // FNV-1a; zero is reserved for structural scalars.
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        hash.max(1)
    }

    fn mailbox_type_text(&self, descriptor: &TypeDescriptor, text: &mut String) {
        use std::fmt::Write;
        if self.db.enum_type_metadata(descriptor.id).is_some() {
            let _ = write!(text, "e{};", descriptor.id.0);
            return;
        }
        match &descriptor.shape {
            TypeShape::PackedAtom { .. } => {
                let _ = write!(
                    text,
                    "p{},{},{};",
                    descriptor.info.width.unwrap_or_default(),
                    u8::from(descriptor.info.signed),
                    u8::from(descriptor.two_state)
                );
            }
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) =>
            {
                let _ = write!(
                    text,
                    "p{},{},{};",
                    descriptor.info.width.unwrap_or_default(),
                    u8::from(descriptor.info.signed),
                    u8::from(descriptor.two_state)
                );
            }
            TypeShape::Aggregate(_) => {
                let _ = write!(text, "a{};", descriptor.id.0);
            }
            TypeShape::Real { shortreal } => {
                text.push_str(if *shortreal { "sr;" } else { "r;" });
            }
            TypeShape::String => text.push_str("s;"),
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                text.push('f');
                for (left, right) in dimensions {
                    let _ = write!(
                        text,
                        "{},",
                        i64::from(*left).abs_diff(i64::from(*right)) + 1
                    );
                }
                text.push('(');
                self.mailbox_type_text(element, text);
                text.push(')');
            }
            TypeShape::Container { element, array, .. } => {
                match array {
                    ArrayKind::Queue { .. } => text.push('q'),
                    ArrayKind::Dynamic => text.push('d'),
                    ArrayKind::Associative(index) => {
                        let _ = write!(text, "a[{index:?}]");
                    }
                    ArrayKind::Static => text.push('x'),
                }
                text.push('(');
                self.mailbox_type_text(element, text);
                text.push(')');
            }
            TypeShape::Opaque { kind } => {
                let _ = write!(text, "o{kind}{};", descriptor.id.0);
            }
        }
    }
}
