//! Owned VPI catalog capture (IEEE 1800-2009 clauses 36-38, Annex K/M).
//!
//! Every object the generated model exposes through `vpi_handle_by_name`,
//! `vpi_iterate` and the call-argument relations is captured here from the
//! owned database while lowering still has the frontend identities. The
//! runtime never reaches back into frontend data: names, ranges, directions
//! and parameter values are copied into [`IrVpiObject`].

use super::*;
use crate::core::db::{ConstantSource, ConstantType, Direction, NetType, TypeId, TypedefRef};
use crate::sim::ir::{
    IrVpiArgRef, IrVpiBuiltinCall, IrVpiCompileArg, IrVpiCompileCall, IrVpiConstant,
    IrVpiDirection, IrVpiObject, IrVpiObjectKind, IrVpiTypespec, IrVpiTypespecKind,
    IrVpiTypespecMember, IrVpiVarKind,
};

const SEPARATOR: &str = "\u{1f}";

/// Typedef aliases and nested types a typespec may chain through before the
/// capture gives up on naming it; deeper chains get no typespec.
const MAX_TYPESPEC_DEPTH: u32 = 256;

/// The interned typespec table under construction (IEEE 1800-2009 37.23).
/// Entries refer to earlier entries only, so equal types intern bottom-up to
/// one index.
#[derive(Default)]
struct TypespecTable {
    entries: Vec<IrVpiTypespec>,
    index: HashMap<IrVpiTypespec, usize>,
}

impl TypespecTable {
    fn intern(&mut self, typespec: IrVpiTypespec) -> usize {
        if let Some(index) = self.index.get(&typespec) {
            return *index;
        }
        let index = self.entries.len();
        self.entries.push(typespec.clone());
        self.index.insert(typespec, index);
        index
    }
}

/// The typedef reference of the next dimension level, or `None` once the
/// typedef has been passed.
fn inner_typedef(naming: Option<TypedefRef>) -> Option<TypedefRef> {
    let naming = naming?;
    Some(TypedefRef {
        typedef: naming.typedef,
        depth: naming.depth.checked_sub(1)?,
    })
}

/// The built-in integral typespec kind of a packed value without explicit
/// ranges, from its width and state domain (6.11: the integer types are the
/// only packed types without a declared range wider than one bit).
fn builtin_integral(width: u32, two_state: bool) -> IrVpiTypespecKind {
    match (two_state, width) {
        (true, 8) => IrVpiTypespecKind::Byte,
        (true, 16) => IrVpiTypespecKind::ShortInt,
        (true, 32) => IrVpiTypespecKind::Int,
        (true, 64) => IrVpiTypespecKind::LongInt,
        (false, 32) => IrVpiTypespecKind::Integer,
        (false, 64) => IrVpiTypespecKind::Time,
        (true, _) => IrVpiTypespecKind::Bit,
        (false, _) => IrVpiTypespecKind::Logic,
    }
}

