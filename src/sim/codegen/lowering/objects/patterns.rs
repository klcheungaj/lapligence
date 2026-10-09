//! `%p`/`%0p` arguments (SV 21.2.1.7).
//!
//! A pattern argument keeps its value's own storage: a packed scalar or an
//! aggregate in its packed transport, a whole container, or a handle. A
//! record with string, real, handle or container leaves has no single
//! storage; its pattern is a format template whose holes are the record's
//! leaves, read through the ordinary record leaf reads. The display type of
//! each value is interned in `IrModel::pattern_types`, children first.
use super::*;
use crate::sim::codegen::lowering::collection::aggregate_path_suffix;
use crate::sim::codegen::lowering::collection::native_values::LeafValue;
use crate::sim::ir::{
    IrPattern, IrPatternClass, IrPatternEnumMember, IrPatternField, IrPatternMember,
    IrPatternSource, IrPatternStorage, IrPatternType,
};

impl Codegen<'_> {
    /// Lower the argument of a `%p`/`%0p` conversion. Strings, reals and
    /// plain packed values keep the scalar formatter; every other value is
    /// rendered by the pattern walker into conversion text.
    pub(in super::super) fn lower_pattern_arg(
        &mut self,
        path: &str,
        node: NodeId,
        abbreviated: bool,
    ) -> Result<IrDisplayArg, String> {
        let Some(descriptor) = self.query_descriptor(node).cloned() else {
            return self.lower_format_arg(path, node);
        };
        match &descriptor.shape {
            TypeShape::String | TypeShape::Real { .. } => return self.lower_format_arg(path, node),
            TypeShape::PackedAtom { .. } if self.enum_metadata_for_expr(node).is_none() => {
                return self.lower_format_arg(path, node)
            }
            _ => {}
        }
        let unsupported = |error: String| {
            format!(
                "%p of a value of type `{}` in `{path}` is not supported: {error}",
                descriptor.name
            )
        };
        if let TypeShape::Opaque { kind } = &descriptor.shape {
            if kind == "Event" {
                // An event is an identity with no printable state.
                return Ok(IrDisplayArg::Text(IrStringExpr::Literal(b"event".to_vec())));
            }
        }
        let ty = self
            .pattern_type_for_expr(node, &descriptor)
            .map_err(unsupported)?;
        let pattern = |source| {
            IrDisplayArg::Text(IrStringExpr::Pattern(Box::new(IrPattern {
                source,
                ty,
                abbreviated,
            })))
        };
        match &descriptor.shape {
            TypeShape::Opaque { .. } => {
                let handle = self.lower_chandle(path, node).map_err(unsupported)?;
                return Ok(pattern(IrPatternSource::Handle(handle)));
            }
            TypeShape::Container { .. } => {
                let container = self.container_of(node).ok_or_else(|| {
                    unsupported(
                        "only a whole queue, dynamic or associative array variable can be printed"
                            .to_owned(),
                    )
                })?;
                return Ok(pattern(IrPatternSource::Container(container.ir)));
            }
            _ => {}
        }
        // A fixed array of native elements is container storage.
        if matches!(descriptor.shape, TypeShape::FixedArray { .. }) {
            if let Some(container) = self.container_of(node) {
                return Ok(pattern(IrPatternSource::Container(container.ir)));
            }
        }
        let width = self.model.pattern_types[ty].width();
        if width != 0 {
            if let Ok(value) = self.lower_expr(path, node) {
                if !value.is_real() && value.width == width {
                    return Ok(pattern(IrPatternSource::Packed(value)));
                }
            }
        }
        if let Some(array) = self.whole_real_array(node) {
            return self
                .real_array_pattern(path, &array, abbreviated)
                .map_err(unsupported);
        }
        if let TypeShape::Aggregate(_) | TypeShape::FixedArray { .. } = &descriptor.shape {
            let mut setup = Vec::new();
            if let Some(reads) = self.record_leaf_reads(path, node, &mut setup)? {
                if setup.is_empty() {
                    return self
                        .record_pattern(path, &descriptor, reads, abbreviated)
                        .map_err(unsupported);
                }
            }
        }
        Err(unsupported(
            "its storage cannot be read as one value here".to_owned(),
        ))
    }

    /// A whole real fixed array, which keeps `double` cells.
    fn whole_real_array(&self, node: NodeId) -> Option<ArrayInfo> {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            _ => node,
        };
        self.array_globals
            .get(&target)
            .filter(|array| array.real && self.model.arrays[array.ir].real)
            .cloned()
    }

    /// A real fixed array: a template with one `%p` hole per element, read
    /// by its declared indices.
    fn real_array_pattern(
        &mut self,
        path: &str,
        array: &ArrayInfo,
        abbreviated: bool,
    ) -> Result<IrDisplayArg, String> {
        let total = array.dims.iter().try_fold(1u64, |total, (left, right)| {
            total.checked_mul(u64::from(left.abs_diff(*right)) + 1)
        });
        if total.is_none_or(|total| total > PATTERN_TEMPLATE_ELEMENT_LIMIT) {
            return Err(format!(
                "a real array pattern prints at most {PATTERN_TEMPLATE_ELEMENT_LIMIT} elements"
            ));
        }
        let arr = self.reference_array(array.ir);
        let mut format = Vec::new();
        let mut args = Vec::new();
        let mut indices = Vec::new();
        real_array_template(
            arr,
            &array.dims,
            &mut indices,
            abbreviated,
            &mut format,
            &mut args,
        );
        Ok(IrDisplayArg::Text(IrStringExpr::Format {
            format: Box::new(IrStringExpr::Literal(format)),
            args,
            scope: self.display_path(path).to_owned(),
        }))
    }

    /// A record with native leaves: a template with one `%p` hole per leaf.
    fn record_pattern(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        reads: Vec<(Vec<AggregatePathPart>, LeafValue)>,
        abbreviated: bool,
    ) -> Result<IrDisplayArg, String> {
        let mut leaves: HashMap<Vec<AggregatePathPart>, LeafValue> = reads.into_iter().collect();
        let mut format = Vec::new();
        let mut args = Vec::new();
        let mut leaf_path = Vec::new();
        self.record_template(
            descriptor,
            &mut leaf_path,
            &mut leaves,
            abbreviated,
            &mut format,
            &mut args,
        )?;
        Ok(IrDisplayArg::Text(IrStringExpr::Format {
            format: Box::new(IrStringExpr::Literal(format)),
            args,
            scope: self.display_path(path).to_owned(),
        }))
    }

    fn record_template(
        &mut self,
        descriptor: &TypeDescriptor,
        path: &mut Vec<AggregatePathPart>,
        leaves: &mut HashMap<Vec<AggregatePathPart>, LeafValue>,
        abbreviated: bool,
        format: &mut Vec<u8>,
        args: &mut Vec<IrDisplayArg>,
    ) -> Result<(), String> {
        let separator: &[u8] = if abbreviated { b"," } else { b", " };
        if let Some(leaf) = leaves.remove(path.as_slice()) {
            format.extend_from_slice(if abbreviated { b"%0p" } else { b"%p" });
            let needs_type = self.pattern_needs_type(descriptor);
            let typed = |codegen: &mut Self, source| -> Result<IrDisplayArg, String> {
                let ty = codegen.pattern_type(descriptor)?;
                Ok(IrDisplayArg::Text(IrStringExpr::Pattern(Box::new(
                    IrPattern {
                        source,
                        ty,
                        abbreviated,
                    },
                ))))
            };
            args.push(match leaf {
                LeafValue::Packed(value) if needs_type => {
                    typed(self, IrPatternSource::Packed(value))?
                }
                LeafValue::Packed(value) => IrDisplayArg::Packed(value),
                LeafValue::Real(value) => IrDisplayArg::Real(value),
                LeafValue::String(value) => IrDisplayArg::String(value),
                LeafValue::Chandle(value) => {
                    if matches!(&descriptor.shape, TypeShape::Opaque { kind } if kind == "Event") {
                        IrDisplayArg::Text(IrStringExpr::Literal(b"event".to_vec()))
                    } else {
                        typed(self, IrPatternSource::Handle(value))?
                    }
                }
                LeafValue::Container(container) => {
                    typed(self, IrPatternSource::Container(container))?
                }
            });
            return Ok(());
        }
        match &descriptor.shape {
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion
                ) =>
            {
                let union = layout.kind == AggregateKind::UnpackedUnion;
                format.extend_from_slice(b"'{");
                for (index, member) in layout.members.iter().enumerate() {
                    if union && index > 0 {
                        break;
                    }
                    if index > 0 {
                        format.extend_from_slice(separator);
                    }
                    if !abbreviated {
                        format.extend_from_slice(member.name.as_bytes());
                        format.push(b':');
                    }
                    path.push(AggregatePathPart::Member(member.name.clone()));
                    let result = self.record_template(
                        &member.descriptor,
                        path,
                        leaves,
                        abbreviated,
                        format,
                        args,
                    );
                    path.pop();
                    result?;
                }
                format.push(b'}');
                Ok(())
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } => self.record_array_template(
                dimensions,
                element,
                path,
                leaves,
                abbreviated,
                format,
                args,
            ),
            _ => Err(format!(
                "record leaf `{}` of type `{}` has no readable value",
                aggregate_path_suffix(path),
                descriptor.name
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_array_template(
        &mut self,
        dimensions: &[(i32, i32)],
        element: &TypeDescriptor,
        path: &mut Vec<AggregatePathPart>,
        leaves: &mut HashMap<Vec<AggregatePathPart>, LeafValue>,
        abbreviated: bool,
        format: &mut Vec<u8>,
        args: &mut Vec<IrDisplayArg>,
    ) -> Result<(), String> {
        let Some(((left, right), inner)) = dimensions.split_first() else {
            return self.record_template(element, path, leaves, abbreviated, format, args);
        };
        format.extend_from_slice(b"'{");
        let step: i32 = if left <= right { 1 } else { -1 };
        let mut index = *left;
        let mut first = true;
        loop {
            if !first {
                format.extend_from_slice(if abbreviated { b"," } else { b", " });
            }
            first = false;
            path.push(AggregatePathPart::Index(index));
            let result =
                self.record_array_template(inner, element, path, leaves, abbreviated, format, args);
            path.pop();
            result?;
            if index == *right {
                break;
            }
            index += step;
        }
        format.push(b'}');
        Ok(())
    }

    /// Whether a packed leaf needs the walker (enum names, packed or
    /// flattened members) rather than the scalar formatter.
    fn pattern_needs_type(&self, descriptor: &TypeDescriptor) -> bool {
        match &descriptor.shape {
            TypeShape::PackedAtom { .. } => self.db.enum_type_metadata(descriptor.id).is_some(),
            TypeShape::Real { .. } | TypeShape::String => false,
            _ => true,
        }
    }

    /// Display type of `node`, using the expression's enum metadata (which
    /// also covers built-in enumerations).
    fn pattern_type_for_expr(
        &mut self,
        node: NodeId,
        descriptor: &TypeDescriptor,
    ) -> Result<usize, String> {
        if let TypeShape::PackedAtom { .. } = descriptor.shape {
            if let Some(metadata) = self.enum_metadata_for_expr(node).cloned() {
                let ty = enum_pattern_type(descriptor, &metadata)?;
                return Ok(self.intern_pattern_type(ty));
            }
        }
        self.pattern_type(descriptor)
    }

    /// Intern the display type of `descriptor` and its children.
    pub(in super::super) fn pattern_type(
        &mut self,
        descriptor: &TypeDescriptor,
    ) -> Result<usize, String> {
        let flat = descriptor.info.width.unwrap_or(0);
        let members = |codegen: &mut Self, layout: &AggregateLayout| {
            layout
                .members
                .iter()
                .map(|member| {
                    Ok(IrPatternMember {
                        name: member.name.clone(),
                        ty: codegen.pattern_type(&member.descriptor)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        };
        let ty = match &descriptor.shape {
            TypeShape::PackedAtom { .. } => match self.db.enum_type_metadata(descriptor.id) {
                Some(metadata) => enum_pattern_type(descriptor, &metadata.clone())?,
                None => IrPatternType::Packed {
                    width: flat,
                    signed: descriptor.info.signed,
                    enum_members: Vec::new(),
                },
            },
            TypeShape::Real { shortreal } => IrPatternType::Real {
                shortreal: *shortreal,
                width: if *shortreal { 32 } else { 64 },
            },
            TypeShape::String => IrPatternType::String,
            TypeShape::Aggregate(layout) => match layout.kind {
                AggregateKind::PackedStruct => IrPatternType::PackedStruct {
                    width: flat,
                    members: members(self, layout)?,
                },
                // A packed union prints its first member, which spans the
                // whole union (SV 21.2.1.7).
                AggregateKind::PackedUnion => match layout.members.first() {
                    Some(member) => return self.pattern_type(&member.descriptor),
                    None => IrPatternType::Packed {
                        width: flat,
                        signed: false,
                        enum_members: Vec::new(),
                    },
                },
                AggregateKind::UnpackedStruct => {
                    let members = members(self, layout)?;
                    let widths = self.member_widths(&members);
                    IrPatternType::Struct {
                        width: widths
                            .and_then(|widths| {
                                widths
                                    .iter()
                                    .try_fold(0u32, |sum, width| sum.checked_add(*width))
                            })
                            .unwrap_or(0),
                        members,
                    }
                }
                AggregateKind::UnpackedUnion => {
                    let members = members(self, layout)?;
                    let widths = self.member_widths(&members);
                    IrPatternType::Union {
                        width: widths
                            .and_then(|widths| widths.into_iter().max())
                            .unwrap_or(0),
                        members,
                    }
                }
                AggregateKind::TaggedUnion => {
                    return Err("tagged unions have no pattern form yet".to_owned())
                }
            },
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let element = self.pattern_type(element)?;
                // The packed transport of a fixed array is its elements'
                // transports, first element most significant.
                let element_width = u64::from(self.model.pattern_types[element].width());
                let width = dimensions
                    .iter()
                    .try_fold(element_width, |total, (left, right)| {
                        total.checked_mul(u64::from(left.abs_diff(*right)) + 1)
                    })
                    .and_then(|width| u32::try_from(width).ok())
                    .unwrap_or(0);
                IrPatternType::FixedArray {
                    width,
                    bounds: dimensions.clone(),
                    element,
                }
            }
            TypeShape::Container { element, array, .. } => {
                let element = self.pattern_type(element)?;
                match array {
                    ArrayKind::Queue { .. } => IrPatternType::Queue { element },
                    ArrayKind::Dynamic => IrPatternType::Dynamic { element },
                    ArrayKind::Associative(_) => IrPatternType::Associative { element },
                    ArrayKind::Static => {
                        return Err("static container metadata has no pattern form".to_owned())
                    }
                }
            }
            TypeShape::Opaque { kind } => match kind.as_str() {
                "Class" => match descriptor.name.as_str() {
                    "process" => IrPatternType::Process,
                    name if name == "semaphore" || name.starts_with("mailbox") => {
                        return Err(
                            "built-in semaphore and mailbox handles have no pattern form yet"
                                .to_owned(),
                        )
                    }
                    _ => {
                        self.ensure_pattern_classes()?;
                        IrPatternType::Class
                    }
                },
                "Chandle" => IrPatternType::Chandle,
                "Event" => IrPatternType::Event,
                "VirtualInterface" => IrPatternType::VirtualInterface,
                other => return Err(format!("{other} values have no pattern form")),
            },
        };
        Ok(self.intern_pattern_type(ty))
    }

    /// Packed transport widths of every member, if each has one.
    fn member_widths(&self, members: &[IrPatternMember]) -> Option<Vec<u32>> {
        members
            .iter()
            .map(|member| {
                let width = self.model.pattern_types[member.ty].width();
                (width != 0).then_some(width)
            })
            .collect()
    }

    fn intern_pattern_type(&mut self, ty: IrPatternType) -> usize {
        if let Some(index) = self
            .model
            .pattern_types
            .iter()
            .position(|known| *known == ty)
        {
            return index;
        }
        self.model.pattern_types.push(ty);
        self.model.pattern_types.len() - 1
    }

    /// Build the display layout of every class once, the first time a
    /// pattern can reach a class object. Class layouts are complete before
    /// any statement is lowered.
    fn ensure_pattern_classes(&mut self) -> Result<(), String> {
        if !self.model.pattern_classes.is_empty() || self.pattern_classes_started {
            return Ok(());
        }
        // Class properties may themselves be class handles; the flag stops
        // the recursion while the tables are built.
        self.pattern_classes_started = true;
        let mut classes = vec![None; self.model.classes.len()];
        let mut nodes: Vec<(NodeId, usize)> = self
            .class_nodes
            .iter()
            .map(|(node, index)| (*node, *index))
            .collect();
        nodes.sort_by_key(|(_, index)| *index);
        let mut own: Vec<Vec<(usize, IrPatternField)>> = vec![Vec::new(); classes.len()];
        for (class, class_index) in nodes {
            for child in self.node(class).children.clone() {
                let Some(field) = self.class_field_index(child) else {
                    continue;
                };
                if field.0 != class_index {
                    continue;
                }
                let Some(descriptor) = self.db.type_descriptor(child).cloned() else {
                    return Err(format!(
                        "class property `{}` has no type metadata",
                        self.node(child).full_name
                    ));
                };
                let storage = self.class_field_storage(class_index, field.1, &descriptor);
                let ty = match &descriptor.shape {
                    // A semaphore or mailbox property prints as a handle.
                    TypeShape::Opaque { kind }
                        if kind == "Class"
                            && (descriptor.name == "semaphore"
                                || descriptor.name.starts_with("mailbox")) =>
                    {
                        self.intern_pattern_type(IrPatternType::Chandle)
                    }
                    _ => self.pattern_type(&descriptor)?,
                };
                own[class_index].push((
                    field.1,
                    IrPatternField {
                        name: self.node(child).name.clone(),
                        ty,
                        storage,
                    },
                ));
            }
        }
        for index in 0..classes.len() {
            self.pattern_class_layout(index, &mut own, &mut classes)?;
        }
        self.model.pattern_classes = classes
            .into_iter()
            .map(|class| class.unwrap_or(IrPatternClass { fields: Vec::new() }))
            .collect();
        Ok(())
    }

    fn pattern_class_layout(
        &self,
        index: usize,
        own: &mut [Vec<(usize, IrPatternField)>],
        classes: &mut [Option<IrPatternClass>],
    ) -> Result<(), String> {
        if classes[index].is_some() {
            return Ok(());
        }
        let mut fields = match self.model.classes[index].base {
            Some(base) => {
                self.pattern_class_layout(base, own, classes)?;
                classes[base]
                    .clone()
                    .map(|class| class.fields)
                    .unwrap_or_default()
            }
            None => Vec::new(),
        };
        let mut mine = std::mem::take(&mut own[index]);
        mine.sort_by_key(|(field, _)| *field);
        for (field, entry) in mine {
            if field != fields.len() {
                return Err(format!(
                    "class layout {index} has no display entry for field {}",
                    fields.len()
                ));
            }
            fields.push(entry);
        }
        if fields.len() != self.model.classes[index].fields.len() {
            return Err(format!(
                "class layout {index} has {} fields but {} display entries",
                self.model.classes[index].fields.len(),
                fields.len()
            ));
        }
        classes[index] = Some(IrPatternClass { fields });
        Ok(())
    }

    /// The (class, field) slot of an instance property declaration.
    fn class_field_index(&self, child: NodeId) -> Option<(usize, usize)> {
        if let Some(slot) = self.class_fields.get(&child) {
            return Some(*slot);
        }
        if let Some(value) = self.class_native_fields.get(&child) {
            return self.model.native_values[*value].class_field;
        }
        self.container_declaration(child)
            .and_then(|info| self.model.containers[info.ir].class_field)
    }

    fn class_field_storage(
        &self,
        class: usize,
        field: usize,
        descriptor: &TypeDescriptor,
    ) -> IrPatternStorage {
        let slot = &self.model.classes[class].fields[field];
        if let Some(container) = slot.container {
            return if matches!(
                self.model.containers[container].element,
                crate::sim::ir::IrContainerElement::Packed { .. }
            ) {
                IrPatternStorage::PackedContainer
            } else {
                IrPatternStorage::ValueContainer
            };
        }
        if slot.native_value.is_some() {
            return IrPatternStorage::Value;
        }
        match slot.ty {
            IrClassFieldType::Packed { .. } => IrPatternStorage::Packed,
            IrClassFieldType::Real { .. } => IrPatternStorage::Real,
            IrClassFieldType::String => IrPatternStorage::String,
            IrClassFieldType::Chandle => {
                let _ = descriptor;
                IrPatternStorage::Handle
            }
        }
    }
}

/// Largest fixed array printed through a per-element format template; such
/// arrays have no single storage the pattern walker can read.
const PATTERN_TEMPLATE_ELEMENT_LIMIT: u64 = 1 << 16;

fn real_array_template(
    arr: usize,
    dims: &[(i32, i32)],
    indices: &mut Vec<IrExpr>,
    abbreviated: bool,
    format: &mut Vec<u8>,
    args: &mut Vec<IrDisplayArg>,
) {
    let Some(((left, right), inner)) = dims.split_first() else {
        format.extend_from_slice(if abbreviated { b"%0p" } else { b"%p" });
        args.push(IrDisplayArg::Real(IrExpr::new(
            IrExprKind::ArrayRead {
                arr,
                indices: indices.clone(),
                elem_sel: IrElemSel::Whole,
            },
            0,
            false,
            None,
        )));
        return;
    };
    format.extend_from_slice(b"'{");
    let step: i64 = if left <= right { 1 } else { -1 };
    let mut index = i64::from(*left);
    loop {
        if index != i64::from(*left) {
            format.extend_from_slice(if abbreviated { b"," } else { b", " });
        }
        indices.push(crate::sim::codegen::lowering::containers::pattern_key_expr(
            i128::from(index),
            32,
            true,
            true,
        ));
        real_array_template(arr, inner, indices, abbreviated, format, args);
        indices.pop();
        if index == i64::from(*right) {
            break;
        }
        index += step;
    }
    format.push(b'}');
}

fn enum_pattern_type(
    descriptor: &TypeDescriptor,
    metadata: &crate::core::db::EnumTypeMetadata,
) -> Result<IrPatternType, String> {
    let mut enum_members = Vec::with_capacity(metadata.members.len());
    for member in &metadata.members {
        let Val::Bits(value) = &member.value else {
            return Err(format!(
                "enum member {} has a non-integral value",
                member.name
            ));
        };
        let value = val_to_const(value)?;
        if value.width != metadata.width {
            return Err(format!(
                "enum member {} value width does not match its enum type",
                member.name
            ));
        }
        // Every plane spans the value's limbs; the constant may elide
        // all-zero X/Z planes.
        let limbs = metadata.width.div_ceil(64) as usize;
        let plane = |words: &[u64]| {
            let mut words = words.to_vec();
            words.resize(limbs, 0);
            words
        };
        enum_members.push(IrPatternEnumMember {
            name: member.name.clone(),
            bits: plane(&value.bits),
            x: plane(&value.x),
            z: plane(&value.z),
        });
    }
    let _ = descriptor;
    Ok(IrPatternType::Packed {
        width: metadata.width,
        signed: metadata.signed,
        enum_members,
    })
}
