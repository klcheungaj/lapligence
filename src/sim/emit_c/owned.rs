//! Structured C11 emission for uniquely owned packed values.
//!
//! Each expression emits ordered setup statements and yields one tracked value.
//! Operand payloads are destroyed as soon as the consuming operation completes.
//! Descriptor slots are reused within a procedure; lexical locals use separate
//! registered scopes so queued writes can retain a stable target after exit.

use super::constants::{c_string_literal, emit_const, round_shortreal};
use super::context::RCtx;
use super::frame_layout::{declaration, FrameLayout, FrameStorage};
use crate::sim::execution::{CallMechanism, CoroutineId, ExecutionAnalysis, SuspensionOperation};
use crate::sim::ir::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

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
mod fixed_array_reductions;
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

#[derive(Clone, Copy)]
struct PendingCall {
    resume: u32,
    callee: Option<usize>,
    mechanism: CallMechanism,
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
    callback_signal_overrides: Vec<HashMap<usize, usize>>,
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
    layout: FrameLayout,
    declaration_error: Option<String>,
    pending_calls: VecDeque<PendingCall>,
    frame_upper_bounds: BTreeMap<usize, usize>,
    coroutine_functions: BTreeSet<usize>,
}

fn pending(feature: &str) -> String {
    format!("P05 ownership emission is not yet implemented for {feature}; no C model was emitted")
}

impl<'a, 'm> Frame<'a, 'm> {
    pub(super) fn new(ctx: &'a RCtx<'m>) -> Self {
        Self::with_storage(ctx, FrameStorage::CStack)
    }