/// The catalog variable kind of a value of typespec kind `kind`.
fn typespec_var_kind(kind: IrVpiTypespecKind, ranges: usize) -> IrVpiVarKind {
    match kind {
        IrVpiTypespecKind::Logic if ranges > 1 => IrVpiVarKind::PackedArray,
        IrVpiTypespecKind::Bit if ranges > 1 => IrVpiVarKind::PackedArray,
        IrVpiTypespecKind::Logic | IrVpiTypespecKind::Array => IrVpiVarKind::Logic,
        IrVpiTypespecKind::Bit => IrVpiVarKind::Bit,
        IrVpiTypespecKind::Int => IrVpiVarKind::Int,
        IrVpiTypespecKind::Integer => IrVpiVarKind::Integer,
        IrVpiTypespecKind::ShortInt => IrVpiVarKind::ShortInt,
        IrVpiTypespecKind::LongInt => IrVpiVarKind::LongInt,
        IrVpiTypespecKind::Byte => IrVpiVarKind::Byte,
        IrVpiTypespecKind::Time => IrVpiVarKind::Time,
        IrVpiTypespecKind::Real | IrVpiTypespecKind::String => IrVpiVarKind::Real,
        IrVpiTypespecKind::ShortReal => IrVpiVarKind::ShortReal,
        IrVpiTypespecKind::Enum => IrVpiVarKind::Enum,
        IrVpiTypespecKind::Struct => IrVpiVarKind::Struct,
        IrVpiTypespecKind::Union => IrVpiVarKind::Union,
    }
}

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

    /// The typespec of the declared type of `id`, named by the typedefs the
    /// declaration names (IEEE 1800-2009 37.23).
    fn vpi_declared_typespec(&self, table: &mut TypespecTable, id: NodeId) -> Option<usize> {
        let descriptor = self.db.type_descriptor(id)?;
        self.vpi_typespec(table, descriptor, self.db.declared_typedef(id), 0)
    }

    /// The typespec of `descriptor`, whose typedef (if any) `naming` gives
    /// relative to this level. A typedef at this level names the typespec;
    /// when it renames another typedef or a built-in type, the renamed type
    /// is its `vpiTypedefAlias` and shares its structure (detail 1 and 6).
    fn vpi_typespec(
        &self,
        table: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        naming: Option<TypedefRef>,
        depth: u32,
    ) -> Option<usize> {
        if depth > MAX_TYPESPEC_DEPTH {
            return None;
        }
        let Some(typedef) = naming.filter(|naming| naming.depth == 0) else {
            let typespec = self.vpi_typespec_level(table, descriptor, naming, depth)?;
            return Some(table.intern(typespec));
        };
        let renamed = self.db.declared_typedef(typedef.typedef);
        let alias = match renamed {
            Some(renamed) if renamed.depth == 0 => {
                Some(self.vpi_typespec(table, descriptor, Some(renamed), depth + 1)?)
            }
            _ if self.db.typedef_renames_builtin(typedef.typedef) => {
                Some(self.vpi_typespec(table, descriptor, None, depth + 1)?)
            }
            _ => None,
        };
        let mut typespec = match alias {
            Some(alias) => table.entries[alias].clone(),
            None => self.vpi_typespec_level(table, descriptor, renamed, depth)?,
        };
        typespec.name = Some(self.node(typedef.typedef).name.clone());
        typespec.alias = alias;
        Some(table.intern(typespec))
    }

    /// One unnamed typespec level; `naming` (if any) applies below it.
    fn vpi_typespec_level(
        &self,
        table: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        naming: Option<TypedefRef>,
        depth: u32,
    ) -> Option<IrVpiTypespec> {
        let info = &descriptor.info;
        match &descriptor.shape {
            TypeShape::PackedAtom { ranges } => {
                let ranges = ranges
                    .iter()
                    .map(|range| {
                        Some((
                            i64::try_from(range.left).ok()?,
                            i64::try_from(range.right).ok()?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                let width = info.width?;
                match info.kind.as_str() {
                    "enum" => self.vpi_enum_typespec(table, descriptor, &ranges, width),
                    "struct" | "union" | "array" | "other" => None,
                    _ => Some(self.vpi_integral_typespec(
                        table,
                        &ranges,
                        width,
                        info.signed,
                        descriptor.two_state,
                        naming,
                        depth,
                    )?),
                }
            }
            TypeShape::Real { shortreal } => Some(IrVpiTypespec {
                kind: if *shortreal {
                    IrVpiTypespecKind::ShortReal
                } else {
                    IrVpiTypespecKind::Real
                },
                ..IrVpiTypespec::default()
            }),
            TypeShape::String => Some(IrVpiTypespec {
                kind: IrVpiTypespecKind::String,
                ..IrVpiTypespec::default()
            }),
            TypeShape::Aggregate(layout) => {
                let fixed = Self::fixed_descriptor_width(descriptor);
                let mut members = Vec::with_capacity(layout.members.len());
                for (index, member) in layout.members.iter().enumerate() {
                    let member_naming = layout
                        .type_id
                        .and_then(|aggregate| self.db.member_typedef(aggregate, index));
                    let typespec =
                        self.vpi_typespec(table, &member.descriptor, member_naming, depth + 1)?;
                    let lsb = fixed.and_then(|_| {
                        Self::fixed_descriptor_path(
                            descriptor,
                            &[AggregatePathPart::Member(member.name.clone())],
                        )
                        .map(|(_, offset)| offset)
                    });
                    members.push(IrVpiTypespecMember {
                        name: member.name.clone(),
                        typespec,
                        lsb,
                        width: Self::fixed_descriptor_width(&member.descriptor).unwrap_or(0),
                    });
                }
                Some(IrVpiTypespec {
                    kind: match layout.kind {
                        AggregateKind::PackedStruct | AggregateKind::UnpackedStruct => {
                            IrVpiTypespecKind::Struct
                        }
                        _ => IrVpiTypespecKind::Union,
                    },
                    width: fixed.unwrap_or(0),
                    signed: info.signed,
                    two_state: descriptor.two_state,
                    packed: match layout.kind {
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion => Some(true),
                        AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion => Some(false),
                        AggregateKind::TaggedUnion => None,
                    },
                    tagged: layout.kind == AggregateKind::TaggedUnion,
                    members,
                    ..IrVpiTypespec::default()
                })
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let (first, rest) = dimensions.split_first()?;
                let inner = inner_typedef(naming);
                let element = if rest.is_empty() {
                    self.vpi_typespec(table, element, inner, depth + 1)?
                } else {
                    let rest = TypeDescriptor {
                        shape: TypeShape::FixedArray {
                            dimensions: rest.to_vec(),
                            element: element.clone(),
                        },
                        ..descriptor.clone()
                    };
                    self.vpi_typespec(table, &rest, inner, depth + 1)?
                };
                Some(IrVpiTypespec {
                    kind: IrVpiTypespecKind::Array,
                    element: Some(element),
                    width: Self::fixed_descriptor_width(descriptor).unwrap_or(0),
                    ranges: std::iter::once(first)
                        .chain(rest)
                        .map(|(left, right)| (i64::from(*left), i64::from(*right)))
                        .collect(),
                    ..IrVpiTypespec::default()
                })
            }
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => None,
        }
    }

    /// A logic, bit or built-in integer typespec with packed `ranges`; its
    /// element drops the leftmost range (37.23 detail 8).
    #[allow(clippy::too_many_arguments)]
    fn vpi_integral_typespec(
        &self,
        table: &mut TypespecTable,
        ranges: &[(i64, i64)],
        width: u32,
        signed: bool,
        two_state: bool,
        naming: Option<TypedefRef>,
        depth: u32,
    ) -> Option<IrVpiTypespec> {
        let Some((_, rest)) = ranges.split_first() else {
            return Some(IrVpiTypespec {
                kind: builtin_integral(width, two_state),
                width,
                signed,
                two_state,
                ..IrVpiTypespec::default()
            });
        };
        let extent = |(left, right): &(i64, i64)| left.abs_diff(*right) + 1;
        let outer = u32::try_from(extent(&ranges[0])).ok()?;
        let element_width = width.checked_div(outer)?;
        let inner = inner_typedef(naming);
        if depth > MAX_TYPESPEC_DEPTH {
            return None;
        }
        // The signedness of a packed array covers the whole array; its
        // subarrays are unsigned (7.4.1).
        let element = match inner.filter(|inner| inner.depth == 0) {
            Some(typedef) => {
                let element = TypeDescriptor {
                    id: TypeId(u64::MAX),
                    two_state,
                    name: String::new(),
                    info: crate::core::model::TypeInfo {
                        kind: if two_state { "bit" } else { "logic" }.to_owned(),
                        width: Some(element_width),
                        signed: false,
                        type_name: None,
                    },
                    shape: TypeShape::PackedAtom {
                        ranges: rest
                            .iter()
                            .map(|(left, right)| crate::core::db::PackedRange {
                                left: i128::from(*left),
                                right: i128::from(*right),
                            })
                            .collect(),
                    },
                };
                self.vpi_typespec(table, &element, Some(typedef), depth + 1)?
            }
            None => {
                let typespec = self.vpi_integral_typespec(
                    table,
                    rest,
                    element_width,
                    false,
                    two_state,
                    inner,
                    depth + 1,
                )?;
                table.intern(typespec)
            }
        };
        Some(IrVpiTypespec {
            kind: if two_state {
                IrVpiTypespecKind::Bit
            } else {
                IrVpiTypespecKind::Logic
            },
            width,
            signed,
            two_state,
            ranges: ranges.to_vec(),
            element: Some(element),
            ..IrVpiTypespec::default()
        })
    }

    /// An enum typespec with its constants and base typespec; the packed
    /// ranges of an enum descriptor are those of its base type.
    fn vpi_enum_typespec(
        &self,
        table: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        ranges: &[(i64, i64)],
        width: u32,
    ) -> Option<IrVpiTypespec> {
        let metadata = self.db.enum_type_metadata(descriptor.id)?;
        let base = self.vpi_integral_typespec(
            table,
            ranges,
            width,
            metadata.signed,
            metadata.two_state,
            None,
            0,
        )?;
        let base = table.intern(base);
        let enum_consts = metadata
            .members
            .iter()
            .map(|member| {
                let Val::Bits(bits) = &member.value else {
                    return None;
                };
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
                Some((member.name.clone(), digits))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(IrVpiTypespec {
            kind: IrVpiTypespecKind::Enum,
            base: Some(base),
            width,
            signed: metadata.signed,
            two_state: metadata.two_state,
            enum_consts,
            ..IrVpiTypespec::default()
        })
    }

    /// Catalog rows of an unpacked structure or union variable whose members
    /// keep their own leaf storage: the record row, then one row per member
    /// with a packed or real leaf and one record row per nested unpacked
    /// structure. Members llg keeps elsewhere (unpacked arrays, strings,
    /// handles, containers, and unpacked structures inside unions) are not
    /// catalog objects.
    #[allow(clippy::too_many_arguments)]
    fn vpi_record_rows(
        &self,
        table: &mut TypespecTable,
        rows: &mut Vec<IrVpiObject>,
        template: &IrVpiObject,
        descriptor: &TypeDescriptor,
        naming: Option<TypedefRef>,
        path: &[AggregatePathPart],
        leaves: &[AggregateMemberInfo],
        depth: u32,
    ) {
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return;
        };
        if depth > MAX_TYPESPEC_DEPTH {
            return;
        }
        let union = layout.kind == AggregateKind::UnpackedUnion;
        rows.push(IrVpiObject {
            kind: IrVpiObjectKind::Record,
            var_kind: if union {
                IrVpiVarKind::Union
            } else {
                IrVpiVarKind::Struct
            },
            typespec: self.vpi_typespec(table, descriptor, naming, depth),
            ..template.clone()
        });
        for (index, member) in layout.members.iter().enumerate() {
            let member_naming = layout
                .type_id
                .and_then(|aggregate| self.db.member_typedef(aggregate, index));
            let mut member_path = path.to_vec();
            member_path.push(AggregatePathPart::Member(member.name.clone()));
            let row = IrVpiObject {
                full_name: format!("{}{SEPARATOR}{}", template.full_name, member.name),
                name: member.name.clone(),
                member: true,
                ..template.clone()
            };
            let nested_record = matches!(&member.descriptor.shape,
                TypeShape::Aggregate(nested) if matches!(nested.kind,
                    AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion));
            if nested_record {
                if !union {
                    self.vpi_record_rows(
                        table,
                        rows,
                        &row,
                        &member.descriptor,
                        member_naming,
                        &member_path,
                        leaves,
                        depth + 1,
                    );
                }
                continue;
            }
            let Some(signal) = leaves
                .iter()
                .find(|leaf| leaf.path == member_path)
                .and_then(|leaf| leaf.signal.as_ref())
            else {
                continue;
            };
            let typespec = self.vpi_typespec(table, &member.descriptor, member_naming, depth + 1);
            let real = signal.real;
            let width = if real {
                0
            } else {
                Self::fixed_descriptor_width(&member.descriptor).unwrap_or(signal.width)
            };
            let kind = typespec.map(|typespec| table.entries[typespec].kind);
            let ranges = typespec.map_or(0, |typespec| table.entries[typespec].ranges.len());
            rows.push(IrVpiObject {
                kind: if real {
                    IrVpiObjectKind::RealVar
                } else {
                    IrVpiObjectKind::Reg
                },
                width,
                signed: member.descriptor.info.signed,
                real,
                signal: Some(signal.ir),
                var_kind: kind.map_or_else(
                    || IrVpiVarKind::from_type_kind(&member.ty.kind, member.packed_ranges.len()),
                    |kind| typespec_var_kind(kind, ranges),
                ),
                packed_range: (!real)
                    .then(|| {
                        member
                            .packed_ranges
                            .first()
                            .and_then(|range| {
                                Some((
                                    i64::try_from(range.left).ok()?,
                                    i64::try_from(range.right).ok()?,
                                ))
                            })
                            .or_else(|| (width > 1).then(|| (i64::from(width) - 1, 0)))
                    })
                    .flatten(),
                two_state: member.two_state,
                typespec,
                slice: !real && width < signal.width,
                ..row
            });
        }
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
        let mut typespecs = TypespecTable::default();
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
                        typespec: self.vpi_declared_typespec(&mut typespecs, id),
                        ..IrVpiObject::default()
                    }));
                }
                // An unpacked structure or union kept as member leaves
                // (procedural-block records are not catalog objects).
                NodeKind::Var { .. }
                    if self
                        .unpacked_aggregates
                        .get(&id)
                        .is_some_and(|info| !info.columns)
                        && !self.block_records.contains_key(&id) =>
                {
                    let Some(descriptor) = self.db.type_descriptor(id) else {
                        continue;
                    };
                    let template = source(IrVpiObject {
                        full_name: self.vpi_full_name(id),
                        name: node.name.clone(),
                        ..IrVpiObject::default()
                    });
                    let mut rows = Vec::new();
                    self.vpi_record_rows(
                        &mut typespecs,
                        &mut rows,
                        &template,
                        descriptor,
                        self.db.declared_typedef(id),
                        &[],
                        &self.unpacked_aggregates[&id].leaves,
                        0,
                    );
                    for row in rows {
                        insert(row);
                    }
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
                        typespec: self.vpi_declared_typespec(&mut typespecs, id),
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
                        typespec: self.vpi_declared_typespec(&mut typespecs, id),
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::Port {
                    direction, low, ty, ..
                } => {
                    // The components of a multi-port are not module ports of
                    // their own, and an unnamed port (a select or concatenation
                    // port expression without `.name(...)`, SV 23.2.2.1) has no
                    // name to publish; it still takes its place in the order.
                    let Some(parent) = node.parent else {
                        continue;
                    };
                    let is_module_port = |port: NodeId| {
                        matches!(self.kind(port), NodeKind::Port { .. })
                            && self.db.multi_port_of(port).is_none()
                    };
                    if node.name.is_empty() || !is_module_port(id) {
                        continue;
                    }
                    let port_index = self
                        .node(parent)
                        .children
                        .iter()
                        .take_while(|child| **child != id)
                        .filter(|child| is_module_port(**child))
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
        model.vpi_typespecs = typespecs.entries;
        Ok(())
    }

    /// Record every built-in system subroutine the design calls and llg runs
    /// natively, with its first call site in source order, so a VPI
    /// registration under its name stops the run at startup instead of being
    /// silently ignored (IEEE 1800-2009 36.3.2, decision S99-D6). Names with
    /// a VPI call site (user system tasks and built-ins without a lowering)
    /// are dispatched to the registration and are left out.
    pub(in super::super) fn populate_vpi_builtin_calls(&self, model: &mut IrModel) {
        use std::collections::BTreeMap;
        let dispatched: HashSet<&str> = model
            .vpi_compile_calls
            .iter()
            .map(|call| call.name.as_str())
            .collect();
        let mut first: BTreeMap<&str, (Option<&str>, u32, u32)> = BTreeMap::new();
        for id in self.db.node_ids() {
            let NodeKind::SysCall { name } = self.kind(id) else {
                continue;
            };
            // Built-in methods (`constraint_mode`) share the node kind; only
            // `$` names can be registered as system tasks and functions.
            if !name.starts_with('$') || dispatched.contains(name.as_str()) {
                continue;
            }
            let node = self.node(id);
            let site = (node.file(), node.line, node.col);
            first
                .entry(name.as_str())
                .and_modify(|known| *known = (*known).min(site))
                .or_insert(site);
        }
        model.vpi_builtin_calls = first
            .into_iter()
            .map(|(name, (file, line, _))| IrVpiBuiltinCall {
                name: name.to_owned(),
                file: file.map(str::to_owned),
                line,
            })
            .collect();
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

    /// Lower one actual of a VPI system task or function call (`routine` is
    /// "task" or "function"). A scope actual (`$list(tb)`)
    /// passes its catalog object, so the value is a placeholder; a string
    /// literal converted to a `string` formal (`$input("cmds")`) passes its
    /// packed image, which `vpi_get_value(vpiStringVal)` reads as the text.
    /// A string variable or expression has no fixed-width image and stays a
    /// located error.
    pub(in super::super) fn lower_vpi_argument(
        &mut self,
        path: &str,
        name: &str,
        routine: &str,
        arg: NodeId,
    ) -> Result<IrExpr, String> {
        let located = |this: &Self, error: String| {
            format!(
                "{}: argument of VPI system {routine} `{name}`: {error}",
                this.source_location(arg)
            )
        };
        if let NodeKind::Expr(ExprKind::ScopeRef { target }) = self.kind(arg) {
            let target = *target;
            if self.vpi_scope_object(target) {
                return Ok(const_zero_expr(1));
            }
            return Err(located(
                self,
                format!(
                    "scope `{}` has no VPI object; llg passes module, interface and named \
                     generate scopes",
                    self.node(target).name
                ),
            ));
        }
        let string_typed = self
            .query_descriptor(arg)
            .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::String));
        if string_typed {
            let literal = self.vpi_strip_conversions(arg);
            if literal != arg {
                return self
                    .lower_expr(path, literal)
                    .map_err(|error| located(self, error));
            }
            return Err(located(
                self,
                format!(
                    "a string variable or expression is not supported (only string literals \
                     reach VPI applications) in `{path}`"
                ),
            ));
        }
        self.lower_expr(path, arg)
            .map_err(|error| located(self, error))
    }

    /// Whether scope `target` is a catalog object a VPI argument can name.
    fn vpi_scope_object(&self, target: NodeId) -> bool {
        match self.kind(target) {
            NodeKind::ModuleInst { .. } => true,
            NodeKind::GenScope => !self.node(target).name.is_empty(),
            _ => false,
        }
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
            NodeKind::Expr(ExprKind::ScopeRef { target }) => {
                IrVpiArgRef::Object(self.vpi_full_name(*target))
            }
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
    fn integral_types_without_ranges_map_to_the_builtin_typespecs() {
        use super::builtin_integral;
        use crate::sim::ir::IrVpiTypespecKind as Kind;
        assert_eq!(builtin_integral(8, true), Kind::Byte);
        assert_eq!(builtin_integral(16, true), Kind::ShortInt);
        assert_eq!(builtin_integral(32, true), Kind::Int);
        assert_eq!(builtin_integral(64, true), Kind::LongInt);
        assert_eq!(builtin_integral(32, false), Kind::Integer);
        assert_eq!(builtin_integral(64, false), Kind::Time);
        assert_eq!(builtin_integral(1, true), Kind::Bit);
        assert_eq!(builtin_integral(1, false), Kind::Logic);
    }

    #[test]
    fn typedef_depth_steps_down_one_dimension_per_level() {
        use super::inner_typedef;
        use crate::core::db::{NodeId, TypedefRef};
        let typedef = NodeId::from_index(7);
        let at = |depth| Some(TypedefRef { typedef, depth });
        assert_eq!(inner_typedef(at(2)), at(1));
        assert_eq!(inner_typedef(at(1)), at(0));
        assert_eq!(inner_typedef(at(0)), None);
        assert_eq!(inner_typedef(None), None);
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
