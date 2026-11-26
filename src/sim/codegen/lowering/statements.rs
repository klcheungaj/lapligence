//! Procedural statement lowering through the shared emission context.

use super::objects::object_query;
use super::*;
use crate::core::db::ConcurrentAssertionKind;
use crate::sim::ir::{
    IrAssertionControlKind, IrContainerElement, IrElemSel, IrMemorySelector, IrObjectQuery,
    IrObjectStmt, IrStringExpr,
};

mod assertions;
mod assignments;
mod calls;
mod clocking;
mod context;
mod control_flow;
mod declarations;
mod dispatch;
mod drivers;
mod events;
mod forks;
mod formatting;
mod memory_tasks;
mod native_delays;
pub(super) mod nba_merge;
mod randsequence;
mod system_tasks;

pub(in crate::sim::codegen) use randsequence::RandseqState;

fn default_real_local_initializer(width: u32) -> Option<Box<IrExpr>> {
    (width == 0).then(|| {
        Box::new(IrExpr::new(
            IrExprKind::Const(IrConst::real(0.0)),
            0,
            false,
            None,
        ))
    })
}

fn lower_unique_priority_check(
    check: crate::core::db::UniquePriorityCheck,
    origin: crate::sim::semantic::Origin,
) -> IrUniquePriorityCheck {
    match check {
        crate::core::db::UniquePriorityCheck::None => IrUniquePriorityCheck::None,
        crate::core::db::UniquePriorityCheck::Unique => IrUniquePriorityCheck::Unique(origin),
        crate::core::db::UniquePriorityCheck::Unique0 => IrUniquePriorityCheck::Unique0(origin),
        crate::core::db::UniquePriorityCheck::Priority => IrUniquePriorityCheck::Priority(origin),
        crate::core::db::UniquePriorityCheck::Unsupported => IrUniquePriorityCheck::None,
    }
}

#[derive(Clone, Copy)]
enum DisplayTaskKind {
    Immediate { newline: bool, file: bool },
    Deferred { strobe: bool, file: bool },
}

struct LoweredCallReceiver {
    class: Option<IrChandleExpr>,
    virtual_interface: Option<crate::sim::ir::IrVirtualCall>,
}

/// Whether `name` is a display task that formats its arguments while its
/// statement runs (`$display`, `$write`, `$fdisplay`, ...), not later.
pub(in crate::sim::codegen) fn immediate_display_task(name: &str) -> bool {
    matches!(
        display_task_variant(name),
        Some((DisplayTaskKind::Immediate { .. }, _))
    )
}

/// Whether `name` is a display task that formats its arguments later
/// (`$strobe`, `$monitor`, ...), reading them again then.
pub(in crate::sim::codegen) fn deferred_display_task(name: &str) -> bool {
    matches!(
        display_task_variant(name),
        Some((DisplayTaskKind::Deferred { .. }, _))
    )
}

/// Whether `name` is a task that prints while its statement runs and lets the
/// run continue: an immediate display task, `$info`, `$warning` or `$error`.
pub(in crate::sim::codegen) fn immediate_output_task(name: &str) -> bool {
    immediate_display_task(name)
        || severity_task_variant(name).is_some_and(|level| level != IrSeverityLevel::Fatal)
}

