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
        let fields = super::super::model::owned_frame_param_fields(function);
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

    /// Suspendable class virtual call (SIM-011): the slot's arena-dispatch
    /// helper selects the implementation from the receiver evaluated at the
    /// call, so a later rebinding of the receiver variable cannot change the
    /// running activation's `this`. A suspending implementation gets an
    /// arena frame entered here; any other ran as a plain call and the helper
    /// returned NULL.
    fn dispatch_coroutine_call(
        &mut self,
        function_index: usize,
        function: &IrFunc,
        parameters: &[String],
    ) -> Result<(), String> {
        if function.ret.is_some() || function.ret_string || function.ret_chandle {
            return Err("a suspendable subprogram cannot return a value".to_owned());
        }
        let virtual_slot = function
            .virtual_slot
            .ok_or_else(|| "virtual call has no slot".to_owned())?;
        let slot = self.take_call_slot(function_index, "llg_co_anchor_t")?;
        if slot.mechanism != crate::sim::execution::CallMechanism::Arena {
            return Err(format!(
                "suspendable virtual call site {} is not an arena call",
                slot.resume
            ));
        }
        let storage = self.declare("llg_co_anchor_t*", "arena_call", "NULL".to_owned());
        let mut arguments = vec!["ch".to_owned()];
        arguments.extend(parameters.iter().cloned());
        self.line(format!(
            "{storage} = llg_class_co_enter_{virtual_slot}({});",
            arguments.join(", ")
        ));
        self.line(format!("if ({storage}) {{"));
        self.line(format!(
            "LLG_CO_CALL_ARENA(co, ch, {}, {storage}->desc, {storage});",
            slot.resume
        ));
        self.line("}");
        Ok(())
    }

    /// Arena call from a recursive subprogram's coroutine into its own
    /// component. `parameters` are the callee's plain parameters in order,
    /// with packed inputs as by-value descriptors for a static callee frame.
    /// The result destination is caller storage that survives the resume: a
    /// temporary slot, a reserved native object or a frame-resident scalar.
    fn recursive_call(
        &mut self,
        function: &IrFunc,
        target: crate::sim::execution::CallTarget,
        parameters: Vec<String>,
        native_result: Option<NativeValue>,
    ) -> Result<Option<CallValue>, String> {
        use crate::sim::execution::CallTarget;
        let (result, destination) = if let Some(value) = native_result {
            let destination = value.address.clone();
            (Some(CallValue::Native(value)), Some(destination))
        } else if let Some(ty) = function.ret {
            if ty.width() == 0 {
                let name = self.scalar("double", "0.0".to_owned());
                let destination = format!("&{name}");
                (
                    Some(CallValue::Numeric(Value {
                        code: name,
                        width: 0,
                        signed: ty.signed(),
                        fill: None,
                        slot: None,
                        borrowed_address: None,
                    })),
                    Some(destination),
                )
            } else {
                let value = self.reserve(ty.width(), ty.signed());
                let destination = format!("&{}", value.code);
                (Some(CallValue::Numeric(value)), Some(destination))
            }
        } else {
            (None, None)
        };
        let storage = self.declare("llg_co_anchor_t*", "arena_call", "NULL".to_owned());
        match target {
            CallTarget::Static(callee) => {
                let name = &self.ctx.model.func(callee).c_name;
                let descriptor = format!("{name}_co_desc");
                let child = format!("(*({name}_co_frame_t*)LLG_CO_ANCHOR_FRAME({storage}))");
                self.line(format!("LLG_CO_ARENA_ENTER(ch, &{descriptor}, {storage});"));
                let fields = super::super::model::owned_frame_param_fields(function);
                if fields.len() != parameters.len() {
                    return Err("recursive call argument layout mismatch".to_owned());
                }
                for ((_, field), value) in fields.iter().zip(&parameters) {
                    self.line(format!("{child}.{field} = {value};"));
                }
                if let Some(destination) = destination {
                    self.line(format!("{child}._llg_result = {destination};"));
                }
                let resume = self.next_recursive_resume();
                self.line(format!(
                    "LLG_CO_CALL_ARENA(co, ch, {resume}, &{descriptor}, {storage});"
                ));
                // A synchronous resume never yields to the scheduler, so only a
                // callee that may disable makes this a cancellation point, as
                // for a plain call (see `call_target`).
                if !self.callee_may_disable(callee) {
                    self.cancellation_points -= 1;
                }
            }
            CallTarget::Virtual(_) | CallTarget::Interface(..) => {
                let helper = match target {
                    CallTarget::Interface(interface, method) => {
                        format!("llg_vif_co_enter_{interface}_{method}")
                    }
                    _ => format!(
                        "llg_class_co_enter_{}",
                        function
                            .virtual_slot
                            .ok_or_else(|| "virtual call has no slot".to_owned())?
                    ),
                };
                let mut arguments = vec!["ch".to_owned()];
                arguments.extend(parameters);
                arguments.extend(destination);
                self.line(format!("{storage} = {helper}({});", arguments.join(", ")));
                // A null anchor means the selected implementation already ran
                // as a plain call (or dispatch failed and wrote the default).
                self.line(format!("if ({storage}) {{"));
                let resume = self.next_recursive_resume();
                self.line(format!(
                    "LLG_CO_CALL_ARENA(co, ch, {resume}, {storage}->desc, {storage});"
                ));
                self.line("}");
            }
        }
        Ok(result)
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
            .any(|arg| matches!(arg, IrCallArg::RefAddr { width, .. } if *width != 0))
        {
            self.line("llg_ref_scope_begin_owned();");
        }
        // Coroutine frames hold argument descriptors; plain functions borrow
        // packed inputs by address (see `model::functions::func_param_fields`).
        // Class virtual dispatch to a slot with a suspending implementation
        // enters the selected implementation through its arena-dispatch
        // helper with the plain parameters (SIM-011).
        let dispatch_coroutine = virtual_dispatch
            && virtual_call.is_none()
            && function
                .virtual_slot
                .is_some_and(|slot| self.suspendable_slots.contains(&slot));
        let coroutine = !dispatch_coroutine && self.coroutine_functions.contains(&f);
        // A recursive coroutine enters its own component through the chain
        // arena (see `execution::recursion`): statically with the callee's
        // frame, dynamically through an arena-dispatch helper taking the
        // plain parameters.
        let recursive_target =
            crate::sim::execution::CallTarget::of_call(f, virtual_dispatch, virtual_call);
        let recursive =
            !coroutine && !dispatch_coroutine && self.recursive_targets.contains(&recursive_target);
        let by_value_inputs = coroutine
            || (recursive
                && matches!(
                    recursive_target,
                    crate::sim::execution::CallTarget::Static(_)
                ));
        let mut owners = Vec::new();
        let mut copyouts = Vec::new();
        let mut native_owners = Vec::new();
        let mut fixed_copyouts = Vec::new();
        let mut native_copyouts = Vec::new();
        // Persistent handles written through an output or ref operand and
        // their values before the call: a changed handle publishes its
        // change marker after the callee returns (SIM-007).
        let mut handle_publishes = Vec::new();
        let mut container_copyouts = Vec::new();
        let mut string_copyouts = Vec::new();
        let mut real_copyouts = Vec::new();
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
                IrCallArg::FixedValue(value) => {
                    let shape = formal
                        .fixed_array
                        .ok_or("fixed operand requires descriptor formal")?;
                    let actual = if let IrFixedValue::Array(view) = value.as_ref() {
                        Some(self.fixed_view(view)?)
                    } else {
                        None
                    };
                    if formal.is_ref() {
                        parameters.push(actual.ok_or("fixed reference requires a storage view")?);
                    } else {
                        let storage = self.new_fixed_array(shape)?;
                        if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                            let source =
                                self.fixed_value(value, shape, self.ctx.model.array(shape).total)?;
                            self.line(format!(
                                "llg_fixed_array_copy({storage}, {source}, {}, 0);",
                                u8::from(self.ctx.model.array(shape).two_state)
                            ));
                        }
                        if formal.is_out {
                            fixed_copyouts.push((
                                actual.ok_or("fixed output requires a storage view")?,
                                storage.clone(),
                                self.ctx.model.array(shape).two_state,
                            ));
                        }
                        parameters.push(storage);
                    }
                }
                IrCallArg::NativeRefBound => {
                    // The specialization binds this `ref` formal to record
                    // leaves of its own (SIM-008); its parameters stay null.
                    parameters.push("NULL".to_owned());
                    parameters.extend(formal.native_companions.iter().map(|_| "NULL".to_owned()));
                }
                IrCallArg::NativeValue(value) => {
                    let actual = self.native_value_address(*value)?;
                    // A `ref` formal aliases the caller's value and its
                    // companion containers (SIM-008).
                    if formal.is_ref() {
                        parameters.push(actual);
                        for container in self.ctx.model.native_values[*value].companions.clone() {
                            parameters.push(format!("(void*)&{}", self.container_name(container)?));
                        }
                        continue;
                    }
                    // A function cannot suspend or be disabled by another
                    // process, and an activation value is private to its
                    // frame: an output or result written in place is then
                    // indistinguishable from copy-out at return, unless the
                    // same value is another operand of this call.
                    if formal.is_out
                        && !function.is_task
                        && self.ctx.model.native_values[*value].activation
                        && args
                            .iter()
                            .filter(
                                |other| matches!(other, IrCallArg::NativeValue(v) if v == value),
                            )
                            .count()
                            == 1
                    {
                        parameters.push(actual);
                        for container in self.ctx.model.native_values[*value].companions.clone() {
                            parameters.push(format!("(void*)&{}", self.container_name(container)?));
                        }
                        continue;
                    }
                    // Otherwise the callee gets a fresh value: inputs and
                    // inouts copy the actual in, outputs and results are
                    // copied back after the callee returns.
                    let callee = formal
                        .native_value
                        .ok_or("native operand requires a native-value formal")?;
                    let storage = self.new_native_value(self.ctx.model.native_values[callee].ty);
                    if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                        self.line(format!("llg_native_value_copy({storage}, {actual});"));
                    }
                    if formal.is_out {
                        native_copyouts.push((actual, storage.clone()));
                    }
                    parameters.push(storage);
                    // Companion containers follow the same copy-in/copy-out
                    // protocol as container operands (SIM-007).
                    let pairs = formal
                        .native_companions
                        .iter()
                        .copied()
                        .zip(self.ctx.model.native_values[*value].companions.clone())
                        .collect::<Vec<_>>();
                    for (callee, actual) in pairs {
                        let actual = self.container_name(actual)?;
                        let storage = self.new_container(callee)?;
                        let copy =
                            super::containers::copy_function(&self.ctx.model.containers[callee]);
                        if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                            self.line(format!("{copy}(&{storage}, &{actual});"));
                        }
                        if formal.is_out {
                            container_copyouts.push((copy, actual, storage.clone()));
                        }
                        parameters.push(format!("(void*)&{storage}"));
                    }
                }
                IrCallArg::Container(container) => {
                    let actual = self.container_name(*container)?;
                    // A `ref` formal aliases the caller's storage (SIM-008).
                    if formal.is_ref() {
                        parameters.push(format!("(void*)&{actual}"));
                        continue;
                    }
                    // Otherwise the callee receives fresh storage of its
                    // formal type: inputs copy the actual in, outputs and
                    // results are copied back after the callee returns.
                    let callee = formal
                        .container
                        .ok_or("container operand requires a container formal")?;
                    let storage = self.new_container(callee)?;
                    let copy = super::containers::copy_function(&self.ctx.model.containers[callee]);
                    if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                        self.line(format!("{copy}(&{storage}, &{actual});"));
                    }
                    if formal.is_out {
                        container_copyouts.push((copy, actual, storage.clone()));
                    }
                    parameters.push(format!("(void*)&{storage}"));
                }
                IrCallArg::ContainerValues { container, values } => {
                    // Element values are evaluated in order into fresh
                    // storage that only this operand references.
                    let storage = self.new_container(*container)?;
                    self.containers.insert(*container, storage.clone());
                    let build = if self.ctx.model.containers[*container].element.is_real() {
                        IrContainerStmt::AssignRealValues {
                            container: *container,
                            values: values.clone(),
                        }
                    } else {
                        IrContainerStmt::AssignValues {
                            container: *container,
                            values: values.clone(),
                        }
                    };
                    self.container_statement(&build)?;
                    parameters.push(format!("(void*)&{storage}"));
                }
                IrCallArg::NativeCall { value, call } => {
                    // The inner result is a fresh temporary that only this
                    // operand references, so the callee may own it directly.
                    let storage = self.new_native_value(self.ctx.model.native_values[*value].ty);
                    self.native_values.insert(*value, storage.clone());
                    self.declare_native_companions(*value)?;
                    self.call_statement(call)?;
                    parameters.push(storage);
                    for container in self.ctx.model.native_values[*value].companions.clone() {
                        parameters.push(format!("(void*)&{}", self.container_name(container)?));
                    }
                }
                IrCallArg::NativeLeaves {
                    ty,
                    leaves,
                    containers,
                } => {
                    let storage = self.new_native_value(*ty);
                    self.native_leaves_into(&storage, *ty, leaves)?;
                    parameters.push(storage);
                    // Container members are fresh copies of their sources.
                    for (callee, source) in
                        formal.native_companions.clone().into_iter().zip(containers)
                    {
                        let copy =
                            super::containers::copy_function(&self.ctx.model.containers[callee]);
                        let source = self.container_name(*source)?;
                        let storage = self.new_container(callee)?;
                        self.line(format!("{copy}(&{storage}, &{source});"));
                        parameters.push(format!("(void*)&{storage}"));
                    }
                }
                IrCallArg::RealArray(array) => {
                    let actual = self.real_array_base(*array)?;
                    if formal.is_ref() {
                        parameters.push(actual);
                        continue;
                    }
                    let shape = formal
                        .real_array
                        .ok_or("real-array operand requires a real-array formal")?;
                    let total = self.ctx.model.array(shape).total;
                    let storage = self.new_real_array(shape);
                    if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                        self.line(format!(
                            "memcpy({storage}, {actual}, sizeof(double) * {total}ULL);"
                        ));
                    }
                    if formal.is_out {
                        real_copyouts.push((actual, storage.clone(), total));
                    }
                    parameters.push(storage);
                }
                IrCallArg::RealArrayCall { array, call } => {
                    // The inner result is a fresh lexical array that only this
                    // operand references, so the callee may own it directly.
                    let storage = self.new_real_array(*array);
                    self.fixed_arrays.insert(*array, storage.clone());
                    self.call_statement(call)?;
                    parameters.push(storage);
                }
                IrCallArg::RealArrayValues(values) => {
                    let shape = formal
                        .real_array
                        .ok_or("real-array values require a real-array formal")?;
                    let shortreal = self.ctx.model.array(shape).shortreal;
                    let storage = self.new_real_array(shape);
                    for (cell, value) in values.iter().enumerate() {
                        let value = self.expression(value)?;
                        self.line(format!(
                            "{storage}[{cell}] = {};",
                            round_shortreal(value.real(), shortreal)
                        ));
                        self.discard(value);
                    }
                    parameters.push(storage);
                }
                IrCallArg::FixedArray(array) => {
                    let actual = self.fixed_array_address(*array)?;
                    if formal.is_ref() {
                        parameters.push(actual);
                    } else {
                        let storage = self.new_fixed_array(
                            formal
                                .fixed_array
                                .ok_or("descriptor operand requires descriptor formal")?,
                        )?;
                        if matches!(formal.mode, IrFormalMode::Input | IrFormalMode::Inout) {
                            self.line(format!(
                                "llg_fixed_array_copy({storage}, {actual}, {}, 0);",
                                u8::from(formal.two_state)
                            ));
                        }
                        if formal.is_out {
                            fixed_copyouts.push((
                                actual,
                                storage.clone(),
                                self.ctx.model.array(*array).two_state,
                            ));
                        }
                        parameters.push(storage);
                    }
                }

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
                    parameters.push(if by_value_inputs || value.width == 0 {
                        value.code.clone()
                    } else {
                        format!("&{}", value.code)
                    });
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
                IrCallArg::EventAddr(event) => parameters.push(self.event_address(event)?),
                IrCallArg::StringOutAddr(address)
                | IrCallArg::StringRefAddr { addr: address, .. } => {
                    parameters.push(self.native_address(address, NativeKind::String)?.address);
                }
                IrCallArg::ChandleAddr(address) | IrCallArg::ChandleRefAddr(address) => {
                    let address = self.native_address(address, NativeKind::Chandle)?.address;
                    if let Some(object) = self.ctx.model.objects.iter().find(|object| {
                        object.ty == crate::sim::ir::IrObjectType::Chandle
                            && address == format!("&{}", object.c_name)
                    }) {
                        let name = object.c_name.clone();
                        let previous = self.scalar("void*", name.clone());
                        handle_publishes.push((name, previous));
                    }
                    parameters.push(address);
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
                            "llg_string_assign({}, {});",
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
                    if formal.real {
                        parameters.push(self.real_reference_argument(lhs)?);
                        continue;
                    }
                    parameters
                        .push(self.reference_argument(lhs, read, *width, *signed, *two_state)?);
                }
            }
        }
        parameters.push(depth.code().to_owned());
        let call_mark = self.cancellation_mark();
        let result = if coroutine {
            self.coroutine_call(f, &function, &parameters)?;
            None
        } else if dispatch_coroutine {
            self.dispatch_coroutine_call(f, &function, &parameters)?;
            None
        } else if recursive {
            self.recursive_call(&function, recursive_target, parameters, native_result)?
        } else {
            let invocation = format!("{callee}({})", parameters.join(", "));
            let result = if let Some(value) = native_result {
                self.line(format!("{} = {invocation};", value.code()));
                Some(CallValue::Native(value))
            } else if super::super::model::owned_packed_result(&function) {
                // The callee replaces this initialized slot through its
                // trailing `_llg_result` destination parameter.
                let ty = function.ret.expect("packed result has a type");
                let value = self.reserve(ty.width(), ty.signed());
                parameters.push(format!("&{}", value.code));
                self.line(format!("{callee}({});", parameters.join(", ")));
                Some(CallValue::Numeric(value))
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
        if !coroutine && !dispatch_coroutine && (dynamic || self.callee_may_disable(f)) {
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
        for (target, storage, two_state) in fixed_copyouts {
            self.line(format!(
                "llg_fixed_array_copy({target}, {storage}, {}, 0);",
                u8::from(two_state)
            ));
        }
        for (target, storage, count) in real_copyouts {
            self.publish_real_cells(&target, &storage, count);
        }
        for (copy, target, storage) in container_copyouts {
            self.line(format!("{copy}(&{target}, &{storage});"));
        }
        for (target, storage) in native_copyouts {
            self.line(format!("llg_native_value_copy({target}, {storage});"));
        }
        for (name, previous) in handle_publishes {
            self.line(format!(
                "if ({name} != {previous}) llg_dependency_changed(&{name}_llg_dep);"
            ));
        }
        for (target, storage) in string_copyouts {
            self.line(format!(
                "llg_string_assign({}, {});",
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
