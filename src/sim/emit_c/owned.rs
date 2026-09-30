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
mod cached_fields;
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
pub(super) mod pca_batches;
mod pure_calls;
mod qualifiers;
mod references;
mod repeated_values;
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
    /// A cancellation check branches to `exit`; the enclosing construct must
    /// check again because an outer activation may be the cancelled one.
    checked: bool,
}

#[derive(Clone)]
struct PendingSite {
    resume: u32,
    operation: SuspensionOperation,
    mechanism: Option<CallMechanism>,
}

/// A finished coroutine function body with its frame layout.
pub(super) struct CoroutineBody {
    pub(super) body: String,
    pub(super) layout: FrameLayout,
    /// Uninitialized C locals mirroring frame fields (see `cached_fields`),
    /// declared before the resume dispatch.
    pub(super) cached_locals: String,
}

#[derive(Clone, Debug)]
struct DeferredDeclaration {
    ty: String,
    name: String,
    standalone: bool,
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
    /// Count of emitted cancellation points: resume points, `disable` and
    /// calls that may disable. A cancellation check is emitted only when this
    /// changed since the start of the construct it follows (design §13.2
    /// rule 1): between such points no activation of a running process can
    /// become cancelled.
    cancellation_points: usize,
    may_disable: HashMap<usize, bool>,
    access_stack: Vec<String>,
    construction_stack: Vec<usize>,
    layout: FrameLayout,
    declarations: Vec<DeferredDeclaration>,
    declaration_error: Option<String>,
    brace_kinds: Vec<bool>,
    in_block_comment: bool,
    structural_error: Option<String>,
    resume_probe: bool,
    pending_sites: VecDeque<PendingSite>,
    resume_numbers: Vec<u32>,
    frame_upper_bounds: BTreeMap<usize, usize>,
    coroutine_functions: BTreeSet<usize>,
    cached_fields: cached_fields::CachedFields,
    pca_owner: Option<String>,
    pca_batches: Vec<super::statements::pca_batches::Batch>,
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
        let mut sites = analysis
            .sites(owner)
            .into_iter()
            .flat_map(|sites| sites.values())
            .map(|site| PendingSite {
                resume: site.resume(),
                operation: site.operation().clone(),
                mechanism: site.mechanism(),
            })
            .collect::<Vec<_>>();
        sites.sort_by_key(|site| site.resume);
        frame.resume_numbers = sites.iter().map(|site| site.resume).collect();
        frame.pending_sites = sites.into();
        if !frame.resume_numbers.is_empty() {
            for (ty, name) in [
                ("llg_value_scope_t*", "_llg_frame_base"),
                ("llg_value_scope_t*", "_llg_temp_scope"),
                ("sv4_t*", "_llg_t"),
            ] {
                frame.layout.declare(ty, name)?;
            }
            // `_llg_temp_scope` is only ever written, so it has nothing to cache.
            for (ty, name) in [
                ("llg_value_scope_t*", "_llg_frame_base"),
                ("sv4_t*", "_llg_t"),
            ] {
                frame.cached_fields.register(ty, name, true);
            }
        }
        Ok(frame)
    }

    fn with_storage(ctx: &'a RCtx<'m>, storage: FrameStorage) -> Self {
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
            cancellation_points: 0,
            may_disable: HashMap::new(),
            access_stack: Vec::new(),
            construction_stack: Vec::new(),
            layout: FrameLayout::new(storage),
            declarations: Vec::new(),
            declaration_error: None,
            brace_kinds: Vec::new(),
            in_block_comment: false,
            structural_error: None,
            resume_probe: false,
            pending_sites: VecDeque::new(),
            resume_numbers: Vec::new(),
            frame_upper_bounds: BTreeMap::new(),
            coroutine_functions: BTreeSet::new(),
            cached_fields: cached_fields::CachedFields::default(),
            pca_owner: None,
            pca_batches: Vec::new(),
        }
    }
    fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if self.resume_probe {
            self.mark_continuation_fields(text);
            if has_statement_boundary(text) {
                self.resume_probe = false;
            }
        }
        if self.layout.storage() == FrameStorage::CoFrame {
            self.track_frame_blocks(text);
        }
        let rewritten = (self.layout.storage() == FrameStorage::CStack)
            .then(|| self.rewrite_frame_accesses(text));
        let text = rewritten.as_deref().unwrap_or(text);
        self.code.push_str("    ");
        self.code.push_str(text);
        self.code.push('\n');
        if is_runtime_suspension(text) {
            self.layout.mark_resume();
            self.resume_probe = true;
            self.cancellation_points += 1;
            if self.layout.storage() == FrameStorage::CoFrame {
                // A resume lands inside the suspension macro with every cached
                // local indeterminate; the frame holds the only surviving copy.
                if let Some(reload) = self.cached_fields.reload_placeholder() {
                    self.code.push_str(&reload);
                }
            }
        }
    }

    fn mark_continuation_fields(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let mut index = 0;
        let mut quoted = None;
        let mut escaped = false;
        while index < bytes.len() {
            if let Some(quote) = quoted {
                let byte = bytes[index];
                index += 1;
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    quoted = None;
                }
            } else if matches!(bytes[index], b'\'' | b'"') {
                quoted = Some(bytes[index]);
                index += 1;
            } else if bytes[index] == b'_' || bytes[index].is_ascii_alphabetic() {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
                {
                    index += 1;
                }
                self.layout.mark_hot(&text[start..index]);
            } else {
                index += 1;
            }
        }
    }

    /// Mirror the structural braces in emitted C. Aggregate initializer and
    /// compound-literal braces are balanced too, but do not create frame
    /// blocks. This single text path makes a missing block event an error
    /// rather than silently producing a layout that differs from the C body.
    fn track_frame_blocks(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let mut index = 0;
        let mut segment_start = 0;
        let mut paren_depth = 0usize;
        let mut quoted = None;
        let mut escaped = false;
        while index < bytes.len() {
            if self.in_block_comment {
                if bytes[index..].starts_with(b"*/") {
                    self.in_block_comment = false;
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }
            if let Some(quote) = quoted {
                let byte = bytes[index];
                index += 1;
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    quoted = None;
                }
                continue;
            }
            if bytes[index..].starts_with(b"//") {
                break;
            }
            if bytes[index..].starts_with(b"/*") {
                self.in_block_comment = true;
                index += 2;
                continue;
            }
            match bytes[index] {
                b'\'' | b'"' => quoted = Some(bytes[index]),
                b'(' | b'[' => paren_depth += 1,
                b')' | b']' => paren_depth = paren_depth.saturating_sub(1),
                b';' if paren_depth == 0 => segment_start = index + 1,
                b'{' => {
                    let inside_initializer = self.brace_kinds.last() == Some(&false);
                    let segment = text[segment_start..index].trim();
                    let structural = !inside_initializer && is_structural_block_open(segment);
                    self.brace_kinds.push(structural);
                    if structural {
                        self.layout.begin_block();
                        self.cached_fields.open_block();
                    }
                    segment_start = index + 1;
                }
                b'}' => {
                    match self.brace_kinds.pop() {
                        Some(true) => {
                            self.cached_fields.close_block();
                            if let Err(error) = self.layout.end_block() {
                                self.structural_error.get_or_insert(error);
                            }
                        }
                        Some(false) => {}
                        None => {
                            self.structural_error.get_or_insert_with(|| {
                                "generated coroutine body closed an unmatched C brace".to_owned()
                            });
                        }
                    }
                    segment_start = index + 1;
                }
                _ => {}
            }
            index += 1;
        }
    }

    fn rewrite_frame_accesses(&self, text: &str) -> String {
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
                if ident == "self" && !member_access {
                    out.push_str(if self.layout.storage() == FrameStorage::CoFrame {
                        "LLG_CO_OWNER(ch, llg_proc_t)"
                    } else {
                        "llg_current()"
                    });
                } else if let Some(access) = self
                    .layout
                    .field_access(ident)
                    .filter(|_| !member_access && !self.cached_fields.is_cached(ident))
                {
                    out.push_str("F->");
                    out.push_str(access);
                } else {
                    out.push_str(ident);
                }
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
        name.to_owned()
    }
    fn frame_field(&mut self, ty: &str, name: &str) -> Result<String, String> {
        self.layout.declare_required(ty, name)
    }
    fn take_call_slot(
        &mut self,
        callee: usize,
        callee_type: &str,
    ) -> Result<super::frame_layout::CallSlot, String> {
        let plan = self
            .pending_sites
            .pop_front()
            .ok_or_else(|| format!("missing coroutine call-site analysis for callee {callee}"))?;
        let SuspensionOperation::Call {
            callee: expected_callee,
        } = plan.operation
        else {
            return Err(format!(
                "coroutine suspension site {} is {:?}, emitted call to callee {callee}",
                plan.resume, plan.operation
            ));
        };
        if expected_callee.is_some_and(|expected| expected != callee) {
            return Err(format!(
                "coroutine call-site analysis expected callee {:?}, emitted {callee}",
                expected_callee
            ));
        }
        let mechanism = plan
            .mechanism
            .ok_or_else(|| format!("coroutine call site {} has no mechanism", plan.resume))?;
        let upper_bound = if mechanism == CallMechanism::Arena {
            0
        } else {
            self.frame_upper_bounds
                .get(&callee)
                .copied()
                .ok_or_else(|| format!("missing frame upper bound for coroutine callee {callee}"))?
        };
        self.layout
            .add_call(plan.resume, callee_type, mechanism, upper_bound)
    }
    fn take_resume(&mut self, operation: SuspensionOperation) -> Result<u32, String> {
        let site = self
            .pending_sites
            .pop_front()
            .ok_or_else(|| format!("emitted unnumbered coroutine suspension site {operation:?}"))?;
        if site.operation != operation {
            return Err(format!(
                "coroutine suspension site {} is {:?}, emitted {:?}",
                site.resume, site.operation, operation
            ));
        }
        Ok(site.resume)
    }
    fn await_arm(
        &mut self,
        operation: SuspensionOperation,
        arm: impl AsRef<str>,
    ) -> Result<(), String> {
        let resume = self.take_resume(operation)?;
        self.line(format!("LLG_CO_AWAIT(co, ch, {resume}, {});", arm.as_ref()));
        Ok(())
    }
    fn dispatch(&self) -> String {
        let mut dispatch = String::from("    LLG_CO_DISPATCH_BEGIN(co)\n");
        for resume in &self.resume_numbers {
            dispatch.push_str(&format!("    LLG_CO_RESUME_CASE({resume})\n"));
        }
        dispatch.push_str("    LLG_CO_DISPATCH_END(co)\n");
        dispatch
    }
    fn declare_named(&mut self, ty: &str, name: &str, init: String) -> String {
        self.declare_named_with(ty, name, init, false)
    }
    /// Like `declare_named`, for a pointer that is assigned only here and read
    /// afterwards: a frame-resident field is then mirrored by a C local.
    fn declare_named_cached(&mut self, ty: &str, name: &str, init: String) -> String {
        self.declare_named_with(ty, name, init, true)
    }
    fn declare_named_with(&mut self, ty: &str, name: &str, init: String, cache: bool) -> String {
        let registered = match self.layout.declare(ty, name) {
            Ok(access) => {
                if cache && self.layout.storage() == FrameStorage::CoFrame {
                    self.cached_fields.register(ty, name, false);
                }
                access
            }
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
            let declaration_index = self.declarations.len();
            let target = self.defer_declaration(ty, name, false);
            self.line(format!(
                "{target} = {init}; /*__llg_local_use_{declaration_index}__*/"
            ));
            name.to_owned()
        } else {
            self.line(format!("{} = {init};", declaration(ty, name)));
            registered
        }
    }
    fn declare(&mut self, ty: &str, purpose: &str, init: String) -> String {
        let name = self.name(purpose);
        self.declare_named(ty, &name, init)
    }
    fn loop_variable(&mut self, ty: &str, purpose: &str) -> (String, String) {
        let name = self.name(purpose);
        if self.layout.storage() == FrameStorage::CoFrame {
            match self.layout.declare(ty, &name) {
                Ok(_) => {}
                Err(error) => {
                    self.declaration_error.get_or_insert(error);
                }
            }
            let target = self.defer_declaration(ty, &name, false);
            (name, target)
        } else {
            (name.clone(), declaration(ty, &name))
        }
    }
    fn declaration_target_named(&mut self, ty: &str, name: &str) -> (String, String) {
        if self.layout.storage() == FrameStorage::CoFrame {
            match self.layout.declare(ty, name) {
                Ok(_) => {}
                Err(error) => {
                    self.declaration_error.get_or_insert(error);
                }
            }
            let target = self.defer_declaration(ty, name, false);
            (name.to_owned(), target)
        } else {
            (name.to_owned(), declaration(ty, name))
        }
    }
    fn declare_array(&mut self, ty: &str, purpose: &str, count: usize) -> String {
        let name = self.name(purpose);
        let array_ty = format!("{ty}[{count}]");
        let registered = match self.layout.declare(&array_ty, &name) {
            Ok(access) => access,
            Err(error) => {
                self.declaration_error.get_or_insert(error);
                name.clone()
            }
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            let declaration = self.defer_declaration(&array_ty, &name, true);
            self.line(format!("{declaration};"));
            self.line(format!("memset({name}, 0, sizeof({name}));"));
            name
        } else {
            self.line(format!("{ty} {name}[{count}] = {{0}};"));
            registered
        }
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
        let registered = match self.layout.declare(&array_ty, &name) {
            Ok(access) => access,
            Err(error) => {
                self.declaration_error.get_or_insert(error);
                name.clone()
            }
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            let declaration = self.defer_declaration(&array_ty, &name, true);
            self.line(format!("{declaration};"));
            self.line(format!(
                "memcpy({name}, ({ty}[]){{ {entries} }}, sizeof({name}));"
            ));
            name
        } else {
            self.line(format!("{ty} {name}[{count}] = {{ {entries} }};"));
            registered
        }
    }

    /// Array argument of a runtime arm that copies it before returning (see
    /// the copy contract on each arm in `llg_rt.h`). A coroutine passes it as
    /// a compound literal, which lives for the enclosing block and therefore
    /// covers the arm call, so it needs neither a frame field nor a `memcpy`.
    /// `count` must be nonzero.
    fn arm_array(&mut self, ty: &str, purpose: &str, count: usize, entries: &str) -> String {
        if self.layout.storage() == FrameStorage::CoFrame {
            format!("({ty}[]){{ {entries} }}")
        } else {
            self.declare_array_init(ty, purpose, count, entries)
        }
    }

    fn defer_declaration(&mut self, ty: &str, name: &str, standalone: bool) -> String {
        let index = self.declarations.len();
        self.declarations.push(DeferredDeclaration {
            ty: ty.to_owned(),
            name: name.to_owned(),
            standalone,
        });
        format!("__llg_declaration_{index}__")
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
            // suspended. A native C stack address cannot survive a stackless
            // return, so the registered scope owns stable storage.
            let owner = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(double), NULL)".to_owned(),
            );
            let pointer = self.declare_named_cached(
                "double*",
                &pointer,
                format!("(double*)llg_value_scope_object({owner})"),
            );
            self.line(format!("*{pointer} = 0.0;"));
            pointer
        } else {
            self.declare_named_cached(
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
        if self.layout.storage() == FrameStorage::CoFrame && !self.resume_numbers.is_empty() {
            format!("    F->_llg_frame_base = llg_value_scope_mark();\n    F->_llg_temp_scope = llg_value_scope_begin({});\n    F->_llg_t = llg_value_scope_values(F->_llg_temp_scope);\n    (void)F->_llg_t;\n", self.slots.len())
        } else {
            format!("{}    llg_value_scope_t* _llg_frame_base = llg_value_scope_mark();\n    llg_value_scope_t* _llg_temp_scope = llg_value_scope_begin({});\n    sv4_t* _llg_t = llg_value_scope_values(_llg_temp_scope);\n    (void)_llg_t;\n", if self.layout.storage() == FrameStorage::CoFrame { "    (void)F;\n    (void)ch;\n" } else { "" }, self.slots.len())
        }
    }
    pub(super) fn macro_epilogue(&self) -> &'static str {
        ""
    }
    pub(super) fn body(&self) -> &str {
        &self.code
    }
    pub(super) fn into_coframe(mut self) -> Result<CoroutineBody, String> {
        // Emission can fold an expression which still has a conservative call
        // site in the pre-emission execution analysis. Preserve storage and
        // descriptor offsets for those unreachable sites even though no call
        // expression consumed their slots.
        if let Some(site) = self.pending_sites.front() {
            return Err(format!(
                "coroutine suspension site {} ({:?}) was not emitted",
                site.resume, site.operation
            ));
        }
        if !self.brace_kinds.is_empty() {
            self.structural_error.get_or_insert_with(|| {
                format!(
                    "generated coroutine body has {} unclosed C brace(s)",
                    self.brace_kinds.len()
                )
            });
        }
        self.layout.finish_blocks()?;
        if let Some(error) = self
            .declaration_error
            .take()
            .or(self.structural_error.take())
        {
            Err(error)
        } else {
            for (original, flattened) in self.layout.finalize_paths()? {
                self.code = self
                    .code
                    .replace(&format!("F->{original}"), &format!("F->{flattened}"));
            }
            let layout = &self.layout;
            self.cached_fields
                .decide(&self.code, |name| layout.field_access(name).is_some());
            let declarations = self.resolve_declarations()?;
            let resolved = self.resolve_local_uses(&declarations)?;
            self.code = self
                .cached_fields
                .expand_reloads(&resolved, |name| self.layout.field_access(name))?;
            let rewritten = self.rewrite_frame_accesses(&self.code);
            let loads = self
                .cached_fields
                .prologue_loads(|name| self.layout.field_access(name));
            Ok(CoroutineBody {
                body: format!("{loads}{rewritten}"),
                cached_locals: self.cached_fields.locals(),
                layout: self.layout,
            })
        }
    }

    fn resolve_declarations(&self) -> Result<String, String> {
        const PREFIX: &str = "__llg_declaration_";
        let mut output = String::with_capacity(self.code.len());
        let mut rest = self.code.as_str();
        while let Some(start) = rest.find(PREFIX) {
            output.push_str(&rest[..start]);
            let suffix = &rest[start + PREFIX.len()..];
            let end = suffix
                .find("__")
                .ok_or_else(|| "unterminated deferred coroutine declaration".to_owned())?;
            let index = suffix[..end]
                .parse::<usize>()
                .map_err(|_| "invalid deferred coroutine declaration".to_owned())?;
            let record = self
                .declarations
                .get(index)
                .ok_or_else(|| format!("unknown deferred coroutine declaration {index}"))?;
            if let Some(access) = self.layout.field_access(&record.name) {
                if !record.standalone {
                    if self.cached_fields.is_cached(&record.name) {
                        output.push_str(&record.name);
                        output.push_str(" = ");
                    }
                    output.push_str("F->");
                    output.push_str(access);
                }
            } else {
                output.push_str(&declaration(&record.ty, &record.name));
            }
            rest = &suffix[end + 2..];
        }
        output.push_str(rest);
        Ok(output)
    }

    fn resolve_local_uses(&self, code: &str) -> Result<String, String> {
        const PREFIX: &str = "/*__llg_local_use_";
        const SUFFIX: &str = "__*/";
        let uses = identifier_counts(code);
        let mut output = String::with_capacity(code.len());
        let mut rest = code;
        while let Some(start) = rest.find(PREFIX) {
            output.push_str(&rest[..start]);
            let suffix = &rest[start + PREFIX.len()..];
            let end = suffix
                .find(SUFFIX)
                .ok_or_else(|| "unterminated deferred coroutine local use".to_owned())?;
            let index = suffix[..end]
                .parse::<usize>()
                .map_err(|_| "invalid deferred coroutine local use".to_owned())?;
            let record = self
                .declarations
                .get(index)
                .ok_or_else(|| format!("unknown deferred coroutine local use {index}"))?;
            if self.layout.field_access(&record.name).is_none()
                && uses.get(record.name.as_str()).copied().unwrap_or(0) <= 1
            {
                output.push_str("\n    (void)sizeof(");
                output.push_str(&record.name);
                output.push_str(");");
            }
            rest = &suffix[end + SUFFIX.len()..];
        }
        output.push_str(rest);
        Ok(output)
    }
}