fn display_task_variant(name: &str) -> Option<(DisplayTaskKind, IrDisplayRadix)> {
    let variant = match name {
        "$display" => (
            DisplayTaskKind::Immediate {
                newline: true,
                file: false,
            },
            IrDisplayRadix::Decimal,
        ),
        "$displayb" => (
            DisplayTaskKind::Immediate {
                newline: true,
                file: false,
            },
            IrDisplayRadix::Binary,
        ),
        "$displayo" => (
            DisplayTaskKind::Immediate {
                newline: true,
                file: false,
            },
            IrDisplayRadix::Octal,
        ),
        "$displayh" => (
            DisplayTaskKind::Immediate {
                newline: true,
                file: false,
            },
            IrDisplayRadix::Hex,
        ),
        "$write" => (
            DisplayTaskKind::Immediate {
                newline: false,
                file: false,
            },
            IrDisplayRadix::Decimal,
        ),
        "$writeb" => (
            DisplayTaskKind::Immediate {
                newline: false,
                file: false,
            },
            IrDisplayRadix::Binary,
        ),
        "$writeo" => (
            DisplayTaskKind::Immediate {
                newline: false,
                file: false,
            },
            IrDisplayRadix::Octal,
        ),
        "$writeh" => (
            DisplayTaskKind::Immediate {
                newline: false,
                file: false,
            },
            IrDisplayRadix::Hex,
        ),
        "$strobe" => (
            DisplayTaskKind::Deferred {
                strobe: true,
                file: false,
            },
            IrDisplayRadix::Decimal,
        ),
        "$strobeb" => (
            DisplayTaskKind::Deferred {
                strobe: true,
                file: false,
            },
            IrDisplayRadix::Binary,
        ),
        "$strobeo" => (
            DisplayTaskKind::Deferred {
                strobe: true,
                file: false,
            },
            IrDisplayRadix::Octal,
        ),
        "$strobeh" => (
            DisplayTaskKind::Deferred {
                strobe: true,
                file: false,
            },
            IrDisplayRadix::Hex,
        ),
        "$monitor" => (
            DisplayTaskKind::Deferred {
                strobe: false,
                file: false,
            },
            IrDisplayRadix::Decimal,
        ),
        "$monitorb" => (
            DisplayTaskKind::Deferred {
                strobe: false,
                file: false,
            },
            IrDisplayRadix::Binary,
        ),
        "$monitoro" => (
            DisplayTaskKind::Deferred {
                strobe: false,
                file: false,
            },
            IrDisplayRadix::Octal,
        ),
        "$monitorh" => (
            DisplayTaskKind::Deferred {
                strobe: false,
                file: false,
            },
            IrDisplayRadix::Hex,
        ),
        "$fdisplay" => (
            DisplayTaskKind::Immediate {
                newline: true,
                file: true,
            },
            IrDisplayRadix::Decimal,
        ),
        "$fdisplayb" => (
            DisplayTaskKind::Immediate {
                newline: true,
                file: true,
            },
            IrDisplayRadix::Binary,
        ),
        "$fdisplayo" => (
            DisplayTaskKind::Immediate {
                newline: true,
                file: true,
            },
            IrDisplayRadix::Octal,
        ),
        "$fdisplayh" => (
            DisplayTaskKind::Immediate {
                newline: true,
                file: true,
            },
            IrDisplayRadix::Hex,
        ),
        "$fwrite" => (
            DisplayTaskKind::Immediate {
                newline: false,
                file: true,
            },
            IrDisplayRadix::Decimal,
        ),
        "$fwriteb" => (
            DisplayTaskKind::Immediate {
                newline: false,
                file: true,
            },
            IrDisplayRadix::Binary,
        ),
        "$fwriteo" => (
            DisplayTaskKind::Immediate {
                newline: false,
                file: true,
            },
            IrDisplayRadix::Octal,
        ),
        "$fwriteh" => (
            DisplayTaskKind::Immediate {
                newline: false,
                file: true,
            },
            IrDisplayRadix::Hex,
        ),
        "$fstrobe" => (
            DisplayTaskKind::Deferred {
                strobe: true,
                file: true,
            },
            IrDisplayRadix::Decimal,
        ),
        "$fstrobeb" => (
            DisplayTaskKind::Deferred {
                strobe: true,
                file: true,
            },
            IrDisplayRadix::Binary,
        ),
        "$fstrobeo" => (
            DisplayTaskKind::Deferred {
                strobe: true,
                file: true,
            },
            IrDisplayRadix::Octal,
        ),
        "$fstrobeh" => (
            DisplayTaskKind::Deferred {
                strobe: true,
                file: true,
            },
            IrDisplayRadix::Hex,
        ),
        "$fmonitor" => (
            DisplayTaskKind::Deferred {
                strobe: false,
                file: true,
            },
            IrDisplayRadix::Decimal,
        ),
        "$fmonitorb" => (
            DisplayTaskKind::Deferred {
                strobe: false,
                file: true,
            },
            IrDisplayRadix::Binary,
        ),
        "$fmonitoro" => (
            DisplayTaskKind::Deferred {
                strobe: false,
                file: true,
            },
            IrDisplayRadix::Octal,
        ),
        "$fmonitorh" => (
            DisplayTaskKind::Deferred {
                strobe: false,
                file: true,
            },
            IrDisplayRadix::Hex,
        ),
        _ => return None,
    };
    Some(variant)
}

fn severity_task_variant(name: &str) -> Option<IrSeverityLevel> {
    match name {
        "$info" => Some(IrSeverityLevel::Info),
        "$warning" => Some(IrSeverityLevel::Warning),
        "$error" => Some(IrSeverityLevel::Error),
        "$fatal" => Some(IrSeverityLevel::Fatal),
        _ => None,
    }
}