    pub(super) fn new_coframe(
        ctx: &'a RCtx<'m>,
        analysis: &ExecutionAnalysis,
        owner: CoroutineId,
        frame_upper_bounds: &BTreeMap<usize, usize>,
    ) -> Result<Self, String> {
        let mut frame = Self::with_storage(ctx, FrameStorage::CoFrame);
        frame.frame_upper_bounds = frame_upper_bounds.clone();
        frame.coroutine_functions = (0..ctx.model.funcs.len())
            .filter(|function| analysis.is_coroutine_function(*function))
            .collect();
        let mut calls = analysis
            .sites(owner)
            .into_iter()
            .flat_map(|sites| sites.values())
            .filter_map(|site| match site.operation() {
                SuspensionOperation::Call { callee } => Some(PendingCall {
                    resume: site.resume(),
                    callee: *callee,
                    mechanism: site.mechanism()?,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        calls.sort_by_key(|call| call.resume);
        frame.pending_calls = calls.into();
        Ok(frame)
    }

    fn with_storage(ctx: &'a RCtx<'m>, storage: FrameStorage) -> Self {
        let mut layout = FrameLayout::new(storage);
        if storage == FrameStorage::CoFrame {
            for (ty, name) in [
                ("llg_co_arena_t*", "arena"),
                ("llg_value_scope_t*", "_llg_frame_base"),
                ("llg_value_scope_t*", "_llg_temp_scope"),
                ("sv4_t*", "_llg_t"),
            ] {
                layout
                    .declare(ty, name)
                    .expect("fixed coroutine fields are unique and have known layouts");
            }
        }
        Self {
            ctx,
            code: String::new(),
            slots: Vec::new(),
            next_name: 0,
            bindings: vec![HashMap::new()],
            event_bindings: vec![HashMap::new()],
            native_bindings: vec![HashMap::new()],
            formal_overrides: Vec::new(),
            callback_signal_overrides: Vec::new(),
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
            layout,
            declaration_error: None,
            pending_calls: VecDeque::new(),
            frame_upper_bounds: BTreeMap::new(),
            coroutine_functions: BTreeSet::new(),
        }
    }
    fn line(&mut self, text: impl AsRef<str>) {
        let text = if self.layout.storage() == FrameStorage::CoFrame {
            self.rewrite_frame_accesses(text.as_ref())
        } else {
            text.as_ref().to_owned()
        };
        self.code.push_str("    ");
        self.code.push_str(&text);
        self.code.push('\n');
    }
    fn rewrite_frame_accesses(&self, text: &str) -> String {
        let names = self.layout.field_names().collect::<BTreeSet<_>>();
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len() + 16);
        let mut index = 0;
        let mut quoted = None;
        let mut escaped = false;
        while index < bytes.len() {
            let start = index;
            if let Some(quote) = quoted {
                let ch = text[index..].chars().next().unwrap_or('\0');
                out.push(ch);
                index += ch.len_utf8();
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    quoted = None;
                }
            } else if matches!(bytes[index], b'\'' | b'"') {
                let quote = bytes[index] as char;
                out.push(quote);
                index += 1;
                quoted = Some(quote);
            } else if bytes[index] == b'_' || bytes[index].is_ascii_alphabetic() {
                index += 1;
                while index < bytes.len()
                    && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
                {
                    index += 1;
                }
                let ident = &text[start..index];
                let member_access = start > 0 && matches!(bytes[start - 1], b'.' | b'>');
                if names.contains(ident) && !member_access {
                    out.push_str("F->");
                }
                out.push_str(ident);
            } else {
                let ch = text[index..]
                    .chars()
                    .next()
                    .expect("index remains within the generated text");
                out.push(ch);
                index += ch.len_utf8();
            }
        }
        out
    }
    fn name(&mut self, purpose: &str) -> String {
        let result = format!("_llg_{purpose}_{}", self.next_name);
        self.next_name += 1;
        result
    }
    fn access(&self, name: &str) -> String {
        if self.layout.storage() == FrameStorage::CoFrame {
            format!("F->{name}")
        } else {
            name.to_owned()
        }
    }
    fn frame_field(&mut self, ty: &str, name: &str) -> Result<String, String> {
        self.layout.declare(ty, name)
    }
    fn take_call_slot(
        &mut self,
        callee: usize,
        callee_type: &str,
    ) -> Result<super::frame_layout::CallSlot, String> {
        let plan = self
            .pending_calls
            .pop_front()
            .ok_or_else(|| format!("missing coroutine call-site analysis for callee {callee}"))?;
        if plan.callee.is_some_and(|expected| expected != callee) {
            return Err(format!(
                "coroutine call-site analysis expected callee {:?}, emitted {callee}",
                plan.callee
            ));
        }
        let upper_bound = if plan.mechanism == CallMechanism::Arena {
            0
        } else {
            self.frame_upper_bounds
                .get(&callee)
                .copied()
                .ok_or_else(|| format!("missing frame upper bound for coroutine callee {callee}"))?
        };
        self.layout
            .add_call(plan.resume, callee_type, plan.mechanism, upper_bound)
    }
    fn declare_named(&mut self, ty: &str, name: &str, init: String) -> String {
        let access = match self.layout.declare(ty, name) {
            Ok(access) => access,
            Err(error) => {
                self.declaration_error.get_or_insert(error);
                name.to_owned()
            }
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            let init = if init.trim_start().starts_with('{') {
                format!("({ty}){init}")
            } else {
                init
            };
            self.line(format!("{access} = {init};"));
        } else {
            self.line(format!("{} = {init};", declaration(ty, name)));
        }
        access
    }
    fn declare(&mut self, ty: &str, purpose: &str, init: String) -> String {
        let name = self.name(purpose);
        self.declare_named(ty, &name, init)
    }
    fn loop_variable(&mut self, ty: &str, purpose: &str) -> (String, String) {
        let name = self.name(purpose);
        if self.layout.storage() == FrameStorage::CoFrame {
            let access = match self.layout.declare(ty, &name) {
                Ok(access) => access,
                Err(error) => {
                    self.declaration_error.get_or_insert(error);
                    name
                }
            };
            (access.clone(), access)
        } else {
            (name.clone(), declaration(ty, &name))
        }
    }
    fn declaration_target_named(&mut self, ty: &str, name: &str) -> (String, String) {
        if self.layout.storage() == FrameStorage::CoFrame {
            let access = match self.layout.declare(ty, name) {
                Ok(access) => access,
                Err(error) => {
                    self.declaration_error.get_or_insert(error);
                    name.to_owned()
                }
            };
            (access.clone(), access)
        } else {
            (name.to_owned(), declaration(ty, name))
        }
    }
    fn declare_array(&mut self, ty: &str, purpose: &str, count: usize) -> String {
        let name = self.name(purpose);
        let array_ty = format!("{ty}[{count}]");
        let access = match self.layout.declare(&array_ty, &name) {
            Ok(access) => access,
            Err(error) => {
                self.declaration_error.get_or_insert(error);
                name.clone()
            }
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            self.line(format!("memset({access}, 0, sizeof({access}));"));
        } else {
            self.line(format!("{ty} {name}[{count}] = {{0}};"));
        }
        access
    }
    fn declare_array_init(
        &mut self,
        ty: &str,
        purpose: &str,
        count: usize,
        entries: &str,
    ) -> String {
        let name = self.name(purpose);
        let array_ty = format!("{ty}[{count}]");
        let access = match self.layout.declare(&array_ty, &name) {
            Ok(access) => access,
            Err(error) => {
                self.declaration_error.get_or_insert(error);
                name.clone()
            }
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            self.line(format!(
                "memcpy({access}, ({ty}[]){{ {entries} }}, sizeof({access}));"
            ));
        } else {
            self.line(format!("{ty} {name}[{count}] = {{ {entries} }};"));
        }
        access
    }
    fn scalar(&mut self, ty: &str, code: String) -> String {
        self.declare(ty, "scalar", code)
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
            let pointer = self.declare_named(
                "double*",
                &pointer,
                format!("(double*)llg_value_scope_object({owner})"),
            );
            self.line(format!("*{pointer} = 0.0;"));
            pointer
        } else {
            self.declare_named(
                "sv4_t*",
                &pointer,
                "llg_value_scope_values(llg_value_scope_begin(1))".to_owned(),
            )
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
        if self.layout.storage() == FrameStorage::CoFrame {
            format!("    F->_llg_frame_base = llg_value_scope_mark();\n    F->_llg_temp_scope = llg_value_scope_begin({});\n    F->_llg_t = llg_value_scope_values(F->_llg_temp_scope);\n    (void)F->_llg_t;\n", self.slots.len())
        } else {
            format!("    llg_value_scope_t* _llg_frame_base = llg_value_scope_mark();\n    llg_value_scope_t* _llg_temp_scope = llg_value_scope_begin({});\n    sv4_t* _llg_t = llg_value_scope_values(_llg_temp_scope);\n    (void)_llg_t;\n", self.slots.len())
        }
    }
    pub(super) fn macro_epilogue(&self) -> &'static str {
        ""
    }
    pub(super) fn body(&self) -> &str {
        &self.code
    }
    pub(super) fn into_layout(mut self) -> Result<FrameLayout, String> {
        // Emission can fold an expression which still has a conservative call
        // site in the pre-emission execution analysis. Preserve storage and
        // descriptor offsets for those unreachable sites even though no call
        // expression consumed their slots.
        while let Some(call) = self.pending_calls.pop_front() {
            let (callee_type, upper_bound) = match call.callee {
                Some(callee) => {
                    let callee_type = format!("{}_frame_t", self.ctx.model.func(callee).c_name);
                    let upper_bound = if call.mechanism == CallMechanism::Arena {
                        0
                    } else {
                        self.frame_upper_bounds
                            .get(&callee)
                            .copied()
                            .ok_or_else(|| {
                                format!("missing frame upper bound for coroutine callee {callee}")
                            })?
                    };
                    (callee_type, upper_bound)
                }
                None if call.mechanism == CallMechanism::Arena => ("void".to_owned(), 0),
                None => {
                    return Err(format!(
                        "embedded coroutine call-site analysis entry {} has no callee",
                        call.resume
                    ));
                }
            };
            self.layout
                .add_call(call.resume, &callee_type, call.mechanism, upper_bound)?;
        }
        if let Some(error) = self.declaration_error {
            Err(error)
        } else {
            Ok(self.layout)
        }
    }
}

#[cfg(test)]
mod tests;
