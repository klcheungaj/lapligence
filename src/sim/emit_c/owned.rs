//! Structured C11 emission for uniquely owned packed values.
//!
//! Each expression emits ordered setup statements and yields one tracked value.
//! Operand payloads are destroyed as soon as the consuming operation completes.
//! Descriptor slots are reused within a procedure; lexical locals use separate
//! registered scopes so queued writes can retain a stable target after exit.

use super::constants::{c_string_literal, emit_const, round_shortreal};
use super::context::RCtx;
use crate::sim::ir::*;
use std::collections::HashMap;

mod activations;
mod assertion_tasks;
pub(super) mod assertions;
mod calls;
mod captures;
mod clocking;
pub(super) mod containers;
mod control;
mod event_waits;
mod events;
mod expressions;
mod force;
mod formatting;
mod inertial;
mod input;
mod mailboxes;
pub(super) mod model;
pub(super) mod native;
mod native_access;
mod native_tasks;
mod objects;
mod pure_calls;
mod qualifiers;
mod references;
mod runtime_tasks;
mod statements;
mod stores;
mod streaming;
mod strings;
mod system;

#[must_use]
struct Value {
    code: String,
    width: u32,
    signed: bool,
    fill: Option<u8>,
    slot: Option<usize>,
}

impl Value {
    fn real(&self) -> String {
        if self.width == 0 {
            self.code.clone()
        } else {
            format!("sv4_to_real({})", self.code)
        }
    }
    fn truth(&self) -> String {
        if self.width == 0 {
            format!("llg_real_to_bool({})", self.code)
        } else {
            format!("sv4_to_bool({})", self.code)
        }
    }
    fn unknown_truth(&self) -> String {
        if self.width == 0 {
            "0".to_owned()
        } else {
            format!("(!sv4_to_bool({0}) && sv4_is_unknown({0}))", self.code)
        }
    }
}

#[derive(Clone)]
struct Binding {
    address: String,
    width: u32,
    signed: bool,
    two_state: bool,
    shortreal: bool,
    automatic: bool,
}

struct Activation {
    exit: String,
    handle: String,
    lexical_depth: usize,
}

pub(super) struct Frame<'a, 'm> {
    ctx: &'a RCtx<'m>,
    code: String,
    slots: Vec<bool>,
    next_name: usize,
    bindings: Vec<HashMap<String, Binding>>,
    event_bindings: Vec<HashMap<String, String>>,
    native_bindings: Vec<HashMap<String, native::NativeBinding>>,
    formal_overrides: Vec<Vec<Binding>>,
    marks: Vec<String>,
    labels: Vec<HashMap<String, bool>>,
    temp_roots: Vec<Vec<bool>>,
    return_address: Option<String>,
    allow_calls: bool,
    read_only_callback: bool,
    sampled_reads: bool,
    item_callback: bool,
    sequence_addresses: HashMap<String, Binding>,
    activations: Vec<Activation>,
    cancellation_return: bool,
    access_stack: Vec<String>,
    construction_stack: Vec<usize>,
}

fn pending(feature: &str) -> String {
    format!("P05 ownership emission is not yet implemented for {feature}; no C model was emitted")
}