fn loop_index_expr(value: i32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![value as u32 as u64],
            x: vec![0],
            z: vec![0],
            width: 32,
            signed: true,
            real: None,
            fill: None,
        }),
        32,
        true,
        None,
    )
}

/// How a message names `array`; a static class property has no hierarchical
/// name.
fn array_phrase(array: &crate::sim::ir::IrArray) -> String {
    if array.hdl_name.is_empty() {
        "a static class property".to_owned()
    } else {
        format!("`{}`", array.hdl_name.replace('\u{1f}', "."))
    }
}

fn force_constant_index(expr: &IrExpr) -> bool {
    let IrExprKind::Const(value) = expr.kind() else {
        return false;
    };
    value.real_value().is_none()
        && value.fill().is_none()
        && value.x_mask().iter().all(|mask| *mask == 0)
        && value.z_mask().iter().all(|mask| *mask == 0)
}

/// The integer value of a lowered net selector. Lowering normalizes declared
/// ranges with conversions and `+`/`-`/`*` of constants; the frontend has
/// already required the source selector to be a constant expression.
pub(super) fn force_constant_i64(expr: &IrExpr) -> Option<i64> {
    let value = match expr.kind() {
        IrExprKind::Const(value) => {
            if value.real_value().is_some()
                || value.fill().is_some()
                || value.x_mask().iter().any(|mask| *mask != 0)
                || value.z_mask().iter().any(|mask| *mask != 0)
                || value.bits().iter().skip(1).any(|limb| *limb != 0)
                || value.width() == 0
                || value.width() > 64
            {
                return None;
            }
            let bits = value.bits().first().copied().unwrap_or(0);
            let width = value.width();
            if value.signed() && width < 64 && bits >> (width - 1) & 1 != 0 {
                (bits | (u64::MAX << width)) as i64
            } else {
                i64::try_from(bits).ok()?
            }
        }
        IrExprKind::Convert { a } => force_constant_i64(a)?,
        IrExprKind::Bin { op, a, b } => {
            let (a, b) = (force_constant_i64(a)?, force_constant_i64(b)?);
            match op {
                IrBinOp::Add => a.checked_add(b)?,
                IrBinOp::Sub => a.checked_sub(b)?,
                IrBinOp::Mul => a.checked_mul(b)?,
                _ => return None,
            }
        }
        _ => return None,
    };
    // A narrower result type would truncate; leave such selectors alone.
    let width = expr.width();
    let fits = width >= 64
        || (width > 0
            && if expr.signed() {
                value >= -(1i64 << (width - 1)) && value < (1i64 << (width - 1))
            } else {
                value >= 0 && value < (1i64 << width)
            });
    fits.then_some(value)
}

/// Rewrite a constant indexed part-select or a constant packed member/element
/// selection of a vector net into the fixed part descriptor the force runtime
/// overlays (IEEE 1800-2009 10.6.2 admits "a constant bit-select of a vector
/// net, a constant part-select of a vector net, or a concatenation of
/// these"). Variables and non-constant selectors are returned unchanged, so
/// `validate_force_lhs` keeps rejecting them.
fn normalize_force_lhs(model: &IrModel, lhs: IrLhs) -> IrLhs {
    let is_net = |index: usize| {
        model
            .signals
            .get(index)
            .is_some_and(|signal| signal.net_driver.is_some() || !signal.net_alias.is_empty())
    };
    match lhs {
        // A whole net array or a selected row lowers to a stream of constant
        // element lvalues; each names one net cell, forced through that
        // cell's electrical signal like `force na[i]` (10.6.2 "a net").
        IrLhs::ArrayElem {
            arr,
            indices,
            elem_sel: IrElemSel::Whole,
        } => match net_array_cell(model, arr, &indices) {
            Some(signal) => IrLhs::Whole(signal),
            None => IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel: IrElemSel::Whole,
            },
        },
        IrLhs::IdxPart(index, base, width_expr, width, negative, two_state) => {
            let span = i64::from(width) - 1;
            let bounds = force_constant_i64(&base)
                .filter(|_| is_net(index) && width > 0)
                .and_then(|base| {
                    if negative {
                        Some((base, base.checked_sub(span)?))
                    } else {
                        Some((base.checked_add(span)?, base))
                    }
                });
            match bounds {
                Some((left, right)) => IrLhs::Part(index, left, right, two_state),
                None => IrLhs::IdxPart(index, base, width_expr, width, negative, two_state),
            }
        }
        IrLhs::PackedSelect {
            target,
            steps,
            signed,
            two_state,
        } => {
            let constant_part = match target.as_ref() {
                IrLhs::Whole(index) if is_net(*index) => {
                    let mut available = i64::from(model.signal(*index).ty.width());
                    let mut lsb = 0i64;
                    let mut selected = None;
                    for step in &steps {
                        let width = i64::from(step.width);
                        match force_constant_i64(&step.base) {
                            Some(base) if base >= 0 && width > 0 && base + width <= available => {
                                lsb += base;
                                available = width;
                                selected = Some((*index, lsb + width - 1, lsb));
                            }
                            _ => {
                                selected = None;
                                break;
                            }
                        }
                    }
                    selected
                }
                _ => None,
            };
            match constant_part {
                Some((index, left, right)) => IrLhs::Part(index, left, right, two_state),
                None => IrLhs::PackedSelect {
                    target,
                    steps,
                    signed,
                    two_state,
                },
            }
        }
        IrLhs::Stream {
            parts,
            width,
            slice,
            direction,
        } => IrLhs::Stream {
            parts: parts
                .into_iter()
                .map(|(part, width)| (normalize_force_lhs(model, part), width))
                .collect(),
            width,
            slice,
            direction,
        },
        other => other,
    }
}

