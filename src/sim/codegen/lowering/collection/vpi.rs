//! Owned VPI catalog capture (IEEE 1800-2009 clauses 36-38, Annex K/M).
//!
//! Every object the generated model exposes through `vpi_handle_by_name`,
//! `vpi_iterate` and the call-argument relations is captured here from the
//! owned database while lowering still has the frontend identities. The
//! runtime never reaches back into frontend data: names, ranges, directions
//! and parameter values are copied into [`IrVpiObject`].

use super::*;
use crate::core::db::{
    ConstantSource, ConstantType, Direction, NetType, Strength, TypeId, TypedefRef,
};
use crate::sim::ir::{
    IrSdfTarget, IrVpiArgRef, IrVpiBuiltinCall, IrVpiCompileArg, IrVpiCompileCall, IrVpiConn,
    IrVpiConstant, IrVpiDirection, IrVpiInstance, IrVpiObject, IrVpiObjectKind, IrVpiQualifiers,
    IrVpiTiming, IrVpiTypespec, IrVpiTypespecKind, IrVpiTypespecMember, IrVpiUdp, IrVpiVarKind,
};

mod code;
mod declarations;
mod dynamic;
mod scopes;
mod timing;
use scopes::ScopeStatement;
pub(in super::super) use scopes::VpiScopeIndex;
pub(in super::super) use timing::VpiTimingMetadata;

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

/// `vpiFuncType` of a function returning `ret` (37.37); `None` for a task
/// or a void function.
fn vpi_func_type(
    is_task: bool,
    ret: Option<&crate::core::model::TypeInfo>,
) -> Option<&'static str> {
    let ret = ret.filter(|_| !is_task)?;
    Some(match ret.kind.as_str() {
        "integer" => "vpiIntFunc",
        "real" | "shortreal" | "realtime" => "vpiRealFunc",
        "time" => "vpiTimeFunc",
        _ if ret.width.is_none() => "vpiOtherFunc",
        _ if ret.signed => "vpiSizedSignedFunc",
        _ => "vpiSizedFunc",
    })
}

/// The VPI value of an elaborated parameter: `(constant, width, signed,
/// real)`, or `None` for a value the catalog cannot hold.
fn vpi_parameter_constant(value: &Val) -> Option<(IrVpiConstant, u32, bool, bool)> {
    match value {
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
                return None;
            }
            Some((IrVpiConstant::Bits(digits), width, bits.signed, false))
        }
        Val::Real(real) => Some((IrVpiConstant::Real(*real), 0, false, true)),
        Val::Str(text) => {
            if text.contains('\0') {
                return None;
            }
            let width = u32::try_from(text.len().saturating_mul(8))
                .unwrap_or(u32::MAX)
                .min(LLG_MAX_WIDTH);
            Some((IrVpiConstant::Text(text.clone()), width, false, false))
        }
    }
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
        IrVpiTypespecKind::Real | IrVpiTypespecKind::String | IrVpiTypespecKind::Chandle => {
            IrVpiVarKind::Real
        }
        IrVpiTypespecKind::ShortReal => IrVpiVarKind::ShortReal,
        IrVpiTypespecKind::Enum => IrVpiVarKind::Enum,
        IrVpiTypespecKind::Struct => IrVpiVarKind::Struct,
        IrVpiTypespecKind::Union => IrVpiVarKind::Union,
        IrVpiTypespecKind::PackedArray => IrVpiVarKind::PackedArray,
    }
}

