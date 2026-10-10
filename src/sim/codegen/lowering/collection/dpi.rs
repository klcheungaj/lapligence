//! DPI-C import signatures (SV 35.5, Annex H): the foreign C-layer type of
//! each formal and result, and per-call specializations of open-array
//! imports (SV 35.5.6.1, H.12).
//!
//! Every formal keeps its internal transport: scalars as before, packed
//! values and sized unpacked aggregates as one packed payload. The generated
//! thunk converts that payload to the canonical or C-compatible foreign
//! layout recorded here. An open-array formal takes its unsized ranges from
//! the call's actual, so each distinct actual shape gets its own thunk whose
//! formal is the actual's payload.

use super::*;
use crate::core::db::DpiOpenType;
use crate::sim::ir::{IrDpiImport, IrDpiType};

/// Foreign representation of a declared DPI formal or member type.
pub(super) fn dpi_type_of(descriptor: &TypeDescriptor) -> Result<IrDpiType, String> {
    let packed = || {
        let width = descriptor
            .info
            .width
            .filter(|width| *width != 0)
            .ok_or_else(|| format!("DPI-C type `{}` has no packed width", descriptor.name))?;
        if width > LLG_MAX_WIDTH {
            return Err(format!(
                "DPI-C type `{}` is {width} bits wide; the runtime supports at most {LLG_MAX_WIDTH}",
                descriptor.name
            ));
        }
        Ok(IrDpiType::Vector {
            width,
            logic: !descriptor.two_state,
        })
    };
    match &descriptor.shape {
        TypeShape::PackedAtom { ranges } => {
            let width = descriptor.info.width.unwrap_or(0);
            // A type without a packed range is a scalar or an integer atom
            // (also as an enumeration base, H.7.3); only the 2-state atoms
            // map to C integers (Table H.1). `integer` and `time` are 4-state
            // packed arrays in canonical form.
            if ranges.is_empty() && width == 1 {
                return Ok(if descriptor.two_state {
                    IrDpiType::Bit
                } else {
                    IrDpiType::Logic
                });
            }
            if ranges.is_empty() && descriptor.two_state && matches!(width, 8 | 16 | 32 | 64) {
                return Ok(IrDpiType::Int {
                    bytes: (width / 8) as u8,
                    signed: descriptor.info.signed,
                });
            }
            packed()
        }
        TypeShape::Real { shortreal: true } => Ok(IrDpiType::ShortReal),
        TypeShape::Real { shortreal: false } => Ok(IrDpiType::Real),
        TypeShape::String => Ok(IrDpiType::String),
        TypeShape::Opaque { kind } if kind == "Chandle" => Ok(IrDpiType::Chandle),
        TypeShape::Aggregate(layout) => match layout.kind {
            AggregateKind::PackedStruct | AggregateKind::PackedUnion => packed(),
            AggregateKind::UnpackedStruct => layout
                .members
                .iter()
                .map(|member| dpi_type_of(&member.descriptor))
                .collect::<Result<Vec<_>, _>>()
                .map(IrDpiType::Struct),
            AggregateKind::TaggedUnion if descriptor.info.width.is_some() => packed(),
            _ => Err(format!(
                "DPI-C type `{}`: only packed unions may cross the DPI (SV 35.5.6)",
                descriptor.name
            )),
        },
        TypeShape::FixedArray {
            dimensions,
            element,
        } => Ok(IrDpiType::Array {
            dims: dimensions.clone(),
            element: Box::new(dpi_type_of(element)?),
        }),
        TypeShape::Container { .. } => Err(format!(
            "DPI-C type `{}`: dynamic arrays, queues and associative arrays are not DPI formal types (SV 35.5.6); use an open array",
            descriptor.name
        )),
        TypeShape::Opaque { .. } => Err(format!(
            "DPI-C type `{}` is not a permitted DPI formal type (SV 35.5.6)",
            descriptor.name
        )),
    }
}