/// A record variable held as one variable signal per leaf, concatenated.
fn record_leaves(model: &IrModel, lhs: &IrLhs) -> bool {
    let IrLhs::Stream { parts, .. } = lhs else {
        return false;
    };
    !parts.is_empty()
        && parts.iter().all(|(part, _)| match part {
            IrLhs::Whole(index) | IrLhs::Part(index, ..) => {
                model.signals.get(*index).is_some_and(|signal| {
                    signal.net_driver.is_none()
                        && signal.net_alias.is_empty()
                        && !matches!(signal.ty, IrType::Real { .. })
                })
            }
            _ => false,
        })
}

/// The electrical signal of the net-array cell at constant `indices`.
fn net_array_cell(model: &IrModel, array: usize, indices: &[IrExpr]) -> Option<usize> {
    let array = model.arrays.get(array)?;
    if !array.is_net() || indices.len() != array.dims.len() {
        return None;
    }
    let mut linear = 0u64;
    for ((left, right), index) in array.dims.iter().zip(indices) {
        let index = force_constant_i64(index)?;
        let (left, right) = (i64::from(*left), i64::from(*right));
        if index < left.min(right) || index > left.max(right) {
            return None;
        }
        let offset = if left >= right {
            left - index
        } else {
            index - left
        };
        let extent = (left - right).unsigned_abs() + 1;
        linear = linear.checked_mul(extent)?.checked_add(offset as u64)?;
    }
    array
        .net_elements
        .iter()
        .find_map(|(element, signal)| (*element == linear).then_some(*signal))
}

