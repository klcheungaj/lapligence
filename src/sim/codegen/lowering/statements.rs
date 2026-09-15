//! Procedural statement lowering through the shared emission context.

use super::objects::object_query;
use super::*;
use crate::core::db::ConcurrentAssertionKind;
use crate::sim::ir::{
    IrAssertionControlKind, IrContainerElement, IrObjectQuery, IrObjectStmt, IrStringExpr,
    IrVpiCompileArg, IrVpiCompileCall,
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
mod system_tasks;


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

fn force_constant_index(expr: &IrExpr) -> bool {
    let IrExprKind::Const(value) = expr.kind() else {
        return false;
    };
    value.real_value().is_none()
        && value.fill().is_none()
        && value.x_mask().iter().all(|mask| *mask == 0)
        && value.z_mask().iter().all(|mask| *mask == 0)
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
        IrLhs::WholeRef { .. } => Err(format!(
            "force/release target in `{path}` does not have persistent canonical storage"
        )),
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
        IrLhs::Part(index, ..) => {
            let signal = model.signals.get(*index).ok_or_else(|| {
                format!("force target signal {index} is out of bounds in `{path}`")
            })?;
            if signal.net_driver.is_none() && signal.net_alias.is_empty() {
                return Err(format!(
                    "force/release of a variable part-select in `{path}` is not supported"
                ));
            }
            Ok(false)
        }
        IrLhs::IdxPart(..) => Err(format!(
            "force/release indexed part-select in `{path}` requires a constant net part-select"
        )),
        IrLhs::ArrayElem { .. } => Err(format!(
            "force/release of an unpacked-array element in `{path}` is not supported"
        )),
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
        IrLhs::WholeRef { signed, .. } => *signed,
        IrLhs::Ref { signed, .. } => *signed,
        _ => false,
    }
}
