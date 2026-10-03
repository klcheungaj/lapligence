//! Suspendable calls exchange arguments through caller-owned callee frames;
//! registered scopes remain responsible for payload cleanup and copy-out.

use super::native::{NativeKind, NativeValue};
use super::*;

pub(super) enum CallValue {
    Numeric(Value),
    Native(NativeValue),
}

impl Frame<'_, '_> {
    fn coroutine_call(
        &mut self,
        function_index: usize,
        function: &IrFunc,
        parameters: &[String],
    ) -> Result<(), String> {
        if function.ret.is_some() || function.ret_string || function.ret_chandle {
            return Err("a suspendable subprogram cannot return a value".to_owned());
        }
        let frame_type = format!("{}_frame_t", function.c_name);
        let slot = self.take_call_slot(function_index, &frame_type)?;
        let mut arena_storage = None;
        let child = match slot.mechanism {
            crate::sim::execution::CallMechanism::Polled { .. } => {
                format!("F->{}", slot.path)
            }
            crate::sim::execution::CallMechanism::Anchored => {
                format!("F->{}.f", slot.path)
            }
            crate::sim::execution::CallMechanism::Arena => {
                let storage = self.declare("llg_co_anchor_t*", "arena_call", "NULL".to_owned());
                self.line(format!(
                    "LLG_CO_ARENA_ENTER(ch, &{}_desc, {storage});",
                    function.c_name
                ));
                arena_storage = Some(storage.clone());
                format!("(*({frame_type}*)LLG_CO_ANCHOR_FRAME({storage}))")
            }
        };
        if slot.mechanism != crate::sim::execution::CallMechanism::Arena {
            self.line("#ifdef LLG_CO_DEBUG");
            self.line(format!(
                "LLG_CO_DEBUG_POISON_FRAME(&{child}, sizeof({child}));"
            ));
            self.line("#endif");
        }
        let fields = super::super::model::owned_func_param_fields(function);
        if fields.len() != parameters.len() {
            return Err("coroutine call argument layout mismatch".to_owned());
        }
        for ((_, name), value) in fields.iter().zip(parameters) {
            self.line(format!("{child}.{name} = {value};"));
        }
        match slot.mechanism {
            crate::sim::execution::CallMechanism::Polled { .. } => self.line(format!(
                "LLG_CO_CALL(co, ch, {}, {}, &{child}.co);",
                slot.resume, function.c_name
            )),
            crate::sim::execution::CallMechanism::Anchored => self.line(format!(
                "LLG_CO_CALL_ANCHOR(co, ch, {}, &{}_desc, &F->{}.an);",
                slot.resume, function.c_name, slot.path
            )),
            crate::sim::execution::CallMechanism::Arena => {
                let storage = arena_storage
                    .ok_or_else(|| "arena coroutine call lost its anchor slot".to_owned())?;
                self.line(format!(
                    "LLG_CO_CALL_ARENA(co, ch, {}, &{}_desc, {storage});",
                    slot.resume, function.c_name
                ));
            }
        }
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))] // test-only entry point for owned-call boundary tests
    pub(super) fn call_values(
        &mut self,
        f: usize,
        args: &[IrCallArg],
        depth: IrDepth,
    ) -> Result<Option<CallValue>, String> {
        self.call_target(f, args, depth, None, false, None)
    }

    fn call_target(
        &mut self,
        f: usize,
        args: &[IrCallArg],
        depth: IrDepth,
        receiver: Option<&IrChandleExpr>,
        virtual_dispatch: bool,
        virtual_call: Option<&IrVirtualCall>,
    ) -> Result<Option<CallValue>, String> {
        if !self.allow_calls {
            return Err(pending("subprogram calls in declaration initialization"));
        }
        let function = self.ctx.model.func(f).clone();
        if model::inline_template(&function) {
            return Err("inline-expanded calls must be expanded before C emission".to_owned());
        }
        model::check_function(&function)?;
        if self.read_only_callback {
            return Err(pending("native or impure calls in evaluator callbacks"));
        }
        // A native return slot must precede the call mark: argument cleanup
        // must not invalidate the returned value, including aliased copyouts.
        let native_result = if function.ret_string {
            Some(self.native_reserve(NativeKind::String))
        } else if function.ret_chandle {
            Some(self.native_reserve(NativeKind::Chandle))
        } else {
            None
        };
        let marker = self.declare(
            "llg_value_scope_t*",
            "call_mark",
            "llg_value_scope_mark()".to_owned(),
        );
        self.bindings.push(HashMap::new());
        self.native_bindings.push(HashMap::new());
        let mut parameters = Vec::new();
        let mut callee = function.c_name.clone();
        if let Some(call) = virtual_call {
            parameters.push(self.chandle(&call.receiver)?);
            callee = format!("llg_vif_call_{}_{}", call.interface, call.method);
        } else if let Some(receiver) = receiver {
            parameters.push(self.chandle(receiver)?);
            if virtual_dispatch {
                let slot = function
                    .virtual_slot
                    .ok_or_else(|| "virtual call has no slot".to_owned())?;
                callee = format!("llg_class_dispatch_{slot}");
            }
        } else if function.receiver_class.is_some() {
            return Err("class method call has no receiver".to_owned());
        }
        // Tie retained queue-reference cells to the same lexical cleanup stack.
        // This covers normal return, named disable and nonlocal process exit.
        if args
            .iter()
            .any(|arg| matches!(arg, IrCallArg::RefAddr { .. }))
        {
            self.line("llg_ref_scope_begin_owned();");
        }
        let mut owners = Vec::new();
        let mut copyouts = Vec::new();
        let mut native_owners = Vec::new();
        let mut string_copyouts = Vec::new();
        // The IR stores arguments in the C ABI order (addresses, inputs).
        // Evaluate this explicit order, never nested C argument expressions.
        let order = function
            .formals
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_address())
            .chain(
                function
                    .formals
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| !p.is_address()),
            );
        if args.len() != function.formals.len() {
            return Err("call arity mismatch".to_owned());
        }
        for ((index, formal), argument) in order.zip(args) {
            match argument {
                IrCallArg::Val(expr) => {
                    let value = self.expression(expr)?;
                    let value = self.convert(
                        value,
                        if formal.real { 0 } else { formal.width },
                        formal.signed,
                        formal.two_state,
                        formal.shortreal,
                    );
                    self.bindings
                        .last_mut()
                        .expect("call binding scope")
                        .insert(
                            crate::sim::ir::call_argument_name(index),
                            Binding {
                                address: format!("&({})", value.code),
                                width: value.width,
                                signed: value.signed,
                                two_state: formal.two_state,
                                shortreal: formal.shortreal,
                                automatic: true,
                            },
                        );
                    parameters.push(value.code.clone());
                    owners.push(value);
                }
                IrCallArg::StringVal(expression) => {
                    let value = self.string(expression)?;
                    parameters.push(value.code());
                    native_owners.push(value);
                }
                IrCallArg::ChandleVal(expression) => parameters.push(self.chandle(expression)?),
                IrCallArg::EventVal(event) => {
                    // Pass the object the handle names now, not the handle's
                    // storage, so rebinding the actual cannot move the callee.
                    let address = self.event_address(event)?;
                    let pointer = self.scalar("llg_event_t*", address);
                    parameters.push(format!(
                        "(llg_event_t){{ {pointer} ? {pointer}->object : NULL }}"
                    ));
                }
                IrCallArg::StringOutAddr(address)
                | IrCallArg::StringRefAddr { addr: address, .. } => {
                    parameters.push(self.native_address(address, NativeKind::String)?.address);
                }
                IrCallArg::ChandleAddr(address) | IrCallArg::ChandleRefAddr(address) => {
                    parameters.push(self.native_address(address, NativeKind::Chandle)?.address);
                }
                IrCallArg::StringOutTemp {
                    name,
                    init,
                    writeback,
                    storage_addr,
                    ..
                } => {
                    let temporary = self.native_local(name, NativeKind::String);
                    if let Some(init) = init {
                        self.string_assign(&temporary.address, init)?;
                    }
                    let target = self.native_address(writeback, NativeKind::String)?;
                    let storage = if let Some(address) = storage_addr {
                        let storage = self.native_address(address, NativeKind::String)?;
                        self.line(format!(
                            "llg_string_move({}, llg_string_clone({}));",
                            storage.address, temporary.address
                        ));
                        storage
                    } else {
                        temporary
                    };
                    parameters.push(storage.address.clone());
                    string_copyouts.push((target, storage));
                }
                IrCallArg::OutAddr(addr) => {
                    let binding = self.address(addr)?;
                    if binding.width != if formal.real { 0 } else { formal.width } {
                        return Err("output argument storage width mismatch".to_owned());
                    }
                    parameters.push(binding.address);
                }
                IrCallArg::OutTemp {
                    name,
                    init,
                    writeback,
                    storage_addr,
                    selector_inits,
                    ..
                } => {
                    for (name, width, signed, two_state, initial) in selector_inits {
                        self.local(name, *width, *signed, *two_state, Some(initial))?;
                    }
                    let default = formal.fixed_default.as_ref().map(|value| {
                        IrExpr::new(
                            IrExprKind::Const(value.clone()),
                            value.width,
                            value.signed,
                            None,
                        )
                    });
                    self.local(
                        name,
                        if formal.real { 0 } else { formal.width },
                        formal.signed,
                        formal.two_state,
                        init.as_deref().or(default.as_ref()),
                    )?;
                    let temporary = self
                        .lookup(name)
                        .ok_or_else(|| "missing call temporary".to_owned())?;
                    let target = self.capture_assignment(writeback)?;
                    let storage = if let Some(address) = storage_addr {
                        let storage = self.address(address)?;
                        // Only inout has a copy-in value. A static output
                        // keeps its persistent formal until the callee writes it;
                        // a fresh caller temporary is not an output initializer.
                        if init.is_some() {
                            let initial = self.read_binding(&temporary);
                            let initial = self.convert(
                                initial,
                                storage.width,
                                storage.signed,
                                storage.two_state,
                                storage.shortreal,
                            );
                            if storage.width == 0 {
                                self.line(format!("*({}) = {};", storage.address, initial.code));
                            } else {
                                self.line(format!(
                                    "sv4_move({}, &{});",
                                    storage.address, initial.code
                                ));
                            }
                            self.discard(initial);
                        }
                        storage
                    } else {
                        temporary
                    };
                    parameters.push(storage.address.clone());
                    copyouts.push((target, storage));
                }
                IrCallArg::RefAddr {
                    lhs,
                    read,
                    width,
                    signed,
                    two_state,
                    ..
                } => {
                    parameters
                        .push(self.reference_argument(lhs, read, *width, *signed, *two_state)?);
                }
            }
        }
        parameters.push(depth.code().to_owned());
        let call_mark = self.cancellation_mark();
        let coroutine = self.coroutine_functions.contains(&f);
        let result = if coroutine {
            self.coroutine_call(f, &function, &parameters)?;
            None
        } else {
            let invocation = format!("{callee}({})", parameters.join(", "));
            let result = if let Some(value) = native_result {
                self.line(format!("{} = {invocation};", value.code()));
                Some(CallValue::Native(value))
            } else if let Some(ty) = function.ret {
                Some(CallValue::Numeric(self.value(
                    invocation,
                    ty.width(),
                    ty.signed(),
                )))
            } else {
                self.line(format!("{invocation};"));
                None
            };
            result
        };
        // A coroutine call is a resume point, recorded by `line`. A plain
        // callee, or any override behind dynamic dispatch, may disable an
        // activation of this process; copy-out must then be skipped.
        let dynamic = virtual_dispatch || virtual_call.is_some();
        if !coroutine && (dynamic || self.callee_may_disable(f)) {
            self.cancellation_point();
        }
        self.cancellation_check_covering(call_mark)?;
        for value in owners {
            self.discard(value);
        }
        for (target, storage) in copyouts {
            let mark = self.cancellation_mark();
            let value = self.read_binding(&storage);
            for (target, piece) in self.prepare_captured_assignment(target, value)? {
                self.store(&target, piece, false, "0")?;
                self.release_target(target);
            }
            self.cancellation_check_since(mark)?;
        }
        for (target, storage) in string_copyouts {
            self.line(format!(
                "llg_string_move({}, llg_string_clone({}));",
                target.address, storage.address
            ));
        }
        for value in native_owners {
            self.native_discard(value);
        }
        self.native_bindings.pop();
        self.bindings.pop();
        self.line(format!("llg_value_scopes_end_since({marker});"));
        Ok(result)
    }

    #[cfg_attr(not(test), allow(dead_code))] // test-only entry point for owned-call boundary tests
    pub(super) fn native_call(
        &mut self,
        function: usize,
        args: &[IrCallArg],
        depth: IrDepth,
        kind: NativeKind,
    ) -> Result<NativeValue, String> {
        match self.call_values(function, args, depth)? {
            Some(CallValue::Native(value)) if value.kind == kind => Ok(value),
            _ => Err("native call result type mismatch".to_owned()),
        }
    }

    pub(super) fn native_method_call(
        &mut self,
        function: usize,
        args: &[IrCallArg],
        depth: IrDepth,
        kind: NativeKind,
        receiver: Option<&IrChandleExpr>,
        virtual_dispatch: bool,
    ) -> Result<NativeValue, String> {
        match self.call_target(function, args, depth, receiver, virtual_dispatch, None)? {
            Some(CallValue::Native(value)) if value.kind == kind => Ok(value),
            _ => Err("native method result type mismatch".to_owned()),
        }
    }

    pub(super) fn call_expression(&mut self, call: &IrCallExpr) -> Result<Value, String> {
        Ok(
            match self.call_target(
                call.f,
                &call.args,
                call.depth,
                call.receiver.as_ref(),
                call.virtual_dispatch,
                call.virtual_call.as_ref(),
            )? {
                Some(CallValue::Numeric(value)) => value,
                Some(CallValue::Native(_)) => {
                    return Err("native call used as a packed expression".to_owned())
                }
                None if call.void_x => self.value("sv4_x(1, 0)".to_owned(), 1, false),
                None => return Err("void call has no expression value".to_owned()),
            },
        )
    }

    pub(super) fn call_statement(&mut self, call: &IrCall) -> Result<(), String> {
        self.begin_block(&[]);
        let function = self.ctx.model.func(call.f).clone();
        for (name, index, initial) in &call.temps {
            let formal = &function.formals[*index];
            let default = formal.fixed_default.as_ref().map(|value| {
                IrExpr::new(
                    IrExprKind::Const(value.clone()),
                    value.width,
                    value.signed,
                    None,
                )
            });
            self.local(
                name,
                if formal.real { 0 } else { formal.width },
                formal.signed,
                formal.two_state,
                initial.as_ref().or(default.as_ref()),
            )?;
        }
        let mut captured = Vec::new();
        for (lhs, name, width, signed) in &call.copyouts {
            captured.push((self.capture_assignment(lhs)?, name, *width, *signed));
        }
        if let Some(value) = self.call_target(
            call.f,
            &call.args,
            call.depth,
            call.receiver.as_ref(),
            call.virtual_dispatch,
            call.virtual_call.as_ref(),
        )? {
            match value {
                CallValue::Numeric(value) => self.discard(value),
                CallValue::Native(value) => self.native_discard(value),
            }
        }
        for (target, name, width, signed) in captured {
            let mark = self.cancellation_mark();
            let binding = self
                .lookup(name)
                .ok_or_else(|| format!("unknown copyout {name}"))?;
            let value = self.read_binding(&binding);
            let value = self.convert(value, width, signed, false, false);
            for (target, piece) in self.prepare_captured_assignment(target, value)? {
                self.store(&target, piece, false, "0")?;
                self.release_target(target);
            }
            self.cancellation_check_since(mark)?;
        }
        self.end_block();
        Ok(())
    }
}
