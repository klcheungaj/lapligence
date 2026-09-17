//! Native expression owners use the same unwind stack as packed temporaries.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeKind { String, Chandle, Process }

impl NativeKind {
    pub(super) fn c_type(self) -> &'static str {
        match self { Self::String => "llg_string_t", Self::Chandle => "void*", Self::Process => "llg_process_handle_t*" }
    }
    fn destructor(self) -> &'static str {
        match self { Self::String => "llg_owned_string_drop", Self::Chandle => "NULL", Self::Process => "llg_owned_process_drop" }
    }
}

#[derive(Clone)]
pub(super) struct NativeBinding {
    pub(super) address: String,
    pub(super) kind: NativeKind,
    pub(super) automatic: bool,
}

#[must_use]
pub(super) struct NativeValue {
    pub(super) address: String,
    pub(super) scope: String,
    pub(super) kind: NativeKind,
}
impl NativeValue {
    pub(super) fn code(&self) -> String { format!("*({})", self.address) }
    pub(super) fn take_string(&self) -> String { format!("llg_string_take({})", self.address) }
}

pub(in crate::sim::emit_c) fn helpers(out: &mut String) {
    out.push_str("static void llg_owned_string_drop(void* p) { llg_string_destroy((llg_string_t*)p); }\n\
        static void llg_owned_process_drop(void* p) { llg_process_release(*(llg_process_handle_t**)p); }\n\n");
}

impl Frame<'_, '_> {
    pub(super) fn native_reserve(&mut self, kind: NativeKind) -> NativeValue {
        let scope = self.name("native_scope");
        let pointer = self.name("native");
        let ty = kind.c_type();
        self.line(format!("llg_value_scope_t* {scope} = llg_value_scope_begin_object(sizeof({ty}), {});", kind.destructor()));
        self.line(format!("{ty}* {pointer} = ({ty}*)llg_value_scope_object({scope});"));
        NativeValue { address: pointer, scope, kind }
    }
    pub(super) fn native_value(&mut self, kind: NativeKind, code: String) -> NativeValue {
        let value = self.native_reserve(kind);
        self.line(format!("{} = {code};", value.code()));
        value
    }
    pub(super) fn native_discard(&mut self, value: NativeValue) {
        self.line(format!("llg_value_scope_end({});", value.scope));
    }
    pub(super) fn native_local(&mut self, name: &str, kind: NativeKind) -> NativeBinding {
        let value = self.native_reserve(kind);
        let binding = NativeBinding { address: value.address, kind, automatic: true };
        self.native_bindings.last_mut().expect("native scope").insert(name.to_owned(), binding.clone());
        binding
    }
    pub(super) fn native_lookup(&mut self, name: &str, kind: NativeKind) -> Result<NativeBinding, String> {
        if let Some(binding) = self.native_access_binding(name, kind)? { return Ok(binding); }
        if let Some(address) = name.strip_prefix('*') {
            return self.native_address(address, kind);
        }
        for scope in self.native_bindings.iter().rev() {
            if let Some(binding) = scope.get(name) {
                if binding.kind != kind { return Err(format!("native type mismatch for {name}")); }
                return Ok(binding.clone());
            }
        }
        let object = self.ctx.model.objects.iter().find(|object| object.c_name == name);
        let matches = object.is_some_and(|object| matches!((object.ty, kind),
            (IrObjectType::String, NativeKind::String) | (IrObjectType::Chandle | IrObjectType::Semaphore, NativeKind::Chandle)
            | (IrObjectType::Process, NativeKind::Process)));
        let string_local = kind == NativeKind::String && self.ctx.model.funcs.iter()
            .any(|function| function.locals.iter().any(|local| local.string && local.c_name() == name));
        if matches || string_local {
            return Ok(NativeBinding { address: format!("&{name}"), kind, automatic: false });
        }
        Err(pending(&format!("unresolved native storage {name}")))
    }
    pub(super) fn native_address(&mut self, address: &str, kind: NativeKind) -> Result<NativeBinding, String> {
        if let Some(name) = address.strip_prefix('&') { return self.native_lookup(name, kind); }
        if let Some(function) = self.ctx.func {
            for (index, formal) in function.formals.iter().enumerate() {
                let expected = if formal.string { NativeKind::String } else { NativeKind::Chandle };
                if (formal.string || formal.chandle) && expected == kind &&
                    ((formal.is_out && address == format!("o{index}")) || (formal.is_ref() && address == format!("r{index}"))) {
                    return Ok(NativeBinding { address: address.to_owned(), kind, automatic: true });
                }
            }
        }
        Err(pending(&format!("opaque native address {address}")))
    }
}