fn identifier_counts(text: &str) -> HashMap<&str, usize> {
    let bytes = text.as_bytes();
    let mut counts = HashMap::new();
    let mut index = 0;
    let mut quoted = None;
    let mut escaped = false;
    while index < bytes.len() {
        if let Some(quote) = quoted {
            let byte = bytes[index];
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
        } else if matches!(bytes[index], b'\'' | b'"') {
            quoted = Some(bytes[index]);
            index += 1;
        } else if bytes[index] == b'_' || bytes[index].is_ascii_alphabetic() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
            {
                index += 1;
            }
            *counts.entry(&text[start..index]).or_insert(0) += 1;
        } else {
            index += 1;
        }
    }
    counts
}

fn is_structural_block_open(segment: &str) -> bool {
    if segment.is_empty() {
        return true;
    }
    let segment = segment.trim_start_matches('}').trim_start();
    ["if", "else", "for", "while", "switch", "do"]
        .into_iter()
        .any(|keyword| {
            segment == keyword
                || segment
                    .strip_prefix(keyword)
                    .is_some_and(|rest| rest.starts_with([' ', '(']))
        })
}

fn has_statement_boundary(text: &str) -> bool {
    let mut quoted = None;
    let mut escaped = false;
    for byte in text.bytes() {
        if let Some(quote) = quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quoted = Some(byte);
        } else if byte == b';' {
            return true;
        }
    }
    false
}

/// Runtime calls corresponding to Phase 2 resume sites. Direct coroutine
/// calls arm the same probe explicitly at their emission site.
fn is_runtime_suspension(text: &str) -> bool {
    [
        "LLG_CO_AWAIT(",
        "LLG_CO_CALL(",
        "LLG_CO_CALL_ANCHOR(",
        "LLG_CO_CALL_ARENA(",
        "llg_join(",
        "llg_process_suspend(",
        "llg_process_await(",
        "llg_semaphore_get(",
        "llg_mailbox_get_value(",
        "llg_mailbox_put_value(",
        "llg_rt_stop_with_level(",
    ]
    .into_iter()
    .any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests;
