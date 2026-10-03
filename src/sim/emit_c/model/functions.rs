//! Functions.

use super::*;

/// The C parameter list of a lowered non-coroutine function: outputs first
/// (`o{formal idx}`), then inputs (`a{formal idx}`), the recursion depth and,
/// for a packed result, the caller's destination `_llg_result`.
pub(super) fn func_params(f: &IrFunc) -> String {
    func_param_fields(f)
        .into_iter()
        .map(|(ty, name)| format!("{ty} {name}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether a function returns a packed value through `_llg_result`.
///
/// A returned `sv4_t` travels through a caller stack temporary at every call
/// site on all supported ABIs, so non-coroutine functions write their packed
/// result into an initialized caller-owned destination instead, and borrow
/// packed inputs by address (see `emit_c::destinations`).
pub(super) fn packed_result(f: &IrFunc) -> bool {
    !f.ret_string && !f.ret_chandle && matches!(f.ret, Some(IrType::Packed { .. }))
}

/// Typed C parameters of a non-coroutine function; see [`func_params`].
pub(super) fn func_param_fields(f: &IrFunc) -> Vec<(String, String)> {
    let mut params = frame_param_fields(f)
        .into_iter()
        .map(|(ty, name)| {
            if ty == "sv4_t" {
                ("const sv4_t*".to_owned(), name)
            } else {
                (ty, name)
            }
        })
        .collect::<Vec<_>>();
    if packed_result(f) {
        params.push(("sv4_t*".to_owned(), "_llg_result".to_owned()));
    }
    params
}

/// Argument names forwarding every parameter of [`func_params`] unchanged.
pub(super) fn func_param_names(f: &IrFunc) -> String {
    func_param_fields(f)
        .into_iter()
        .map(|(_, name)| name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Coroutine argument fields stored in the callee frame by the caller. Packed
/// inputs are descriptor borrows copied into the callee's own owners at entry.
pub(super) fn frame_param_fields(f: &IrFunc) -> Vec<(String, String)> {
    let mut params = Vec::new();
    if f.receiver_class.is_some() {
        params.push(("void *".to_owned(), "_this".to_owned()));
    }
    for (idx, form) in f.formals.iter().enumerate() {
        if form.fixed_array.is_some() && form.is_address() {
            params.push((
                "llg_fixed_array_t*".to_owned(),
                format!("{}{idx}", if form.is_ref() { "r" } else { "o" }),
            ));
        } else if form.is_ref() {
            if form.string {
                let qualifier = if form.is_const_ref() { "const " } else { "" };
                params.push((format!("{qualifier}llg_string_t*"), format!("r{idx}")));
            } else if form.chandle {
                let ty = if form.is_const_ref() {
                    "void * const*"
                } else {
                    "void **"
                };
                params.push((ty.to_owned(), format!("r{idx}")));
            } else {
                let qualifier = if form.is_const_ref() { "const " } else { "" };
                params.push((format!("{qualifier}llg_ref_t*"), format!("r{idx}")));
            }
        } else if form.is_out {
            params.push((
                format!(
                    "{}*",
                    if form.string {
                        "llg_string_t"
                    } else if form.chandle {
                        "void *"
                    } else if form.real {
                        "double"
                    } else {
                        "sv4_t"
                    }
                ),
                format!("o{idx}"),
            ));
        }
    }
    for (idx, form) in f.formals.iter().enumerate() {
        if !form.is_address() {
            params.push((
                (if form.fixed_array.is_some() {
                    "llg_fixed_array_t*"
                } else if form.string {
                    "llg_string_t"
                } else if form.event {
                    "llg_event_t"
                } else if form.chandle {
                    "void *"
                } else if form.real {
                    "double"
                } else {
                    "sv4_t"
                })
                .to_owned(),
                format!("a{idx}"),
            ));
        }
    }
    params.push(("int".to_owned(), "depth".to_owned()));
    params
}

pub(super) fn func_prototype(f: &IrFunc) -> Result<String, String> {
    if f.dpi_import().is_some() {
        let mut out = dpi_external_prototype(f)?;
        out.push_str(&format!(
            "static {} {}({});\n",
            internal_return_type(f),
            f.c_name,
            func_params(f)
        ));
        return Ok(out);
    }
    let ret_t = if f.ret_string {
        "llg_string_t"
    } else if f.ret_chandle {
        "void *"
    } else if matches!(f.ret, Some(IrType::Real { .. })) {
        "double"
    } else {
        "void"
    };
    Ok(format!(
        "static {ret_t} {}({});\n",
        f.c_name,
        func_params(f)
    ))
}

#[allow(dead_code)] // legacy model renderer superseded by owned::model
pub(super) fn render_func_body(ctx: &RCtx<'_>, f: &IrFunc) -> Result<String, String> {
    if f.dpi_import().is_some() {
        return render_dpi_thunk(f);
    }
    let ret_t = if f.ret_string {
        "llg_string_t"
    } else if f.ret_chandle {
        "void *"
    } else if matches!(f.ret, Some(IrType::Real { .. })) {
        "double"
    } else if f.ret.is_some() {
        "sv4_t"
    } else {
        "void"
    };
    let mut out = format!("static {ret_t} {}({}) {{\n", f.c_name, func_params(f));
    // The all-X return value used by the recursion guard.
    let string_cleanup = f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.string && !form.is_address())
        .map(|(idx, _)| format!("llg_string_destroy(&a{idx}); "))
        .collect::<String>();
    let ret_clause = if f.ret_string {
        format!("{string_cleanup}return llg_string_bytes(\"\", 0);")
    } else if f.ret_chandle {
        "return NULL;".to_string()
    } else if f.ret.is_some() {
        format!("return {};", f.ret_x())
    } else {
        "return;".to_string()
    };
    out.push_str(&format!(
        "    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \
         \"llg: recursion limit exceeded in %s\\n\", \"{}\");\n        \
         {ret_clause}\n    }}\n",
        f.c_name
    ));
    let persistent = !f.automatic;
    if persistent && (f.ret.is_some() || f.ret_chandle) {
        out.push_str("    static int _static_init;\n");
    }
    if let Some(IrType::Real { .. }) = f.ret {
        if persistent {
            out.push_str("    static double _ret;\n");
        } else {
            out.push_str("    double _ret = 0.0;\n");
        }
    }
    if let Some(IrType::Packed {
        width,
        signed,
        two_state,
    }) = f.ret
    {
        // Function-name return variable → `_ret` local.
        if persistent {
            out.push_str("    static sv4_t _ret;\n");
        } else {
            out.push_str(&format!(
                "    sv4_t _ret = {};\n",
                packed_default(width, signed, two_state)
            ));
        }
    }
    if f.ret_chandle {
        out.push_str(if persistent {
            "    static void *_ret;\n"
        } else {
            "    void *_ret = NULL;\n"
        });
    }
    if f.ret_string {
        if persistent {
            out.push_str("    static llg_string_t _ret = {0};\n");
        } else {
            out.push_str("    llg_string_t _ret = {0};\n");
        }
    }
    if persistent && (f.ret.is_some() || f.ret_chandle) {
        out.push_str("    if (!_static_init) {\n");
        if let Some(IrType::Real { .. }) = f.ret {
            out.push_str("        _ret = 0.0;\n");
        }
        if let Some(IrType::Packed {
            width,
            signed,
            two_state,
        }) = f.ret
        {
            out.push_str(&format!(
                "        _ret = {};\n",
                packed_default(width, signed, two_state)
            ));
        }
        out.push_str("        _static_init = 1;\n    }\n");
    }
    out.push_str(&block_stmts_of(ctx, &f.body)?);
    out.push_str("    ");
    if f.ret_string {
        out.push_str(&format!(
            "{}return {};\n",
            string_cleanup,
            if persistent {
                "llg_string_clone(&_ret)"
            } else {
                "_ret"
            }
        ));
    } else if f.ret.is_some() || f.ret_chandle {
        out.push_str("return _ret;\n");
    }
    out.push_str("}\n\n");
    Ok(out)
}

#[allow(dead_code)] // legacy model renderer superseded by owned::model
pub(super) fn block_stmts_of(
    ctx: &RCtx<'_>,
    stmts: &[crate::sim::ir::IrStmt],
) -> Result<String, String> {
    let mut out = String::new();
    for s in stmts {
        out.push_str(&render_stmt(ctx, s)?);
        if let Some(label) = ctx.activation_label.as_deref() {
            out.push_str(&format!(
                "    if (llg_activation_cancelled()) goto {label};\n"
            ));
        }
    }
    Ok(out)
}
