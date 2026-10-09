//! Storage.

use super::*;

/// One lowered signal (or real companion): a global `sv4_t`/`double`.
#[derive(Clone, Debug, PartialEq)]
pub struct IrSignal {
    pub(in crate::sim) fixed_default: Option<IrConst>,
    pub(in crate::sim) c_name: String,
    /// Original HDL hierarchy, with ASCII unit-separator bytes between path
    /// components. `None` marks synthesized storage that must not be
    /// waveform-visible (compiler-internal state).
    pub(in crate::sim) hdl_name: Option<String>,
    pub(in crate::sim) ty: IrType,
    /// For members of a collapsed inout-net group: `(group index, driver
    /// slot)`.  `c_name` is then `<net>.resolved`.
    pub(in crate::sim) net_driver: Option<(usize, usize)>,
    /// Bit-level canonical network memberships for a true SystemVerilog net
    /// alias. Each entry identifies one signal bit and the resolved group bit
    /// that owns its electrical value.
    pub(in crate::sim) net_alias: Vec<IrNetAliasBinding>,
    /// Canonical variable storage for a reference alias; never another alias.
    pub(in crate::sim) alias: Option<usize>,
    /// Storage pruning marker (`unused_storage` pass): the declaration is
    /// skipped when set.  Indices are NEVER remapped.
    pub(in crate::sim) omit: bool,
}

impl IrSignal {
    pub fn new(
        c_name: String,
        hdl_name: Option<String>,
        ty: IrType,
        net_driver: Option<(usize, usize)>,
    ) -> Result<Self, IrValidationError> {
        if let IrType::Packed { width, .. } = ty {
            validate_width("signal.ty", width)?;
        }
        Ok(Self {
            c_name,
            hdl_name,
            fixed_default: None,
            ty,
            net_driver,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        })
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn hdl_name(&self) -> Option<&str> {
        self.hdl_name.as_deref()
    }
    pub fn ty(&self) -> IrType {
        self.ty
    }
    pub fn net_driver(&self) -> Option<(usize, usize)> {
        self.net_driver
    }
    pub fn net_alias(&self) -> &[IrNetAliasBinding] {
        &self.net_alias
    }
    pub fn is_omitted(&self) -> bool {
        self.omit
    }
}

/// One bit-level membership in a canonical true-net-alias network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrNetAliasBinding {
    pub(in crate::sim) group: usize,
    pub(in crate::sim) slot: usize,
    pub(in crate::sim) signal_bit: u32,
    pub(in crate::sim) group_bit: u32,
}

impl IrNetAliasBinding {
    pub fn group(&self) -> usize {
        self.group
    }

    pub fn signal_bit(&self) -> u32 {
        self.signal_bit
    }

    pub fn slot(&self) -> usize {
        self.slot
    }

    pub fn group_bit(&self) -> u32 {
        self.group_bit
    }
}

/// Equal-strength resolution rule for a simulated net group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrNetKind {
    Wire,
    Wand,
    Wor,
    Tri0,
    Tri1,
    Supply0,
    Supply1,
}

impl IrNetKind {
    /// Packed fill state (0, 1 or 3 = Z) of a net of this kind with no
    /// active driver: pull and supply nets hold their constant value.
    pub const fn undriven_fill(self) -> u8 {
        match self {
            Self::Tri0 | Self::Supply0 => 0,
            Self::Tri1 | Self::Supply1 => 1,
            Self::Wire | Self::Wand | Self::Wor => 3,
        }
    }

    pub const fn c_value(self) -> &'static str {
        match self {
            Self::Wire => "LLG_RESOLVE_WIRE",
            Self::Wand => "LLG_RESOLVE_WAND",
            Self::Wor => "LLG_RESOLVE_WOR",
            Self::Tri0 => "LLG_RESOLVE_TRI0",
            Self::Tri1 => "LLG_RESOLVE_TRI1",
            Self::Supply0 => "LLG_RESOLVE_SUPPLY0",
            Self::Supply1 => "LLG_RESOLVE_SUPPLY1",
        }
    }
}