/// The template type of an open-array formal.
pub(super) fn dpi_open_template(open: &DpiOpenType) -> Result<IrDpiType, String> {
    let element = if open.packed_open {
        IrDpiType::OpenPacked {
            logic: !open.element.two_state,
        }
    } else {
        dpi_type_of(&open.element)?
    };
    Ok(IrDpiType::Open {
        dims: open.unpacked.clone(),
        element: Box::new(element),
    })
}

/// Whether a type fits the packed payload transport: every leaf has a packed
/// width (no real, string or chandle inside an aggregate).
fn payload_only(ty: &IrDpiType) -> bool {
    match ty {
        IrDpiType::Array { element, .. } | IrDpiType::Open { element, .. } => payload_only(element),
        IrDpiType::Struct(members) => members.iter().all(payload_only),
        IrDpiType::Real | IrDpiType::ShortReal | IrDpiType::Chandle | IrDpiType::String => false,
        _ => true,
    }
}

impl Codegen<'_> {
    /// Foreign types of the declared formals of DPI import `ft`, after
    /// checking that each one's internal transport can carry it.
    pub(super) fn dpi_formal_types(
        &self,
        ft: NodeId,
        formals: &[(NodeId, bool)],
        formals_ir: &[IrFormal],
    ) -> Result<Vec<IrDpiType>, String> {
        let name = &self.node(ft).name;
        let mut types = Vec::with_capacity(formals.len());
        for (index, ((io, _), ir)) in formals.iter().zip(formals_ir).enumerate() {
            let formal_name = &self.node(*io).name;
            let ty = if let Some(open) = self.db.dpi_open_type(*io) {
                dpi_open_template(open)?
            } else {
                let descriptor = self.query_descriptor(*io).ok_or_else(|| {
                    format!("DPI-C import `{name}` formal {index} (`{formal_name}`) has no captured type")
                })?;
                dpi_type_of(descriptor).map_err(|error| {
                    format!("DPI-C import `{name}` formal {index} (`{formal_name}`): {error}")
                })?
            };
            if matches!(
                ty,
                IrDpiType::Array { .. } | IrDpiType::Struct(_) | IrDpiType::Open { .. }
            ) {
                if !payload_only(&ty) {
                    return Err(format!(
                        "DPI-C import `{name}` formal {index} (`{formal_name}`): unpacked aggregates with real, shortreal, string or chandle elements are not supported (SIM-040)"
                    ));
                }
                if ir.fixed_array.is_some()
                    || ir.native_value.is_some()
                    || ir.real_array.is_some()
                    || ir.container.is_some()
                {
                    return Err(format!(
                        "DPI-C import `{name}` formal {index} (`{formal_name}`): an unpacked aggregate wider than {LLG_MAX_WIDTH} payload bits is not supported (SIM-040)"
                    ));
                }
                if let Some(width) = ty.payload_width() {
                    if width != u64::from(ir.width) {
                        return Err(format!(
                            "DPI-C import `{name}` formal {index} (`{formal_name}`): foreign layout covers {width} bits but the formal carries {}",
                            ir.width
                        ));
                    }
                }
            }
            types.push(ty);
        }
        Ok(types)
    }

    /// Foreign result type of DPI import function `ft`. Results are
    /// restricted to small values (SV 35.5.5); the frontend rejects others.
    pub(super) fn dpi_return_type(&self, ft: NodeId) -> Result<Option<IrDpiType>, String> {
        let NodeKind::FuncTask { is_task, ret, .. } = self.kind(ft) else {
            return Err("non-FuncTask passed to dpi_return_type".to_owned());
        };
        let Some(ty) = ret.as_ref().filter(|_| !*is_task) else {
            return Ok(None);
        };
        Ok(match ty.kind.as_str() {
            "void" => None,
            "string" => Some(IrDpiType::String),
            "chandle" => Some(IrDpiType::Chandle),
            "shortreal" => Some(IrDpiType::ShortReal),
            "real" => Some(IrDpiType::Real),
            _ => match self.dpi_return_info(ft)? {
                Some((1, _, true, _)) => Some(IrDpiType::Bit),
                Some((1, _, false, _)) => Some(IrDpiType::Logic),
                Some((width @ (8 | 16 | 32 | 64), signed, true, _)) => Some(IrDpiType::Int {
                    bytes: (width / 8) as u8,
                    signed,
                }),
                _ => {
                    return Err(format!(
                        "DPI-C import `{}` result type `{}` is not a small value (SV 35.5.5)",
                        self.node(ft).name,
                        ty.render()
                    ));
                }
            },
        })
    }

    /// Flatten the actual of an open-array formal into its unpacked
    /// dimensions (slowest first) and element.
    fn dpi_open_actual_shape(
        &self,
        formal: NodeId,
        actual: NodeId,
    ) -> Result<(Vec<(i32, i32)>, TypeDescriptor), String> {
        let descriptor = self.query_descriptor(actual).cloned().ok_or_else(|| {
            format!(
                "the actual of DPI-C open-array formal `{}` has no captured type",
                self.node(formal).name
            )
        })?;
        let mut dims = Vec::new();
        let mut current = descriptor;
        loop {
            match current.shape {
                TypeShape::FixedArray {
                    dimensions,
                    element,
                } => {
                    dims.extend(dimensions);
                    current = *element;
                }
                TypeShape::Container { .. } => {
                    return Err(format!(
                        "a dynamic array, queue or associative array actual of DPI-C open-array formal `{}` is not supported (SIM-040)",
                        self.node(formal).name
                    ));
                }
                _ => return Ok((dims, current)),
            }
        }
    }

    /// The call specialization of template type `template` for `actual`:
    /// unsized unpacked ranges become the actual's, an unsized packed
    /// dimension the actual's linearized width (H.7.6). Sized dimensions
    /// and the element keep the declaration (WYSIWYG, SV 35.6.1.1).
    pub(super) fn dpi_open_specialize(
        &self,
        formal: NodeId,
        template: &IrDpiType,
        actual: NodeId,
    ) -> Result<IrDpiType, String> {
        let IrDpiType::Open { dims, element } = template else {
            return Ok(template.clone());
        };
        let name = &self.node(formal).name;
        let (actual_dims, actual_element) = self.dpi_open_actual_shape(formal, actual)?;
        if actual_dims.len() < dims.len() {
            return Err(format!(
                "the actual of DPI-C open-array formal `{name}` has {} unpacked dimensions; the formal declares {}",
                actual_dims.len(),
                dims.len()
            ));
        }
        let mut concrete = Vec::with_capacity(dims.len());
        for (declared, actual_dim) in dims.iter().zip(&actual_dims) {
            let range = match declared {
                Some((left, right)) => {
                    if crate::sim::ir::dimension_size(*left, *right)
                        != crate::sim::ir::dimension_size(actual_dim.0, actual_dim.1)
                    {
                        return Err(format!(
                            "the actual of DPI-C open-array formal `{name}` does not match its sized dimension [{left}:{right}]"
                        ));
                    }
                    (*left, *right)
                }
                None => *actual_dim,
            };
            concrete.push(Some(range));
        }
        let rest = &actual_dims[dims.len()..];
        let element = match element.as_ref() {
            IrDpiType::OpenPacked { logic } => {
                if !rest.is_empty() {
                    return Err(format!(
                        "the actual of DPI-C open-array formal `{name}` has more unpacked dimensions than the formal"
                    ));
                }
                let width = actual_element
                    .info
                    .width
                    .filter(|width| *width != 0 && *width <= LLG_MAX_WIDTH)
                    .ok_or_else(|| {
                        format!(
                            "the actual of DPI-C packed open-array formal `{name}` has no packed width of at most {LLG_MAX_WIDTH} bits"
                        )
                    })?;
                IrDpiType::Vector {
                    width,
                    logic: *logic,
                }
            }
            declared => {
                let actual_ty = if rest.is_empty() {
                    dpi_type_of(&actual_element)?
                } else {
                    IrDpiType::Array {
                        dims: rest.to_vec(),
                        element: Box::new(dpi_type_of(&actual_element)?),
                    }
                };
                if actual_ty.payload_width() != declared.payload_width() {
                    return Err(format!(
                        "the actual of DPI-C open-array formal `{name}` has elements of a different size than the formal"
                    ));
                }
                declared.clone()
            }
        };
        Ok(IrDpiType::Open {
            dims: concrete,
            element: Box::new(element),
        })
    }

    /// Payload width and state domain of an open-array formal at one call.
    pub(super) fn dpi_open_payload(
        &self,
        formal: NodeId,
        open: &DpiOpenType,
        actual: NodeId,
    ) -> Result<(u32, bool), String> {
        let template = dpi_open_template(open)?;
        let concrete = self.dpi_open_specialize(formal, &template, actual)?;
        let width = concrete
            .payload_width()
            .filter(|width| *width != 0 && *width <= u64::from(LLG_MAX_WIDTH))
            .ok_or_else(|| {
                format!(
                    "the actual of DPI-C open-array formal `{}` exceeds the {LLG_MAX_WIDTH}-bit payload transport (SIM-040)",
                    self.node(formal).name
                )
            })?;
        // The formal's element type decides the coercion (SV 35.6.1); a
        // packed open dimension records it, a declared element its own state.
        let two_state = if open.packed_open {
            open.element.two_state
        } else {
            dpi_two_state(&concrete)
        };
        Ok((width as u32, two_state))
    }

    /// The function index to call for DPI import template `template` with
    /// the bound actuals: itself unless it has open-array formals, else the
    /// specialization for the actuals' shapes.
    pub(in super::super) fn dpi_open_callee(
        &mut self,
        template: usize,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
    ) -> Result<usize, String> {
        let Some(dpi) = self.model.funcs[template]
            .dpi
            .as_ref()
            .filter(|dpi| dpi.is_open_template())
            .cloned()
        else {
            return Ok(template);
        };
        let mut types = dpi.formals.clone();
        for (index, (formal, _)) in formals.iter().enumerate() {
            if types[index].is_unsized() {
                types[index] =
                    self.dpi_open_specialize(*formal, &types[index], bound[index].expr)?;
            }
        }
        let key = (template, format!("{types:?}"));
        if let Some(index) = self.dpi_specializations.get(&key) {
            return Ok(*index);
        }
        let mut function = self.model.funcs[template].clone();
        for (index, ir) in function.formals.iter_mut().enumerate().take(formals.len()) {
            if dpi.formals[index].is_unsized() {
                ir.width = bound[index].width;
                ir.two_state = bound[index].two_state;
                ir.signed = false;
            }
        }
        function.inline_expanded = false;
        function.c_name = format!("{}_open{}", function.c_name, self.dpi_specializations.len());
        function.dpi = Some(IrDpiImport::new(
            dpi.c_name.clone(),
            dpi.context,
            dpi.pure,
            types,
            dpi.ret.clone(),
        ));
        let index = self.model.funcs.len();
        self.model.funcs.push(function);
        self.dpi_specializations.insert(key, index);
        Ok(index)
    }
}

/// Whether every packed leaf of a payload type is 2-state.
fn dpi_two_state(ty: &IrDpiType) -> bool {
    match ty {
        IrDpiType::Bit | IrDpiType::Int { .. } => true,
        IrDpiType::Vector { logic, .. } | IrDpiType::OpenPacked { logic } => !logic,
        IrDpiType::Array { element, .. } | IrDpiType::Open { element, .. } => {
            dpi_two_state(element)
        }
        IrDpiType::Struct(members) => members.iter().all(dpi_two_state),
        _ => false,
    }
}