impl Codegen<'_> {
    /// The unit-separated hierarchical path of a scope or declaration, with
    /// generate blocks named per SV 27.4 and named statement blocks included.
    fn vpi_path(&self, id: NodeId) -> Vec<String> {
        // A package or `$unit` keys its objects apart from a module of the
        // same name: its key part ends in `::`, the separator its members'
        // full names take (37.10 detail 5).
        if let Some(name) = self.vpi_package_name(id) {
            return vec![format!("{name}::")];
        }
        match self.kind(id) {
            NodeKind::ModuleInst { .. } | NodeKind::GenScope | NodeKind::GenScopeArray => self
                .db
                .hierarchical_name_parts(id)
                .into_iter()
                .map(str::to_owned)
                .collect(),
            _ => {
                // A block statement is named by its declaration scope, which
                // holds what the block declares (decision A19-D27).
                if let Some(scope) = self.vpi_block_scope(id) {
                    return self.vpi_path(scope);
                }
                let node = self.node(id);
                let mut path = node
                    .parent
                    .map_or_else(Vec::new, |parent| self.vpi_path(parent));
                if let Some(name) = self.vpi_unnamed_scope_name(id) {
                    path.push(name.to_owned());
                    return path;
                }
                let named = matches!(
                    self.kind(id),
                    NodeKind::Stmt(StmtKind::Begin | StmtKind::Fork { .. })
                        | NodeKind::Var { .. }
                        | NodeKind::Net { .. }
                        | NodeKind::Array { .. }
                        | NodeKind::Param { .. }
                        | NodeKind::Port { .. }
                        | NodeKind::FuncTask { .. }
                        | NodeKind::NamedEvent
                ) && !node.name.is_empty();
                if named {
                    path.push(node.name.clone());
                }
                path
            }
        }
    }

    /// The name of a package (`vpiPackage`) or of the compilation unit
    /// (`$unit`, 37.10), or `None` for any other node.
    fn vpi_package_name(&self, id: NodeId) -> Option<String> {
        match self.kind(id) {
            NodeKind::Package => Some(strip_lib(&self.node(id).name)),
            _ if self.is_compilation_unit(id) => Some("$unit".to_owned()),
            _ => None,
        }
    }

    pub(in crate::sim::codegen::lowering) fn vpi_full_name(&self, id: NodeId) -> String {
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
                if let Some(element) = self.db.packed_array_element(descriptor.id) {
                    // One range per level; an enum element keeps its base
                    // type's ranges below this level.
                    let below = match &element.shape {
                        TypeShape::PackedAtom { ranges } => ranges.len(),
                        _ => 0,
                    };
                    let level = ranges.get(..ranges.len().checked_sub(below)?)?.to_vec();
                    let element =
                        self.vpi_typespec(table, element, inner_typedef(naming), depth + 1)?;
                    return Some(IrVpiTypespec {
                        kind: IrVpiTypespecKind::PackedArray,
                        element: Some(element),
                        width,
                        signed: info.signed,
                        two_state: descriptor.two_state,
                        ranges: level,
                        ..IrVpiTypespec::default()
                    });
                }
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

    /// How an application sees expression `node` as a structure connection:
    /// a catalog object, a constant select or element of one, a literal
    /// with its value, or an opaque expression (IEEE 1800-2009 37.54).
    fn vpi_conn(&self, node: NodeId, direction: IrVpiDirection) -> IrVpiConn {
        let width = self.fixed_value_width(node).unwrap_or(0);
        let signed = self
            .query_descriptor(node)
            .is_some_and(|descriptor| descriptor.info.signed);
        // The owned database does not tell implicit conversions (a narrower
        // right-hand side extended to its target) from casts, so a
        // conversion of a named object or select shows that object: its
        // value is the operand's, which a cont assign value converts again.
        let mut inner = node;
        while let NodeKind::Expr(ExprKind::Cast {
            operand,
            size_cast: false,
            ..
        }) = self.kind(inner)
        {
            inner = *operand;
        }
        if !matches!(
            self.kind(inner),
            NodeKind::Expr(
                ExprKind::Constant { .. }
                    | ExprKind::Ref { .. }
                    | ExprKind::HierPath { .. }
                    | ExprKind::ScopeRef { .. }
                    | ExprKind::BitSelect { .. }
                    | ExprKind::PartSelect { .. }
                    | ExprKind::ArraySelect { .. }
            )
        ) {
            inner = node;
        }
        let (width, signed) = if inner == node {
            (width, signed)
        } else {
            (
                self.fixed_value_width(inner).unwrap_or(width),
                self.query_descriptor(inner)
                    .map_or(signed, |descriptor| descriptor.info.signed),
            )
        };
        let conn = |reference| IrVpiConn {
            reference,
            width,
            signed,
            direction,
            value: None,
            ..IrVpiConn::default()
        };
        match self.kind(inner) {
            NodeKind::Expr(ExprKind::Constant {
                value,
                size,
                const_type,
                source,
                ..
            }) => {
                let Ok(constant) = read_const_from(value, *size) else {
                    return conn(IrVpiArgRef::Expression);
                };
                if constant.real.is_some() || constant.width == 0 {
                    return conn(IrVpiArgRef::Expression);
                }
                let bit = |plane: &[u64], index: u32| {
                    plane
                        .get((index / 64) as usize)
                        .is_some_and(|word| word >> (index % 64) & 1 != 0)
                };
                let digits = (0..constant.width)
                    .rev()
                    .map(|index| {
                        if bit(&constant.z, index) {
                            'z'
                        } else if bit(&constant.x, index) {
                            'x'
                        } else if bit(&constant.bits, index) {
                            '1'
                        } else {
                            '0'
                        }
                    })
                    .collect::<String>();
                IrVpiConn {
                    reference: IrVpiArgRef::Constant(vpi_const_type(*const_type, source)),
                    width: constant.width,
                    signed: constant.signed,
                    direction,
                    value: Some(digits),
                    ..IrVpiConn::default()
                }
            }
            NodeKind::Expr(ExprKind::ScopeRef { target }) => {
                conn(IrVpiArgRef::Object(self.vpi_full_name(*target)))
            }
            NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. }) => {
                let target = self.vpi_reference_target(inner);
                let reference = target
                    .filter(|target| {
                        matches!(self.kind(*target), NodeKind::Param { value: Some(_), .. })
                            || self.sig_globals.contains_key(target)
                            || self.array_globals.contains_key(target)
                            || self.event_globals.contains_key(target)
                            || matches!(self.kind(*target), NodeKind::ModuleInst { .. })
                    })
                    .map_or(IrVpiArgRef::Expression, |target| {
                        IrVpiArgRef::Object(self.vpi_full_name(target))
                    });
                conn(reference)
            }
            _ => conn(self.vpi_select_reference(inner)),
        }
    }

    /// Catalog key of an unnamed object of `scope`: a hidden last path part
    /// that keeps source order among its siblings.
    fn vpi_unnamed_key(&self, scope: &str, tag: char, id: NodeId) -> String {
        let node = self.node(id);
        format!(
            "{scope}{SEPARATOR}\u{1e}#{tag}{:010}{:06}{:010}",
            node.line,
            node.col,
            id.index()
        )
    }

    /// The names of the primitive instances written without one (decision
    /// A19-D6): `<keyword>$<n>` for a gate or switch and `udp$<n>` for a
    /// UDP, `n` counting the unnamed primitives of the enclosing module
    /// instance from 1, its UDP instances first, each kind in elaboration
    /// order (generate blocks included). Elements of primitive arrays are
    /// named by their array.
    fn vpi_unnamed_primitive_names(
        &self,
        array_of: &HashMap<NodeId, NodeId>,
    ) -> HashMap<NodeId, String> {
        let mut by_instance: HashMap<NodeId, (Vec<NodeId>, Vec<NodeId>)> = HashMap::new();
        for id in self.db.node_ids() {
            let NodeKind::Gate { prim_type, .. } = self.kind(id) else {
                continue;
            };
            if !self.node(id).name.is_empty() || array_of.contains_key(&id) {
                continue;
            }
            let mut owner = self.node(id).parent;
            while let Some(scope) = owner {
                if matches!(self.kind(scope), NodeKind::ModuleInst { .. }) {
                    break;
                }
                owner = self.node(scope).parent;
            }
            let Some(instance) = owner else { continue };
            let (udps, gates) = by_instance.entry(instance).or_default();
            if matches!(
                prim_type,
                PrimitiveType::Sequential | PrimitiveType::Combinational
            ) {
                udps.push(id);
            } else {
                gates.push(id);
            }
        }
        let mut names = HashMap::new();
        for (udps, gates) in by_instance.into_values() {
            let udp_count = udps.len();
            for (ordinal, id) in udps.into_iter().chain(gates).enumerate() {
                let keyword = match self.kind(id) {
                    NodeKind::Gate { prim_type, .. } if ordinal >= udp_count => {
                        vpi_prim_type(*prim_type).map(|(_, keyword)| keyword)
                    }
                    _ => Some("udp"),
                };
                if let Some(keyword) = keyword {
                    names.insert(id, format!("{keyword}${}", ordinal + 1));
                }
            }
        }
        names
    }

    /// The structure rows of a gate, switch or UDP instance (37.31). An
    /// element of a primitive array is named after the array and its index;
    /// an instance written without a name by `unnamed_names`.
    fn vpi_primitive_row(
        &self,
        id: NodeId,
        array_of: &HashMap<NodeId, NodeId>,
        unnamed_names: &HashMap<NodeId, String>,
        template: IrVpiObject,
    ) -> Option<IrVpiObject> {
        let NodeKind::Gate {
            class,
            prim_type,
            terms,
            udp,
            strength0,
            strength1,
            ..
        } = self.kind(id)
        else {
            return None;
        };
        let node = self.node(id);
        // The elements of an array hang off a frontend container; the
        // array's own parent is their scope.
        let owner = array_of
            .get(&id)
            .map_or(node.parent, |array| self.node(*array).parent);
        let scope = self.vpi_full_name(owner?);
        let mut name = node.name.clone();
        if name.is_empty() {
            if let (Some(position), Some(array)) =
                (self.db.primitive_array_position(id), array_of.get(&id))
            {
                let (left, right) = self.db.instance_array_range(*array)?;
                let step = if left >= right { 1 } else { -1 };
                let index = i64::from(right) + i64::from(position.slice) * step;
                name = format!("{}[{index}]", self.node(*array).name);
            }
        }
        // A generated name keeps the source-ordered key of an unnamed
        // object; the emitted full name is its scope's and the name.
        let (full_name, unnamed) = match unnamed_names.get(&id) {
            Some(generated) if name.is_empty() => {
                name = generated.clone();
                (self.vpi_unnamed_key(&scope, 'g', id), false)
            }
            _ if name.is_empty() => (self.vpi_unnamed_key(&scope, 'g', id), true),
            _ => (format!("{scope}{SEPARATOR}{name}"), false),
        };
        let udp_type = matches!(
            prim_type,
            PrimitiveType::Sequential | PrimitiveType::Combinational
        );
        let switch = matches!(
            prim_type,
            PrimitiveType::Nmos
                | PrimitiveType::Pmos
                | PrimitiveType::Cmos
                | PrimitiveType::Rnmos
                | PrimitiveType::Rpmos
                | PrimitiveType::Rcmos
                | PrimitiveType::Rtran
                | PrimitiveType::Rtranif0
                | PrimitiveType::Rtranif1
                | PrimitiveType::Tran
                | PrimitiveType::Tranif0
                | PrimitiveType::Tranif1
        );
        // A pull source is a gate in the grammar (28.10) and a switch
        // object in VPI (decision A19-D47).
        let pull = matches!(prim_type, PrimitiveType::Pullup | PrimitiveType::Pulldown);
        let switch = switch || pull;
        let type_code = match class {
            PrimClass::Udp => "vpiUdp",
            PrimClass::Switch => "vpiSwitch",
            PrimClass::Gate if pull => "vpiSwitch",
            PrimClass::Gate => "vpiGate",
            PrimClass::Array if udp_type => "vpiUdp",
            PrimClass::Array if switch => "vpiSwitch",
            PrimClass::Array => "vpiGate",
        };
        let (detail, keyword) = vpi_prim_type(*prim_type)?;
        let mut ordered = terms.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|term| term.term_index);
        let conns = ordered
            .iter()
            .map(|term| {
                let direction = match term.direction {
                    Direction::Input => IrVpiDirection::Input,
                    Direction::Output => IrVpiDirection::Output,
                    Direction::Inout => IrVpiDirection::Inout,
                    _ => IrVpiDirection::None,
                };
                self.vpi_conn(term.expr, direction)
            })
            .collect();
        Some(IrVpiObject {
            full_name,
            name: if unnamed { String::new() } else { name },
            definition_name: Some(
                udp.as_ref()
                    .map_or_else(|| keyword.to_owned(), |table| table.name.clone()),
            ),
            kind: IrVpiObjectKind::Primitive,
            type_code: Some(type_code),
            detail: Some(detail),
            conns,
            unnamed,
            // 28.10: a pullup or pulldown source drives pull strength
            // without a specification; other primitives default to strong
            // (28.3.2, decision A19-D35).
            strength: Some(vpi_strengths(
                *strength0,
                *strength1,
                if matches!(prim_type, PrimitiveType::Pullup | PrimitiveType::Pulldown) {
                    "vpiPullDrive"
                } else {
                    "vpiStrongDrive"
                },
            )),
            ..template
        })
    }

    /// Whether task or function `id` belongs to an instance or generate
    /// scope (not a class or package), so its full name is hierarchical.
    /// The runtime activation identity `disable` uses for a named block,
    /// task or function, when it has one.
    fn vpi_activation(&self, id: NodeId) -> Option<(u32, u32)> {
        self.activation_target(id)
            .ok()
            .map(|target| (target.declaration(), target.instance()))
    }

    /// The definition and configuration properties of instance `id` (IEEE
    /// 1800-2009 37.10, 33.7). Unmapped sources are compiled into library
    /// `work` (33.3.1); llg rejects the delay-mode directives, so every
    /// definition has `vpiDelayModeNone`, which the runtime reports itself.
    fn vpi_instance_properties(&self, id: NodeId) -> Option<IrVpiInstance> {
        const DEFAULT_SOURCE_LIBRARY: &str = "work";
        let definition = self.db.instance_definition(id)?;
        Some(IrVpiInstance {
            def_file: definition.file.as_deref().map(str::to_owned),
            def_line: definition.line,
            library: self
                .db
                .source_library(id)
                .unwrap_or(DEFAULT_SOURCE_LIBRARY)
                .to_owned(),
            config: definition.config.clone(),
            cell: match self.kind(id) {
                NodeKind::ModuleInst { def_name, .. } => def_name
                    .split_once('@')
                    .map_or(def_name.as_str(), |(_, cell)| cell)
                    .to_owned(),
                _ => String::new(),
            },
            cell_define: definition.cell_define,
            automatic: definition.automatic,
            net_type: match definition.default_net_type {
                NetType::None => "vpiNone",
                net_type => vpi_net_type(net_type),
            },
            unconnected_drive: match self.db.unconnected_drive(id) {
                crate::core::db::UnconnectedDrive::Pull0 => "vpiPull0",
                crate::core::db::UnconnectedDrive::Pull1 => "vpiPull1",
                _ => "vpiHighZ",
            },
        })
    }

    fn vpi_declared_in_instance(&self, id: NodeId) -> bool {
        let mut current = self.node(id).parent;
        while let Some(parent) = current {
            if self.vpi_package_name(parent).is_some() {
                return true;
            }
            match self.kind(parent) {
                NodeKind::ModuleInst { .. } | NodeKind::GenScope => return true,
                NodeKind::Stmt(_) => current = self.node(parent).parent,
                _ => return false,
            }
        }
        false
    }

    /// The row of a module, interface or primitive instance array (37.11):
    /// its declared ranges (one nested array level per dimension) and the
    /// `vpiType` of its elements.
    fn vpi_instance_array_row(&self, id: NodeId) -> Option<IrVpiObject> {
        let node = self.node(id);
        let mut dims = Vec::new();
        let mut level = id;
        let element = loop {
            let (left, right) = self.db.instance_array_range(level)?;
            dims.push((i64::from(left), i64::from(right)));
            let first = *self.node(level).children.first()?;
            match self.kind(first) {
                NodeKind::InstanceArray => level = first,
                _ => break first,
            }
        };
        let (type_code, detail) = match self.kind(element) {
            NodeKind::ModuleInst {
                is_interface: true, ..
            } => ("vpiInterfaceArray", "vpiInterface"),
            NodeKind::ModuleInst { .. } if self.db.is_program_instance(element) => {
                ("vpiProgramArray", "vpiProgram")
            }
            NodeKind::ModuleInst { .. } => ("vpiModuleArray", "vpiModule"),
            NodeKind::Gate { .. } => {
                let primitive = self.vpi_primitive_row(
                    element,
                    &HashMap::new(),
                    &HashMap::new(),
                    IrVpiObject::default(),
                )?;
                match primitive.type_code {
                    Some("vpiUdp") => ("vpiUdpArray", "vpiUdp"),
                    Some("vpiSwitch") => ("vpiSwitchArray", "vpiSwitch"),
                    _ => ("vpiGateArray", "vpiGate"),
                }
            }
            _ => return None,
        };
        // Gate arrays are one-dimensional (IEEE 1800-2009 28.3); a deeper
        // primitive array would leave its elements unnamed.
        if !matches!(detail, "vpiModule" | "vpiInterface" | "vpiProgram") && dims.len() != 1 {
            return None;
        }
        Some(IrVpiObject {
            full_name: format!(
                "{}{SEPARATOR}{}",
                self.vpi_full_name(node.parent?),
                node.name
            ),
            name: node.name.clone(),
            kind: IrVpiObjectKind::InstanceArray,
            type_code: Some(type_code),
            detail: Some(detail),
            dims,
            ..IrVpiObject::default()
        })
    }

    /// Capture the source-owned hierarchy and storage identities used by the
    /// generated VPI catalog.  This runs after collection has assigned stable
    /// IR indices but before optimization can prune any otherwise dead signal.
    /// Fill the catalog. With `code` (a model generated for VPI
    /// applications) the expression and statement objects of processes,
    /// tasks and functions, structure connections and call arguments are
    /// added too (IEEE 1800-2009 37.54-37.74).
    pub(in super::super) fn populate_vpi_metadata(
        &self,
        model: &mut IrModel,
        tops: &[NodeId],
        mut timing: VpiTimingMetadata,
        code: bool,
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

        /// Whether `id` is a named statement block of a process, or of a
        /// task or function of an instance (not the frontend's declaration
        /// scope of the same name, nor an instance body), so every named
        /// block is captured exactly once.
        fn statement_block(cg: &Codegen<'_>, id: NodeId) -> bool {
            if cg.db.semantic_detail(id) == Some("StatementBlock") {
                return false;
            }
            let mut current = cg.node(id).parent;
            while let Some(parent) = current {
                match cg.kind(parent) {
                    NodeKind::Process { .. } => return true,
                    NodeKind::FuncTask { .. } => return cg.vpi_declared_in_instance(parent),
                    NodeKind::Stmt(_) => current = cg.node(parent).parent,
                    _ => return false,
                }
            }
            false
        }

        // Rows keyed by full name, each with the order it was first seen
        // in: the design walk visits declarations in source order, and
        // relations iterate a scope's rows in that order (decision A19-D47).
        let mut objects = BTreeMap::<String, (usize, IrVpiObject)>::new();
        let mut insert = |object: IrVpiObject| {
            let next = objects.len();
            objects
                .entry(object.full_name.clone())
                .or_insert((next, object));
        };
        // The rows whose statements or connection expressions become code
        // objects, by catalog key, with the scope key of their objects.
        let mut owners: Vec<(String, String, CodeOwner)> = Vec::new();
        let mut typespecs = TypespecTable::default();
        let mut dynamic = crate::sim::ir::IrVpiDynamic::default();
        let static_locals = self
            .proc_local_instances
            .iter()
            .filter_map(|((_, node), info)| info.static_signal.as_ref().map(|s| (*node, s.ir)))
            .collect::<HashMap<_, _>>();

        // Elements of primitive arrays are unnamed; their array names them.
        let mut array_of = HashMap::new();
        for id in self.db.node_ids() {
            if matches!(self.kind(id), NodeKind::InstanceArray) {
                for child in &self.node(id).children {
                    array_of.insert(*child, id);
                }
            }
        }
        let primitive_names = self.vpi_unnamed_primitive_names(&array_of);

        // Packages and `$unit` are top-level instances beside the top
        // modules (37.10).
        let roots: Vec<NodeId> = tops
            .iter()
            .copied()
            .chain(
                self.db
                    .node_ids()
                    .filter(|id| self.vpi_package_name(*id).is_some()),
            )
            .collect();
        for id in self.db.node_ids() {
            if !reachable(self.db, id, &roots) {
                continue;
            }
            let node = self.node(id);
            let source = |object: IrVpiObject| IrVpiObject {
                file: node.file().map(str::to_owned),
                line: node.line,
                time_unit_fs: self.timescale_of_node(id).unit_fs,
                constant_variable: self.db.is_const_variable(id),
                ..object
            };
            // Strings, handles and containers (37.17, 37.29).
            if let Some(storage) = self.vpi_dynamic_storage(model, id) {
                if let Some(row) =
                    self.vpi_dynamic_row(model, &mut dynamic, &mut typespecs, id, storage)
                {
                    // 37.17 detail 8: the initialization expression.
                    if let Some(init) = self.db.var_initializer(id).filter(|_| code) {
                        owners.push((
                            row.full_name.clone(),
                            node.parent
                                .map(|parent| self.vpi_full_name(parent))
                                .unwrap_or_default(),
                            CodeOwner::Initializer(init),
                        ));
                    }
                    insert(source(row));
                }
                continue;
            }
            if let Some(name) = self.vpi_package_name(id) {
                insert(source(IrVpiObject {
                    full_name: self.vpi_full_name(id),
                    name: name.clone(),
                    definition_name: Some(name),
                    kind: IrVpiObjectKind::Package,
                    qualifier: self.is_compilation_unit(id),
                    ..IrVpiObject::default()
                }));
                continue;
            }
            // A named clocking block and its clocking io declarations
            // (37.44); an unnamed default clocking block has no name to
            // key or report.
            if let Some(info) = self.db.clocking_block(id) {
                if node.name.is_empty() {
                    continue;
                }
                let key = self.vpi_full_name(id);
                if code {
                    owners.push((key.clone(), key.clone(), CodeOwner::Clocking(id, None)));
                }
                let clock = clock_edge(&info.event_specs);
                insert(source(IrVpiObject {
                    full_name: key,
                    name: node.name.clone(),
                    kind: IrVpiObjectKind::ClockingBlock,
                    detail: Some(edge_pair(
                        skew_edge(&info.default_input, clock),
                        skew_edge(&info.default_output, clock),
                    )),
                    qualifier: info.is_global,
                    ..IrVpiObject::default()
                }));
                continue;
            }
            if let Some(var) = self.db.clocking_var(id) {
                let Some(info) = self
                    .db
                    .clocking_block(var.block)
                    .filter(|_| !self.node(var.block).name.is_empty())
                else {
                    continue;
                };
                let clock = clock_edge(&info.event_specs);
                let (input, output) = clocking_var_skews(var, info);
                let key = self.vpi_full_name(id);
                let scope = self.vpi_full_name(var.block);
                if code {
                    owners.push((
                        key.clone(),
                        scope.clone(),
                        CodeOwner::Conns(vec![var.expression]),
                    ));
                    owners.push((key.clone(), scope, CodeOwner::Clocking(var.block, Some(id))));
                }
                insert(source(IrVpiObject {
                    full_name: key,
                    name: node.name.clone(),
                    kind: IrVpiObjectKind::ClockingIoDecl,
                    direction: match var.direction {
                        Direction::Input => IrVpiDirection::Input,
                        Direction::Output => IrVpiDirection::Output,
                        Direction::Inout => IrVpiDirection::Inout,
                        _ => IrVpiDirection::None,
                    },
                    detail: Some(edge_pair(
                        input.map_or("0", |skew| skew_edge(skew, clock)),
                        output.map_or("0", |skew| skew_edge(skew, clock)),
                    )),
                    conns: vec![self.vpi_conn(var.expression, IrVpiDirection::None)],
                    // Declaration order, which vpiClockingIODecl keeps.
                    port_index: u32::try_from(
                        self.node(var.block)
                            .children
                            .iter()
                            .take_while(|child| **child != id)
                            .count(),
                    )
                    .unwrap_or(u32::MAX),
                    ..IrVpiObject::default()
                }));
                continue;
            }
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
                    let key = path.join(SEPARATOR);
                    // vpiDefaultClocking and vpiDefaultDisableIff (37.5,
                    // 37.6, 37.9).
                    let default_clocking = self
                        .db
                        .default_clocking(id)
                        .filter(|block| !self.node(*block).name.is_empty())
                        .map(|block| self.vpi_full_name(block));
                    let disable = self.db.instance_default_disable(id);
                    if let (true, Some(disable)) = (code, disable) {
                        owners.push((key.clone(), key.clone(), CodeOwner::Conns(vec![disable])));
                    }
                    insert(source(IrVpiObject {
                        port_target: default_clocking,
                        conns: disable
                            .map(|disable| vec![self.vpi_conn(disable, IrVpiDirection::None)])
                            .unwrap_or_default(),
                        full_name: key,
                        name,
                        definition_name: Some(def_name.clone()),
                        kind: if *is_interface {
                            IrVpiObjectKind::Interface
                        } else if self.db.is_program_instance(id) {
                            IrVpiObjectKind::Program
                        } else {
                            IrVpiObjectKind::Module
                        },
                        top: *is_top,
                        instance: self.vpi_instance_properties(id).map(Box::new),
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::GenScope if !node.name.is_empty() => {
                    let path = self.vpi_path(id);
                    insert(source(IrVpiObject {
                        full_name: path.join(SEPARATOR),
                        name: path.last().cloned().unwrap_or_default(),
                        kind: IrVpiObjectKind::GenScope,
                        port_target: self.vpi_gen_scope_array_of(id),
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::GenScopeArray => {
                    if let Some(row) = self.vpi_gen_scope_array_row(id) {
                        insert(source(row));
                    }
                }
                NodeKind::Genvar { .. } => {
                    if let Some(row) = self.vpi_genvar_row(id) {
                        insert(source(row));
                    }
                }
                // An unnamed block that declares items, a `for` that
                // declares its loop variables, a `foreach` (37.12 details
                // 1-3, decision A19-D27).
                NodeKind::Stmt(StmtKind::Begin) if self.vpi_unnamed_scope_name(id).is_some() => {
                    let Some((block, statement)) = self.vpi_scope_statement(id) else {
                        continue;
                    };
                    let line = match statement {
                        ScopeStatement::For(at) | ScopeStatement::Foreach(at) => self.node(at).line,
                        _ => self
                            .db
                            .block_keyword_line(block)
                            .unwrap_or(self.node(block).line),
                    };
                    let path = self.vpi_path(id);
                    insert(IrVpiObject {
                        line,
                        ..source(IrVpiObject {
                            full_name: path.join(SEPARATOR),
                            name: path.last().cloned().unwrap_or_default(),
                            kind: IrVpiObjectKind::StatementScope,
                            type_code: Some(statement.type_code()),
                            ..IrVpiObject::default()
                        })
                    });
                }
                NodeKind::Stmt(kind @ (StmtKind::Begin | StmtKind::Fork { .. }))
                    if !node.name.is_empty() && statement_block(self, id) =>
                {
                    let row = source(IrVpiObject {
                        full_name: self.vpi_full_name(id),
                        name: node.name.clone(),
                        kind: if matches!(kind, StmtKind::Fork { .. }) {
                            IrVpiObjectKind::NamedFork
                        } else {
                            IrVpiObjectKind::NamedBegin
                        },
                        activation: self.vpi_activation(id),
                        ..IrVpiObject::default()
                    });
                    // The block starts at its `begin`/`fork` keyword
                    // (decision A19-D13).
                    insert(IrVpiObject {
                        line: self.db.block_keyword_line(id).unwrap_or(row.line),
                        ..row
                    });
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
                    if let Some(init) = self
                        .db
                        .var_initializer(id)
                        .filter(|_| code && !declared_net)
                    {
                        owners.push((
                            self.vpi_full_name(id),
                            node.parent
                                .map(|parent| self.vpi_full_name(parent))
                                .unwrap_or_default(),
                            CodeOwner::Initializer(init),
                        ));
                    }
                    if let Some(delays) = timing.delay_nodes.get(&id).filter(|_| code) {
                        owners.push((
                            self.vpi_full_name(id),
                            node.parent
                                .map(|parent| self.vpi_full_name(parent))
                                .unwrap_or_default(),
                            CodeOwner::Delay(delays.clone()),
                        ));
                    }
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
                    let typespec = self.vpi_declared_typespec(&mut typespecs, id);
                    // 37.18: a packed array of structures, unions or enums
                    // is a vpiPackedArrayVar; an integer atom or time
                    // declared through a typedef keeps its keyword's kind.
                    let typed_kind = typespec.and_then(|typespec| {
                        let kind = typespecs.entries[typespec].kind;
                        matches!(
                            kind,
                            IrVpiTypespecKind::PackedArray
                                | IrVpiTypespecKind::Int
                                | IrVpiTypespecKind::Integer
                                | IrVpiTypespecKind::ShortInt
                                | IrVpiTypespecKind::LongInt
                                | IrVpiTypespecKind::Byte
                                | IrVpiTypespecKind::Time
                        )
                        .then(|| typespec_var_kind(kind, 0))
                    });
                    insert(source(IrVpiObject {
                        full_name: self.vpi_full_name(id),
                        name: node.name.clone(),
                        kind,
                        width,
                        signed,
                        real,
                        net,
                        signal: Some(ir),
                        var_kind: typed_kind.unwrap_or_else(|| {
                            IrVpiVarKind::from_type_kind(
                                &ty.kind,
                                self.vpi_packed_dimension_count(id),
                            )
                        }),
                        packed_range: (!real).then(|| self.vpi_packed_range(id, width)).flatten(),
                        net_type: net.then(|| net_type.unwrap_or("vpiWire")),
                        two_state,
                        typespec,
                        implicit: declared_net && self.db.is_implicit_net(id),
                        qualifiers: vpi_qualifiers(self.db.declaration_qualifiers(id)),
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
                    if let Some(delays) = timing.delay_nodes.get(&id).filter(|_| code && net) {
                        owners.push((
                            self.vpi_full_name(id),
                            node.parent
                                .map(|parent| self.vpi_full_name(parent))
                                .unwrap_or_default(),
                            CodeOwner::Delay(delays.clone()),
                        ));
                    }
                    if let Some(init) = self
                        .db
                        .arrays()
                        .get(&id)
                        .and_then(|meta| meta.init)
                        .filter(|_| code && !net)
                    {
                        owners.push((
                            self.vpi_full_name(id),
                            node.parent
                                .map(|parent| self.vpi_full_name(parent))
                                .unwrap_or_default(),
                            CodeOwner::Initializer(init),
                        ));
                    }
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
                    let Some((constant, width, signed, real)) = vpi_parameter_constant(value)
                    else {
                        continue;
                    };
                    if let Some(init) = self.db.param_initializer(id).filter(|_| code) {
                        owners.push((
                            self.vpi_full_name(id),
                            node.parent
                                .map(|parent| self.vpi_full_name(parent))
                                .unwrap_or_default(),
                            CodeOwner::Initializer(init),
                        ));
                    }
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
                        // 37.26: a specparam is its own object type.
                        type_code: self.db.specparam(id).map(|_| "vpiSpecParam"),
                        constant: Some(constant),
                        typespec: self.vpi_declared_typespec(&mut typespecs, id),
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::NamedEvent | NodeKind::Var { .. }
                    if !node.name.is_empty() && self.event_globals.contains_key(&id) =>
                {
                    insert(source(IrVpiObject {
                        full_name: self.vpi_full_name(id),
                        name: node.name.clone(),
                        kind: IrVpiObjectKind::NamedEvent,
                        event: Some(self.event_globals[&id].ir),
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::Process { kind } => {
                    let (type_code, detail) = match kind {
                        ProcessKind::Initial => ("vpiInitial", None),
                        ProcessKind::Final => ("vpiFinal", None),
                        ProcessKind::Always { always_type } => (
                            "vpiAlways",
                            match always_type {
                                AlwaysKind::Always => Some("vpiAlways"),
                                AlwaysKind::Comb => Some("vpiAlwaysComb"),
                                AlwaysKind::FlipFlop => Some("vpiAlwaysFF"),
                                AlwaysKind::Latch => Some("vpiAlwaysLatch"),
                                AlwaysKind::Unsupported => None,
                            },
                        ),
                    };
                    let scope = self.vpi_full_name(id);
                    let key = self.vpi_unnamed_key(&scope, 'p', id);
                    if let Some(statement) = node.children.first().filter(|_| code) {
                        owners.push((key.clone(), scope.clone(), CodeOwner::Statement(*statement)));
                    }
                    insert(source(IrVpiObject {
                        full_name: key,
                        kind: IrVpiObjectKind::Process,
                        type_code: Some(type_code),
                        detail,
                        unnamed: true,
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::ContAssign {
                    net_decl,
                    strength0,
                    strength1,
                    ..
                } if node.children.len() >= 2 => {
                    let lhs = self.vpi_conn(node.children[0], IrVpiDirection::None);
                    let rhs = self.vpi_conn(node.children[1], IrVpiDirection::None);
                    let scope = self.vpi_full_name(id);
                    let key = self.vpi_unnamed_key(&scope, 'c', id);
                    if code {
                        owners.push((
                            key.clone(),
                            scope.clone(),
                            CodeOwner::Conns(vec![node.children[0], node.children[1]]),
                        ));
                        if let Some(delays) = timing.delay_nodes.get(&id) {
                            owners.push((
                                key.clone(),
                                scope.clone(),
                                CodeOwner::Delay(delays.clone()),
                            ));
                        }
                    }
                    insert(source(IrVpiObject {
                        full_name: key,
                        kind: IrVpiObjectKind::ContAssign,
                        width: lhs.width,
                        conns: vec![lhs, rhs],
                        unnamed: true,
                        qualifier: *net_decl,
                        timing: timing.delays.remove(&id),
                        // 10.3.4: (strong1, strong0) without a specification.
                        strength: Some(vpi_strengths(*strength0, *strength1, "vpiStrongDrive")),
                        ..IrVpiObject::default()
                    }));
                }
                NodeKind::FuncTask {
                    is_task,
                    automatic,
                    ret,
                    ..
                } if !node.name.is_empty() && self.vpi_declared_in_instance(id) => {
                    let detail = vpi_func_type(*is_task, ret.as_ref());
                    let key = self.vpi_full_name(id);
                    if let Some(body) = self.db.subroutine_body(id).filter(|_| code) {
                        owners.push((key.clone(), key.clone(), CodeOwner::Statement(body)));
                    }
                    insert(source(IrVpiObject {
                        full_name: key,
                        name: node.name.clone(),
                        kind: IrVpiObjectKind::TaskFunc,
                        type_code: Some(if *is_task { "vpiTask" } else { "vpiFunction" }),
                        detail,
                        width: ret.as_ref().and_then(|ret| ret.width).unwrap_or(0),
                        signed: ret.as_ref().is_some_and(|ret| ret.signed),
                        qualifier: *automatic,
                        activation: self.vpi_activation(id),
                        ..IrVpiObject::default()
                    }));
                    for row in self.vpi_argument_io_decls(&mut typespecs, id) {
                        insert(row);
                    }
                }
                NodeKind::Gate { .. } => {
                    if let Some(row) = self.vpi_primitive_row(
                        id,
                        &array_of,
                        &primitive_names,
                        source(IrVpiObject {
                            timing: timing.delays.remove(&id),
                            ..IrVpiObject::default()
                        }),
                    ) {
                        if let (true, NodeKind::Gate { terms, .. }) = (code, self.kind(id)) {
                            let mut ordered = terms.iter().collect::<Vec<_>>();
                            ordered.sort_by_key(|term| term.term_index);
                            let scope = row
                                .full_name
                                .rsplit_once(SEPARATOR)
                                .map_or_else(String::new, |(scope, _)| scope.to_owned());
                            if let Some(delays) = timing.delay_nodes.get(&id) {
                                owners.push((
                                    row.full_name.clone(),
                                    scope.clone(),
                                    CodeOwner::Delay(delays.clone()),
                                ));
                            }
                            owners.push((
                                row.full_name.clone(),
                                scope,
                                CodeOwner::Conns(ordered.iter().map(|term| term.expr).collect()),
                            ));
                        }
                        insert(row);
                    }
                }
                NodeKind::InstanceArray
                    if !node.name.is_empty()
                        && node.parent.is_some_and(|parent| {
                            !matches!(self.kind(parent), NodeKind::InstanceArray)
                        }) =>
                {
                    if let Some(row) = self.vpi_instance_array_row(id) {
                        // A primitive array declares the delay of its
                        // elements (28.3, 37.31).
                        let delays = node
                            .children
                            .first()
                            .filter(|_| code && row.detail != Some("vpiModule"))
                            .and_then(|element| timing.delay_nodes.get(element));
                        if let Some(delays) = delays {
                            owners.push((
                                row.full_name.clone(),
                                row.full_name
                                    .rsplit_once(SEPARATOR)
                                    .map_or_else(String::new, |(scope, _)| scope.to_owned()),
                                CodeOwner::Delay(delays.clone()),
                            ));
                        }
                        insert(source(row));
                    }
                }
                NodeKind::ModPort
                    if !node.name.is_empty()
                        && node.parent.is_some_and(|parent| {
                            matches!(self.kind(parent), NodeKind::ModuleInst { .. })
                        }) =>
                {
                    let Some(parent) = node.parent else {
                        continue;
                    };
                    insert(source(IrVpiObject {
                        full_name: format!(
                            "{}{SEPARATOR}{}",
                            self.vpi_full_name(parent),
                            node.name
                        ),
                        name: node.name.clone(),
                        kind: IrVpiObjectKind::Modport,
                        ..IrVpiObject::default()
                    }));
                    for row in self.vpi_modport_io_decls(&mut typespecs, id, parent) {
                        insert(row);
                    }
                }
                NodeKind::Port {
                    direction,
                    low,
                    ty,
                    high_expr,
                    high_open,
                    ..
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
                    // An interface or modport port connects an interface
                    // instance (37.14 detail 1, 37.15).
                    let interface = node.children.iter().find_map(|child| {
                        let NodeKind::IfaceConn { actual, modport } = self.kind(*child) else {
                            return None;
                        };
                        let actual = self.vpi_full_name(*actual);
                        Some(if modport.is_empty() {
                            ("vpiInterfacePort", actual)
                        } else {
                            ("vpiModportPort", format!("{actual}{SEPARATOR}{modport}"))
                        })
                    });
                    let key = format!("{}{SEPARATOR}\u{1e}port", self.vpi_full_name(id));
                    if let (true, Some(high), false, None) =
                        (code, high_expr, *high_open, interface.as_ref())
                    {
                        // The high connection lives in the instantiating scope.
                        let scope = self
                            .node(parent)
                            .parent
                            .map_or_else(String::new, |outer| self.vpi_full_name(outer));
                        owners.push((key.clone(), scope, CodeOwner::Conns(vec![*high])));
                    }
                    let port = source(IrVpiObject {
                        // A port and the declaration it connects share one
                        // name; the port lives in its own namespace.
                        full_name: key,
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
                        // The SDF interconnect loads of its bits carry its
                        // module input port delays (37.14).
                        timing: timing.port_loads.remove(&id).map(|loads| IrVpiTiming {
                            targets: loads
                                .into_iter()
                                .map(|(bit, load)| (bit, IrSdfTarget::Load { load, entry: 0 }))
                                .collect(),
                            ..IrVpiTiming::default()
                        }),
                        port_target: interface
                            .as_ref()
                            .map(|(_, actual)| actual.clone())
                            .or_else(|| low.map(|low| self.vpi_full_name(low))),
                        detail: interface.as_ref().map(|(port_type, _)| *port_type),
                        // A port has the type of the declaration it connects
                        // (37.14); an interface port has no data type.
                        typespec: match (interface.is_some(), low) {
                            (false, Some(low)) => self.vpi_declared_typespec(&mut typespecs, *low),
                            _ => None,
                        },
                        conns: match (high_expr, high_open, interface.is_some()) {
                            (Some(high), false, false) => {
                                vec![self.vpi_conn(*high, IrVpiDirection::None)]
                            }
                            _ => Vec::new(),
                        },
                        qualifiers: vpi_qualifiers(self.db.declaration_qualifiers(id)),
                        ..IrVpiObject::default()
                    });
                    // A non-ANSI port is on its module header line (decision
                    // A19-D47); its io declaration keeps its own line.
                    insert(IrVpiObject {
                        line: self.db.port_header_line(id).unwrap_or(port.line),
                        ..port
                    });
                    // 37.13: a module port's io declaration; interface and
                    // modport ports declare an interface, not a value.
                    if interface.is_none()
                        && matches!(
                            self.kind(parent),
                            NodeKind::ModuleInst {
                                is_interface: false,
                                ..
                            }
                        )
                    {
                        let declaration = low
                            .filter(|low| {
                                matches!(
                                    self.kind(*low),
                                    NodeKind::Var { .. } | NodeKind::Net { .. }
                                )
                            })
                            .unwrap_or(id);
                        insert(self.vpi_io_decl_row(
                            &mut typespecs,
                            id,
                            &self.vpi_full_name(parent),
                            &node.name,
                            declarations::vpi_direction(*direction),
                            port_index,
                            Some(declaration),
                            low.map(|low| self.vpi_full_name(low)),
                        ));
                    }
                }
                _ => {}
            }
        }
        for row in timing.rows {
            insert(row);
        }
        if code {
            for (key, scope, delays) in timing.row_delays {
                owners.push((key, scope, CodeOwner::Delay(delays)));
            }
        }
        let constraints = self.vpi_dynamic_classes(model, &mut dynamic, &mut typespecs);
        let mut rows = objects.into_values().collect::<Vec<_>>();
        rows.sort_unstable_by_key(|(order, _)| *order);
        model.vpi_objects = rows.into_iter().map(|(_, object)| object).collect();
        if code {
            self.populate_vpi_code(model, &mut typespecs, owners, &mut dynamic, constraints);
        }
        model.vpi_typespecs = typespecs.entries;
        model.vpi_dynamic = dynamic;
        Ok(())
    }

    /// Build the expression and statement objects of the catalog rows in
    /// `owners` and of the VPI call arguments, and link them to their rows.
    fn populate_vpi_code(
        &self,
        model: &mut IrModel,
        typespecs: &mut TypespecTable,
        owners: Vec<(String, String, CodeOwner)>,
        dynamic: &mut crate::sim::ir::IrVpiDynamic,
        constraints: Vec<(usize, usize, NodeId)>,
    ) {
        let mut statements = Vec::new();
        let mut conns = Vec::new();
        let mut arguments = Vec::new();
        let mut delays = Vec::new();
        let mut assertions = Vec::new();
        // Each assertion item's statement and the catalog row of its
        // instance.
        let items: Vec<(Option<NodeId>, Option<String>, String)> = model
            .assertion_coverage
            .as_ref()
            .map(|coverage| {
                coverage
                    .items
                    .iter()
                    .map(|item| {
                        let statement = usize::try_from(item.identity)
                            .ok()
                            .filter(|index| *index < self.db.node_count())
                            .map(NodeId::from_index);
                        let scope = item
                            .instance
                            .and_then(|index| coverage.instances.get(index as usize))
                            .map(|instance| instance.path.clone());
                        (statement, scope, item.name.clone())
                    })
                    .collect()
            })
            .unwrap_or_default();
        let scope_rows: HashSet<usize> = model
            .vpi_objects
            .iter()
            .enumerate()
            .filter(|(_, object)| object.kind.is_scope())
            .map(|(row, _)| row)
            .collect();
        let code::FinishedCode {
            code,
            files,
            named_blocks,
            statement_rows,
        } = {
            let mut tables = code::CodeTables::new(self, model, typespecs);
            // Declarations first, so the instances of assertions link to
            // them (37.12: a scope holds its sequence and property
            // declarations).
            for node in self.design_nodes() {
                if !matches!(self.kind(node), NodeKind::Other)
                    || !matches!(self.db.semantic_detail(node), Some("Sequence" | "Property"))
                {
                    continue;
                }
                let Some(scope) = self
                    .node(node)
                    .parent
                    .and_then(|parent| tables.row(&self.vpi_full_name(parent)))
                else {
                    continue;
                };
                tables.declaration_root(node, Some(scope));
            }
            for (index, (statement, scope, label)) in items.iter().enumerate() {
                let Some(statement) = *statement else {
                    continue;
                };
                if !matches!(
                    self.kind(statement),
                    NodeKind::Stmt(
                        StmtKind::ConcurrentAssertion { .. } | StmtKind::ImmediateAssertion { .. }
                    )
                ) {
                    continue;
                }
                // The scope that declares the statement (a generate scope or
                // named block included), which the vpiConcurrentAssertions
                // and generate-scope vpiAssertion iterations select by
                // (37.12, 37.80); the owning instance when none is a row.
                let mut declaring = None;
                let mut current = self.node(statement).parent;
                // A label is the assertion's name, not a scope of its own: the
                // frontend's named block around a labeled statement is skipped.
                if let Some(parent) = current.filter(|parent| {
                    matches!(self.kind(*parent), NodeKind::Stmt(StmtKind::Begin))
                        && self.node(*parent).name == *label
                        && self.node(*parent).children.first() == Some(&statement)
                }) {
                    current = self.node(parent).parent;
                }
                while let Some(node) = current {
                    if let Some(row) = tables
                        .row(&self.vpi_full_name(node))
                        .filter(|row| scope_rows.contains(row))
                    {
                        declaring = Some(row);
                        break;
                    }
                    current = self.node(node).parent;
                }
                let scope =
                    declaring.or_else(|| scope.as_deref().and_then(|scope| tables.row(scope)));
                assertions.push((index, tables.assertion_root(statement, scope)));
            }
            for (key, scope, owner) in owners {
                let Some(row) = tables.row(&key) else {
                    continue;
                };
                let scope = tables.row(&scope);
                match owner {
                    CodeOwner::Statement(statement) => {
                        if let Some(entry) = tables.statement_root(statement, scope) {
                            statements.push((row, entry));
                        }
                    }
                    CodeOwner::Clocking(block, var) => {
                        statements.push((row, tables.clocking_root(block, var, scope)));
                    }
                    CodeOwner::Initializer(expression) => {
                        statements.push((row, tables.expression_root(expression, scope)));
                    }
                    CodeOwner::Delay(expressions) => {
                        if let Some(entry) = tables.delay_root(&expressions, scope) {
                            delays.push((row, entry));
                        }
                    }
                    CodeOwner::Conns(expressions) => {
                        let object = &model.vpi_objects[row];
                        if object.conns.len() != expressions.len() {
                            continue;
                        }
                        for (index, expression) in expressions.into_iter().enumerate() {
                            if matches!(
                                object.conns[index].reference,
                                IrVpiArgRef::Expression | IrVpiArgRef::Constant(_)
                            ) {
                                let entry = tables.expression_root(expression, scope);
                                conns.push((row, index, entry));
                            }
                        }
                    }
                }
            }
            // The constraint items of class constraints (37.30). They live
            // in a class, which is not a catalog scope: their expressions
            // report no vpiScope.
            for (class, item, body) in constraints {
                let root = tables.constraint_root(body, None);
                if let Some(target) = dynamic
                    .classes
                    .get_mut(class)
                    .and_then(|entry| entry.items.get_mut(item))
                {
                    target.code = Some(root);
                }
            }
            for (site, call) in model.vpi_compile_calls.iter().enumerate() {
                let Some(nodes) = self.vpi_call_args.get(site) else {
                    continue;
                };
                let scope = call.scope.as_deref().and_then(|scope| tables.row(scope));
                for (index, (argument, node)) in call.args.iter().zip(nodes).enumerate() {
                    if matches!(
                        argument.reference,
                        IrVpiArgRef::Expression | IrVpiArgRef::Constant(_)
                    ) {
                        arguments.push((site, index, tables.expression_root(*node, scope)));
                    }
                }
            }
            tables.finish()
        };
        for (row, entry) in statements {
            model.vpi_objects[row].code = Some(entry);
        }
        for (row, entry) in named_blocks {
            model.vpi_objects[row].code.get_or_insert(entry);
        }
        for (row, entry) in delays {
            model.vpi_objects[row]
                .timing
                .get_or_insert_with(IrVpiTiming::default)
                .delay = Some(entry);
        }
        for (row, index, entry) in conns {
            model.vpi_objects[row].conns[index].code = Some(entry);
        }
        for (site, index, entry) in arguments {
            model.vpi_compile_calls[site].args[index].code = Some(entry);
        }
        if let Some(coverage) = model.assertion_coverage.as_mut() {
            for (index, entry) in assertions {
                coverage.items[index].code = Some(entry);
            }
            // Statement coverage entities reached as statement objects
            // (40.5.3 vpiCovered of a statement handle).
            for item in &mut coverage.statements {
                let node = usize::try_from(item.identity)
                    .ok()
                    .filter(|index| *index < self.db.node_count())
                    .map(NodeId::from_index);
                item.code = node.and_then(|node| statement_rows.get(&node).copied());
            }
        }
        model.vpi_code = code;
        model.vpi_code_files = files;
        model.vpi_udps = self.vpi_udp_definitions();
    }

    /// The UDP definitions the design instantiates (37.32), once each, in
    /// the order their first instance appears.
    fn vpi_udp_definitions(&self) -> Vec<IrVpiUdp> {
        let mut seen = HashSet::new();
        let mut udps = Vec::new();
        for node in self.design_nodes() {
            let NodeKind::Gate {
                udp: Some(table), ..
            } = self.kind(node)
            else {
                continue;
            };
            if !seen.insert(table.name.clone()) {
                continue;
            }
            let field = |c: u8| char::from(c).to_string();
            let entries = table
                .rows
                .iter()
                .map(|row| {
                    let mut text = row.inputs.clone();
                    if let Some(state) = row.state {
                        text.push(':');
                        text.push_str(&field(state));
                    }
                    text.push(':');
                    text.push_str(&field(row.output));
                    (text, row.location.line)
                })
                .collect();
            let ports = table
                .ports
                .iter()
                .map(|(name, direction)| {
                    let direction = match direction {
                        Direction::Output => "vpiOutput",
                        Direction::Inout => "vpiInout",
                        _ => "vpiInput",
                    };
                    (name.clone(), direction)
                })
                .collect();
            udps.push(IrVpiUdp {
                name: table.name.clone(),
                file: table
                    .location
                    .file
                    .as_deref()
                    .map(str::to_owned)
                    .unwrap_or_default(),
                line: table.location.line,
                input_count: table.input_count,
                sequential: table.sequential,
                initial: table.initial,
                ports,
                entries,
            });
        }
        udps
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
                code: None,
            })
            .collect();
        let mut scope = self.node(call).parent;
        while let Some(id) = scope {
            let node = self.node(id);
            let is_scope = match self.kind(id) {
                NodeKind::ModuleInst { .. } => true,
                NodeKind::GenScope => !node.name.is_empty(),
                // A named block, or an unnamed one that is a scope
                // (decision A19-D27).
                NodeKind::Stmt(StmtKind::Begin | StmtKind::Fork { .. }) => {
                    !node.name.is_empty()
                        || self
                            .vpi_block_scope(id)
                            .is_some_and(|declared| self.vpi_unnamed_scope_name(declared).is_some())
                }
                _ => false,
            };
            if is_scope {
                break;
            }
            scope = node.parent;
        }
        let site = self.model.vpi_compile_calls.len();
        if self.vpi_call_args.len() < site {
            self.vpi_call_args.resize(site, Vec::new());
        }
        self.vpi_call_args.push(arg_nodes.to_vec());
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
    /// The spelling of a module path terminal (30.4.3: a net or port, or a
    /// constant bit- or part-select of one), or `None` for another form.
    fn vpi_terminal_text(&self, node: NodeId) -> Option<String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.vpi_terminal_text(*operand),
            NodeKind::Expr(ExprKind::Ref { .. }) => {
                Some(self.node(node).name.clone()).filter(|name| !name.is_empty())
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => Some(format!(
                "{}[{}]",
                self.vpi_terminal_text(*base)?,
                self.vpi_constant_index(*index)?
            )),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                Some(format!(
                    "{}[{}]",
                    self.vpi_terminal_text(*base)?,
                    self.vpi_constant_index(indices[0])?
                ))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => Some(format!(
                "{}[{}:{}]",
                self.vpi_terminal_text(*base)?,
                self.vpi_constant_index(*left)?,
                self.vpi_constant_index(*right)?
            )),
            _ => None,
        }
    }

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

/// A catalog row whose statement or connection expressions become code
/// objects.
enum CodeOwner {
    /// The `vpiStmt` of a process, task or function.
    Statement(NodeId),
    /// The source expression of each connection, in connection order.
    Conns(Vec<NodeId>),
    /// The clocking event and skews of clocking block `.0`, or the skews of
    /// its clocking io declaration `.1` (37.44).
    Clocking(NodeId, Option<NodeId>),
    /// The initialization expression of a variable or the default
    /// expression of a value parameter (37.17 detail 8, 37.26 detail 3).
    Initializer(NodeId),
    /// The delay expressions of a primitive, net, continuous assignment,
    /// module path or timing check (`vpiDelay`, 37.3.4).
    Delay(Vec<NodeId>),
}

/// The edge of a clocking event: that of a single edge event, else none.
fn clock_edge(specs: &[EventSpec]) -> &'static str {
    match specs {
        [EventSpec::Edge { posedge: true, .. }] => "vpiPosedge",
        [EventSpec::Edge { posedge: false, .. }] => "vpiNegedge",
        _ => "0",
    }
}

/// `vpiInputEdge`/`vpiOutputEdge` of a skew: its own edge, else the edge of
/// the clocking event it is relative to (14.4; decision A19-D30).
fn skew_edge(skew: &ClockingSkew, clock: &'static str) -> &'static str {
    match skew.edge {
        ClockingEdge::None => clock,
        ClockingEdge::Posedge => "vpiPosedge",
        ClockingEdge::Negedge => "vpiNegedge",
        ClockingEdge::BothEdges => "vpiAnyEdge",
    }
}

/// The catalog `detail` of a clocking block or io declaration: the input
/// edge in the low byte, the output edge in the next.
fn edge_pair(input: &'static str, output: &'static str) -> &'static str {
    const EDGES: [&str; 4] = ["0", "vpiPosedge", "vpiNegedge", "vpiAnyEdge"];
    const PAIRS: [[&str; 4]; 4] = [
        [
            "0",
            "(vpiPosedge << 8)",
            "(vpiNegedge << 8)",
            "(vpiAnyEdge << 8)",
        ],
        [
            "vpiPosedge",
            "(vpiPosedge | vpiPosedge << 8)",
            "(vpiPosedge | vpiNegedge << 8)",
            "(vpiPosedge | vpiAnyEdge << 8)",
        ],
        [
            "vpiNegedge",
            "(vpiNegedge | vpiPosedge << 8)",
            "(vpiNegedge | vpiNegedge << 8)",
            "(vpiNegedge | vpiAnyEdge << 8)",
        ],
        [
            "vpiAnyEdge",
            "(vpiAnyEdge | vpiPosedge << 8)",
            "(vpiAnyEdge | vpiNegedge << 8)",
            "(vpiAnyEdge | vpiAnyEdge << 8)",
        ],
    ];
    let at = |edge| EDGES.iter().position(|e| *e == edge).unwrap_or(0);
    PAIRS[at(input)][at(output)]
}

/// The effective input and output skews of a clocking io declaration: its
/// own, else the block's default, for the directions it has (14.3).
fn clocking_var_skews<'a>(
    var: &'a crate::core::db::ClockingVarInfo,
    block: &'a crate::core::db::ClockingBlockInfo,
) -> (Option<&'a ClockingSkew>, Option<&'a ClockingSkew>) {
    let own = |skew: &'a ClockingSkew, default: &'a ClockingSkew| {
        if skew.delay.is_some() || !matches!(skew.edge, ClockingEdge::None) {
            skew
        } else {
            default
        }
    };
    let input = matches!(var.direction, Direction::Input | Direction::Inout)
        .then(|| own(&var.input, &block.default_input));
    let output = matches!(var.direction, Direction::Output | Direction::Inout)
        .then(|| own(&var.output, &block.default_output));
    (input, output)
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

/// The `vpiStrength0` and `vpiStrength1` code names of a driver: its
/// specified strengths, `default` for an unspecified one (Annex K drive
/// strength constants).
fn vpi_strengths(
    strength0: Strength,
    strength1: Strength,
    default: &'static str,
) -> (&'static str, &'static str) {
    let code = |strength| match strength {
        Strength::Supply => "vpiSupplyDrive",
        Strength::Strong => "vpiStrongDrive",
        Strength::Pull => "vpiPullDrive",
        Strength::Weak => "vpiWeakDrive",
        Strength::HighZ => "vpiHiZ",
        Strength::Large => "vpiLargeCharge",
        Strength::Medium => "vpiMediumCharge",
        Strength::Small => "vpiSmallCharge",
        Strength::Unspecified | Strength::Unsupported => default,
    };
    (code(strength0), code(strength1))
}

/// The VPI flags of a net's or port's declaration qualifiers.
fn vpi_qualifiers(qualifiers: crate::core::db::DeclarationQualifiers) -> IrVpiQualifiers {
    IrVpiQualifiers {
        scalared: qualifiers.scalared,
        vectored: qualifiers.vectored,
        explicit_name: qualifiers.explicit_port_name,
        by_name: qualifiers.connected_by_name,
    }
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

/// `vpiPrimType` code name and gate keyword of a primitive (37.31, 37.32).
fn vpi_prim_type(prim_type: PrimitiveType) -> Option<(&'static str, &'static str)> {
    Some(match prim_type {
        PrimitiveType::And => ("vpiAndPrim", "and"),
        PrimitiveType::Nand => ("vpiNandPrim", "nand"),
        PrimitiveType::Nor => ("vpiNorPrim", "nor"),
        PrimitiveType::Or => ("vpiOrPrim", "or"),
        PrimitiveType::Xor => ("vpiXorPrim", "xor"),
        PrimitiveType::Xnor => ("vpiXnorPrim", "xnor"),
        PrimitiveType::Buf => ("vpiBufPrim", "buf"),
        PrimitiveType::Not => ("vpiNotPrim", "not"),
        PrimitiveType::Bufif0 => ("vpiBufif0Prim", "bufif0"),
        PrimitiveType::Bufif1 => ("vpiBufif1Prim", "bufif1"),
        PrimitiveType::Notif0 => ("vpiNotif0Prim", "notif0"),
        PrimitiveType::Notif1 => ("vpiNotif1Prim", "notif1"),
        PrimitiveType::Nmos => ("vpiNmosPrim", "nmos"),
        PrimitiveType::Pmos => ("vpiPmosPrim", "pmos"),
        PrimitiveType::Cmos => ("vpiCmosPrim", "cmos"),
        PrimitiveType::Rnmos => ("vpiRnmosPrim", "rnmos"),
        PrimitiveType::Rpmos => ("vpiRpmosPrim", "rpmos"),
        PrimitiveType::Rcmos => ("vpiRcmosPrim", "rcmos"),
        PrimitiveType::Rtran => ("vpiRtranPrim", "rtran"),
        PrimitiveType::Rtranif0 => ("vpiRtranif0Prim", "rtranif0"),
        PrimitiveType::Rtranif1 => ("vpiRtranif1Prim", "rtranif1"),
        PrimitiveType::Tran => ("vpiTranPrim", "tran"),
        PrimitiveType::Tranif0 => ("vpiTranif0Prim", "tranif0"),
        PrimitiveType::Tranif1 => ("vpiTranif1Prim", "tranif1"),
        PrimitiveType::Pullup => ("vpiPullupPrim", "pullup"),
        PrimitiveType::Pulldown => ("vpiPulldownPrim", "pulldown"),
        PrimitiveType::Sequential => ("vpiSeqPrim", ""),
        PrimitiveType::Combinational => ("vpiCombPrim", ""),
        PrimitiveType::Unsupported => return None,
    })
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
    fn primitive_types_name_their_annex_k_codes_and_keywords() {
        use super::vpi_prim_type;
        use crate::core::db::PrimitiveType;
        assert_eq!(
            vpi_prim_type(PrimitiveType::And),
            Some(("vpiAndPrim", "and"))
        );
        assert_eq!(
            vpi_prim_type(PrimitiveType::Tranif1),
            Some(("vpiTranif1Prim", "tranif1"))
        );
        assert_eq!(
            vpi_prim_type(PrimitiveType::Pulldown),
            Some(("vpiPulldownPrim", "pulldown"))
        );
        // A UDP is named by its declaration, not a keyword (37.32 detail 2).
        assert_eq!(
            vpi_prim_type(PrimitiveType::Sequential),
            Some(("vpiSeqPrim", ""))
        );
        assert_eq!(vpi_prim_type(PrimitiveType::Unsupported), None);
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