/// A resolved net group: either one collapsed inout net with a driver slot
/// per member, or one wired net with a slot per continuous-assignment site.
#[derive(Clone, Debug, PartialEq)]
pub struct IrNetGroup {
    /// C name of the `llg_net_t` global (e.g. `g_net_0`); driver cells are
    /// `{c_name}__cells[i]`.
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) kind: IrNetKind,
    pub(in crate::sim) n_drivers: usize,
    /// Per-slot `(strength0, strength1)` levels on the IEEE 1800 strength
    /// scale (high impedance 0 through supply 7). Ordinary unspecified
    /// continuous assignments use strong/strong (6, 6).
    pub(in crate::sim) driver_strengths: Vec<(u8, u8)>,
    /// Optional propagation delay applied after all driver slots resolve.
    pub(in crate::sim) propagation_delay: Option<IrTransitionDelay>,
    /// Two-state signal holding the resolved strength view (eight bits per
    /// net bit), present only when a `%v` consumer reads this net.
    pub(in crate::sim) strength_view: Option<usize>,
}

impl IrNetGroup {
    pub fn new(
        c_name: String,
        width: u32,
        signed: bool,
        kind: IrNetKind,
        n_drivers: usize,
    ) -> Result<Self, IrValidationError> {
        validate_width("net_group.width", width)?;
        if n_drivers == 0 {
            return Err(IrValidationError::new(
                "net_group.n_drivers",
                "net group has no drivers",
            ));
        }
        if n_drivers > LLG_MAX_NET_DRIVERS {
            return Err(IrValidationError::new(
                "net_group.n_drivers",
                "net group driver count exceeds the runtime integer range",
            ));
        }
        Ok(Self {
            c_name,
            width,
            signed,
            kind,
            n_drivers,
            driver_strengths: vec![(6, 6); n_drivers],
            propagation_delay: None,
            strength_view: None,
        })
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn kind(&self) -> IrNetKind {
        self.kind
    }
    pub fn driver_count(&self) -> usize {
        self.n_drivers
    }

    pub fn propagation_delay(&self) -> Option<IrTransitionDelay> {
        self.propagation_delay
    }
}

/// Consecutive net-array cells that no structural driver, alias, selected
/// connection or force target reaches. Each cell (and every whole-array inout
/// peer of it) is an undriven net of `kind`, so its value is the constant
/// `kind.undriven_fill()` and it needs no electrical object or generated code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrNetCellRun {
    pub(in crate::sim) first: u64,
    pub(in crate::sim) count: u64,
    pub(in crate::sim) kind: IrNetKind,
}

/// Electrical layout of a net array. Every cell is either one entry of
/// `IrArray::net_elements` or covered by exactly one of `constant_cells`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrNetArray {
    /// Ascending, disjoint runs of undriven cells.
    pub(in crate::sim) constant_cells: Vec<IrNetCellRun>,
}

/// A lowered unpacked array: flat `sv4_t` storage plus linearization data.
#[derive(Clone, Debug, PartialEq)]
pub struct IrArray {
    /// Lexical activation storage, declared by `FixedArrayDeclare`.
    pub(in crate::sim) activation: bool,
    /// Descriptor storage regardless of extent: a column of a column-layout
    /// record (RTL-101), so record values move one descriptor per column.
    pub(in crate::sim) descriptor: bool,
    /// Present for a net array (dense storage published from resolved nets).
    pub(in crate::sim) net: Option<IrNetArray>,
    /// Array cells that observe a canonical resolved net signal.
    pub(in crate::sim) net_elements: Vec<(u64, usize)>,
    /// Typed default for a fixed aggregate element, before declaration initialization.
    pub(in crate::sim) element_default: Option<IrConst>,
    /// Default-uninitialized element value read through an invalid index
    /// (SV 7.4.6). Differs from `packed_default` only for mixed-state
    /// aggregate elements; member initializers are excluded.
    pub(in crate::sim) element_uninitialized: Option<IrConst>,
    pub(in crate::sim) c_name: String,
    /// Original HDL hierarchical name (before C-identifier sanitization).
    pub(in crate::sim) hdl_name: String,
    pub(in crate::sim) elem_width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) two_state: bool,
    /// Native real elements use `double` storage rather than `sv4_t`.
    pub(in crate::sim) real: bool,
    /// `true` for shortreal elements; writes round through a C float.
    pub(in crate::sim) shortreal: bool,
    /// `(left, right)` per declared dimension, in declaration order.
    pub(in crate::sim) dims: Vec<(i32, i32)>,
    /// Total element count (product of dimension sizes).
    pub(in crate::sim) total: u64,
}