impl<'a, 'm> Frame<'a, 'm> {
    pub(super) fn new(ctx: &'a RCtx<'m>) -> Self {
        Self {
            ctx,
            code: String::new(),
            slots: Vec::new(),
            next_name: 0,
            bindings: vec![HashMap::new()],
            event_bindings: vec![HashMap::new()],
            native_bindings: vec![HashMap::new()],
            formal_overrides: Vec::new(),
            marks: Vec::new(),
            labels: Vec::new(),
            temp_roots: Vec::new(),
            return_address: None,
            allow_calls: true,
            read_only_callback: false,
            sampled_reads: ctx.sampled,
            item_callback: false,
            sequence_addresses: HashMap::new(),
            activations: Vec::new(),
            cancellation_return: false,
            access_stack: Vec::new(),
            construction_stack: Vec::new(),
        }
    }
    fn line(&mut self, text: impl AsRef<str>) {
        self.code.push_str("    ");
        self.code.push_str(text.as_ref());
        self.code.push('\n');
    }
    fn name(&mut self, purpose: &str) -> String {
        let result = format!("_llg_{purpose}_{}", self.next_name);
        self.next_name += 1;
        result
    }
    fn scalar(&mut self, ty: &str, code: String) -> String {
        let name = self.name("scalar");
        self.line(format!("{ty} {name} = {code};"));
        name
    }
    fn reserve(&mut self, width: u32, signed: bool) -> Value {
        let slot = if let Some(index) = self.slots.iter().position(|used| !used) {
            self.slots[index] = true;
            index
        } else {
            self.slots.push(true);
            self.slots.len() - 1
        };
        Value {
            code: format!("_llg_t[{slot}]"),
            width,
            signed,
            fill: None,
            slot: Some(slot),
        }
    }
    fn value(&mut self, code: String, width: u32, signed: bool) -> Value {
        if width == 0 {
            return Value {
                code: self.scalar("double", code),
                width,
                signed,
                fill: None,
                slot: None,
            };
        }
        let value = self.reserve(width, signed);
        self.line(format!("sv4_replace(&{}, {code});", value.code));
        value
    }
    fn discard(&mut self, value: Value) {
        if let Some(slot) = value.slot {
            self.line(format!("sv4_destroy(&{});", value.code));
            self.slots[slot] = false;
        } else {
            self.line(format!("(void){};", value.code));
        }
    }
    fn replace(&mut self, mut value: Value, code: String, width: u32, signed: bool) -> Value {
        if value.slot.is_some() && width != 0 {
            self.line(format!("sv4_replace(&{}, {code});", value.code));
            value.width = width;
            value.signed = signed;
            value.fill = None;
            value
        } else {
            let result = self.value(code, width, signed);
            self.discard(value);
            result
        }
    }
    fn convert(
        &mut self,
        value: Value,
        width: u32,
        signed: bool,
        two_state: bool,
        shortreal: bool,
    ) -> Value {
        let code = if width == 0 {
            round_shortreal(value.real(), shortreal)
        } else if let Some(fill) = value.fill {
            format!("sv4_fill({fill}, {width}, {})", u8::from(signed))
        } else if value.width == 0 {
            format!(
                "sv4_from_real({}, {width}, {})",
                value.code,
                u8::from(signed)
            )
        } else {
            format!("sv4_cast({}, {width}, {})", value.code, u8::from(signed))
        };
        let result = self.replace(value, code, width, signed);
        if width != 0 && two_state {
            let code = format!("sv4_to_two_state({})", result.code);
            self.replace(result, code, width, signed)
        } else {
            result
        }
    }
    fn lookup(&self, name: &str) -> Option<Binding> {
        for bindings in self.bindings.iter().rev() {
            if let Some(binding) = bindings.get(name) {
                return Some(binding.clone());
            }
        }
        for func in &self.ctx.model.funcs {
            if let Some(local) = func.locals.iter().find(|local| local.c_name() == name) {
                return Some(Binding {
                    address: format!("&{name}"),
                    width: if local.real { 0 } else { local.width() },
                    signed: local.signed(),
                    two_state: local.two_state,
                    shortreal: local.shortreal,
                    automatic: false,
                });
            }
        }
        for (index, signal) in self.ctx.model.signals.iter().enumerate() {
            if !signal.net_alias.is_empty() && name == format!("llg_net_alias_{index}.visible") {
                return Some(Binding {
                    address: format!("&llg_net_alias_{index}.visible"),
                    width: signal.ty.width(),
                    signed: signal.ty.signed(),
                    two_state: signal.ty.two_state(),
                    shortreal: false,
                    automatic: false,
                });
            }
            if signal.c_name == name && (!signal.omit || signal.net_driver.is_some()) {
                return Some(Binding {
                    address: format!("&{name}"),
                    width: signal.ty.width(),
                    signed: signal.ty.signed(),
                    two_state: matches!(
                        signal.ty,
                        IrType::Packed {
                            two_state: true,
                            ..
                        }
                    ),
                    shortreal: matches!(signal.ty, IrType::Real { shortreal: true }),
                    automatic: false,
                });
            }
        }
        None
    }
    fn address(&mut self, address: &str) -> Result<Binding, String> {
        if let Some(binding) = self.sequence_addresses.get(address) {
            return Ok(binding.clone());
        }
        if let Some(name) = address.strip_prefix('&') {
            return self.resolve_lookup(name);
        }
        if let Some(func) = self.ctx.func {
            for (index, formal) in func.formals.iter().enumerate() {
                if address == format!("o{index}") && formal.is_out && !formal.is_ref() {
                    return Ok(Binding {
                        address: address.to_owned(),
                        width: if formal.real { 0 } else { formal.width },
                        signed: formal.signed,
                        two_state: formal.two_state,
                        shortreal: formal.shortreal,
                        automatic: true,
                    });
                }
            }
        }
        Err(pending(&format!("opaque address {address}")))
    }
    fn read_binding(&mut self, binding: &Binding) -> Value {
        let code = if binding.width == 0 {
            format!("*({})", binding.address)
        } else {
            format!("sv4_clone({})", binding.address)
        };
        self.value(code, binding.width, binding.signed)
    }
    fn local(
        &mut self,
        name: &str,
        width: u32,
        signed: bool,
        two_state: bool,
        init: Option<&IrExpr>,
    ) -> Result<(), String> {
        let pointer = self.name("local");
        let address = if width == 0 {
            // Another coroutine may publish through this address while ours is
            // suspended. libaco's shared stack is not stable variable storage.
            let owner = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(double), NULL)".to_owned(),
            );
            self.line(format!(
                "double* {pointer} = (double*)llg_value_scope_object({owner});"
            ));
            self.line(format!("*{pointer} = 0.0;"));
            pointer
        } else {
            self.line(format!(
                "sv4_t* {pointer} = llg_value_scope_values(llg_value_scope_begin(1));"
            ));
            pointer
        };
        let binding = Binding {
            address: address.clone(),
            width,
            signed,
            two_state,
            shortreal: false,
            automatic: true,
        };
        // Make the default visible to self-referential initializers.
        if width != 0 {
            self.line(format!(
                "sv4_replace({address}, {});",
                super::expressions::packed_default(width, signed, two_state)
            ));
        }
        self.bindings
            .last_mut()
            .expect("frame always has a binding scope")
            .insert(name.to_owned(), binding.clone());
        if let Some(init) = init {
            let value = self.expression(init)?;
            let value = self.convert(value, width, signed, two_state, false);
            if width == 0 {
                self.line(format!("*({address}) = {};", value.code));
            } else {
                self.line(format!("sv4_move({address}, &{});", value.code));
            }
            self.discard(value);
        }
        Ok(())
    }
    pub(super) fn prologue(&self) -> String {
        format!("    llg_value_scope_t* _llg_frame_base = llg_value_scope_mark();\n    llg_value_scope_t* _llg_temp_scope = llg_value_scope_begin({});\n    sv4_t* _llg_t = llg_value_scope_values(_llg_temp_scope);\n    (void)_llg_t;\n", self.slots.len())
    }
    pub(super) fn body(&self) -> &str {
        &self.code
    }
}

#[cfg(test)]
mod tests;