/// Validate the force/release target shape shared by lowering and emission.
/// Packed variable selects are intentionally rejected; constant selected nets
/// are represented by fixed part descriptors and can therefore preserve all
/// current driver contributions on release.
fn validate_force_lhs(model: &IrModel, lhs: &IrLhs, path: &str) -> Result<bool, String> {
    match lhs {
        IrLhs::Whole(index) => {
            let signal = model.signals.get(*index).ok_or_else(|| {
                format!("force target signal {index} is out of bounds in `{path}`")
            })?;
            Ok(matches!(signal.ty, IrType::Real { .. }))
        }
        // An element of a member array of a record held as one signal per
        // leaf, selected by a run-time index: the emitter finds the leaf.
        IrLhs::PackedSelect { target, .. } if record_leaves(model, target) => Ok(false),
        // A member of an element held as one packed image.
        IrLhs::PackedSelect { target, .. }
            if matches!(
                target.as_ref(),
                IrLhs::ArrayElem {
                    elem_sel: IrElemSel::Whole,
                    ..
                }
            ) =>
        {
            validate_force_lhs(model, target, path).and_then(|real| {
                if real {
                    Err(format!(
                        "force/release target in `{path}` selects inside a real element"
                    ))
                } else {
                    Ok(false)
                }
            })
        }
        // A member reached through a native value access: an element of an
        // array of records with a real, string, handle, event or container
        // member, whose members have no fixed address.
        IrLhs::WholeRef { addr, .. } if addr.starts_with("&_llg_access_") => Err(format!(
            "force/release of a member of an element of an array of records in `{path}` is not supported: a record with a real, string, class-handle, chandle, event or container member is a native value whose members are not force targets yet; records of integral members are"
        )),
        IrLhs::PackedSelect { .. } | IrLhs::TaggedSelect { .. } | IrLhs::WholeRef { .. } => Err(
            format!("force/release target in `{path}` does not have persistent canonical storage"),
        ),
        IrLhs::Ref { .. } => Err(format!(
            "force/release target in `{path}` cannot be a ref formal"
        )),
        IrLhs::Bit(index, select, _) => {
            let signal = model.signals.get(*index).ok_or_else(|| {
                format!("force target signal {index} is out of bounds in `{path}`")
            })?;
            if signal.net_driver.is_none() && signal.net_alias.is_empty() {
                return Err(format!(
                    "force/release of a variable bit-select in `{path}` is not supported"
                ));
            }
            if !force_constant_index(select) {
                return Err(format!(
                    "force/release net bit-select in `{path}` requires a constant index"
                ));
            }
            Ok(false)
        }
        IrLhs::Part(index, left, right, _) => {
            let signal = model.signals.get(*index).ok_or_else(|| {
                format!("force target signal {index} is out of bounds in `{path}`")
            })?;
            // A part covering all of a variable is that variable: a constant
            // element of a record member array held as its own leaf
            // (`r.a[1]`) is a singular variable (SV 6.4, 10.6.2).
            let whole = *right == 0
                && u64::try_from(*left).is_ok_and(|left| left + 1 == u64::from(signal.ty.width()));
            if signal.net_driver.is_none() && signal.net_alias.is_empty() && !whole {
                return Err(format!(
                    "force/release of a variable part-select in `{path}` is not supported"
                ));
            }
            Ok(false)
        }
        IrLhs::IdxPart(..) => Err(format!(
            "force/release indexed part-select in `{path}` requires a constant net part-select"
        )),
        // An element of a static unpacked array variable is a singular
        // variable (SV 6.4, 10.6.2): its cell, or a member of a packed-image
        // element, is the force target, found when the statement executes.
        IrLhs::ArrayElem { arr, elem_sel, .. } => {
            let array = model.arrays.get(*arr).ok_or_else(|| {
                format!("force target array {arr} is out of bounds in `{path}`")
            })?;
            if array.is_net() || array.activation {
                return Err(format!(
                    "force/release of an element of {} in `{path}` is not supported: only elements of static unpacked array variables are force targets",
                    array_phrase(array)
                ));
            }
            match elem_sel {
                IrElemSel::Whole => Ok(array.real),
                // A member of an element held as one packed image (an
                // unpacked structure of integral members).
                IrElemSel::PackedChain(_) if !array.real => Ok(false),
                _ => Err(format!(
                    "force/release of a bit- or part-select of an element of {} in `{path}` is not supported: it is a select of a variable (IEEE 1800-2009 10.6.2)",
                    array_phrase(array)
                )),
            }
        }
        IrLhs::Stream {
            parts,
            slice,
            direction,
            ..
        } => {
            let mut real = false;
            for (part, _) in parts {
                if matches!(part, IrLhs::Stream { slice: nested_slice, direction: nested_direction, .. }
                    if *nested_slice != 1
                        || !matches!(nested_direction, IrStreamDirection::LeftToRight))
                {
                    return Err(format!(
                        "nested streaming force/release target in `{path}` is not supported"
                    ));
                }
                if validate_force_lhs(model, part, path)? {
                    real = true;
                }
            }
            if *slice == 0 {
                return Err(format!(
                    "force/release streaming target in `{path}` has zero slice size"
                ));
            }
            let _ = direction;
            if real && parts.len() != 1 {
                return Err(format!(
                    "force/release concatenation containing a real target in `{path}` is not supported"
                ));
            }
            Ok(real)
        }
    }
}

fn force_lhs_signed(model: &IrModel, lhs: &IrLhs) -> bool {
    match lhs {
        IrLhs::Whole(index) => model.signal(*index).ty.signed(),
        IrLhs::ArrayElem {
            arr,
            elem_sel: IrElemSel::Whole,
            ..
        } => model.array(*arr).signed,
        IrLhs::WholeRef { signed, .. } => *signed,
        IrLhs::Ref { signed, .. } => *signed,
        _ => false,
    }
}