impl IrArray {
    pub fn new(
        c_name: String,
        hdl_name: String,
        elem_width: u32,
        signed: bool,
        dims: Vec<(i32, i32)>,
    ) -> Result<Self, IrValidationError> {
        validate_width("array.elem_width", elem_width)?;
        if dims.is_empty() {
            return Err(IrValidationError::new(
                "array.dims",
                "array has no dimensions",
            ));
        }
        let mut total = 1u64;
        for (index, (left, right)) in dims.iter().copied().enumerate() {
            let extent = (i64::from(left) - i64::from(right))
                .unsigned_abs()
                .checked_add(1)
                .ok_or_else(|| {
                    IrValidationError::new(
                        format!("array.dims[{index}]"),
                        "dimension extent overflows u64",
                    )
                })?;
            total = total.checked_mul(extent).ok_or_else(|| {
                IrValidationError::new(
                    format!("array.dims[{index}]"),
                    "dimension product overflows u64",
                )
            })?;
        }
        if total > LLG_MAX_FIXED_ARRAY_CELLS {
            return Err(IrValidationError::new(
                "array.dims",
                format!(
                    "fixed-array cell count {total} exceeds selected cell-wise storage limit {LLG_MAX_FIXED_ARRAY_CELLS}"
                ),
            ));
        }
        Ok(Self {
            c_name,
            hdl_name,
            activation: false,
            descriptor: false,
            net: None,
            net_elements: Vec::new(),
            element_default: None,
            element_uninitialized: None,
            elem_width,
            signed,
            two_state: false,
            real: false,
            shortreal: false,
            dims,
            total,
        })
    }

    /// Value read through an invalid index and streamed for a `with`
    /// selection outside the bounds: the element's default-uninitialized
    /// value, keeping two-state members of aggregate elements at zero.
    pub(in crate::sim) fn element_fallback(&self) -> IrConst {
        self.element_uninitialized
            .clone()
            .unwrap_or_else(|| IrConst::integral_default(self.elem_width, self.two_state))
    }

    /// Net arrays keep dense storage: each published cell is the target of
    /// its electrical view.
    pub(in crate::sim) fn is_net(&self) -> bool {
        self.net.is_some() || !self.net_elements.is_empty()
    }

    pub(in crate::sim) fn sparse(&self) -> bool {
        !self.real
            && !self.is_net()
            && (self.activation
                || self.descriptor
                || self.total > LLG_DENSE_FIXED_ARRAY_CELLS
                || self
                    .total
                    .checked_mul(u64::from(self.elem_width))
                    .is_none_or(|width| width > u64::from(crate::sim::emit_c::LLG_MAX_WIDTH)))
    }

    pub(in crate::sim) fn cell_address(&self, index: &str) -> String {
        if self.sparse() {
            format!("llg_fixed_array_cell(&{}, {index})", self.c_name)
        } else {
            format!("&{}[{index}]", self.c_name)
        }
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn hdl_name(&self) -> &str {
        &self.hdl_name
    }
    pub fn elem_width(&self) -> u32 {
        self.elem_width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn dims(&self) -> &[(i32, i32)] {
        &self.dims
    }
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Return the source spelling of one flattened element using each
    /// declaration's actual left/right bounds.  The flat order is row-major
    /// with the leftmost dimension slowest, matching Verilog indexing.
    pub fn waveform_element_name(&self, index: u64) -> Option<String> {
        if index >= self.total {
            return None;
        }
        let mut remainder = index;
        let mut indices = vec![0i64; self.dims.len()];
        for dimension in (0..self.dims.len()).rev() {
            let (left, right) = self.dims[dimension];
            let extent = (i64::from(left) - i64::from(right))
                .unsigned_abs()
                .checked_add(1)?;
            let offset = remainder % extent;
            remainder /= extent;
            let offset = i64::try_from(offset).ok()?;
            indices[dimension] = if left >= right {
                i64::from(left) - offset
            } else {
                i64::from(left) + offset
            };
        }
        let mut name = self.hdl_name.clone();
        for index in indices {
            name.push('[');
            name.push_str(&index.to_string());
            name.push(']');
        }
        Some(name)
    }
}
