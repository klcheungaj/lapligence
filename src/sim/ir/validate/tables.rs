//! Tables.

use super::*;

impl Validator<'_> {
    pub(super) fn validate(&self) -> ValidationResult {
        if self.model.precision_fs == 0 {
            return self.fail("precision_fs", "scheduler precision must be non-zero");
        }

        for (idx, table) in self.model.udp_tables.iter().enumerate() {
            self.validate_udp_table(table, &format!("udp_tables[{idx}]"))?;
        }

        for (idx, object) in self.model.vpi_objects.iter().enumerate() {
            let path = format!("vpi_objects[{idx}]");
            if object.full_name.is_empty() || object.name.is_empty() {
                return self.fail(path, "VPI object names must be non-empty");
            }
            if object.full_name.contains('\0')
                || object.name.contains('\0')
                || object
                    .definition_name
                    .as_deref()
                    .is_some_and(|name| name.contains('\0'))
                || object
                    .file
                    .as_deref()
                    .is_some_and(|file| file.contains('\0'))
            {
                return self.fail(path, "VPI object metadata must not contain NUL bytes");
            }
            if object.signal.is_some() && object.array.is_some() {
                return self.fail(
                    path,
                    "VPI object cannot reference both signal and array storage",
                );
            }
            match object.kind {
                IrVpiObjectKind::Module => {
                    if object.signal.is_some() || object.array.is_some() {
                        return self.fail(path, "VPI module cannot reference value storage");
                    }
                }
                IrVpiObjectKind::RegArray => {
                    let Some(array) = object.array else {
                        return self.fail(path, "VPI array is missing array storage");
                    };
                    if array >= self.model.arrays.len() {
                        return self.fail(path, "VPI array references an out-of-bounds IR array");
                    }
                    if object.signal.is_some() {
                        return self.fail(path, "VPI array cannot reference signal storage");
                    }
                    let storage = &self.model.arrays[array];
                    if object.real != storage.real || object.width != storage.elem_width {
                        return self
                            .fail(path, "VPI array element type disagrees with its storage");
                    }
                    if object.real {
                        if object.width != 0 {
                            return self.fail(path, "real VPI array elements must use zero width");
                        }
                    } else {
                        self.validate_width(object.width, &format!("{path}.width"))?;
                    }
                }
                IrVpiObjectKind::Net | IrVpiObjectKind::Reg | IrVpiObjectKind::RealVar => {
                    let Some(signal) = object.signal else {
                        return self.fail(path, "VPI value object is missing signal storage");
                    };
                    if signal >= self.model.signals.len() {
                        return self.fail(path, "VPI object references an out-of-bounds IR signal");
                    }
                    if object.array.is_some() {
                        return self.fail(path, "VPI value object cannot reference array storage");
                    }
                    if !object.real {
                        self.validate_width(object.width, &format!("{path}.width"))?;
                    }
                }
            }
        }

        for (idx, call) in self.model.vpi_compile_calls.iter().enumerate() {
            let path = format!("vpi_compile_calls[{idx}]");
            if !call.name.starts_with('$') || call.name.len() < 2 {
                return self.fail(path, "VPI compile-call name must start with `$`");
            }
            if call.args.len() > LLG_MAX_VPI_ARGS {
                return self.fail(path, "VPI compile-call exceeds the argument limit");
            }
            if call.name.contains('\0') {
                return self.fail(path, "VPI compile-call name must not contain NUL bytes");
            }
            for (arg_idx, arg) in call.args.iter().enumerate() {
                if arg.real {
                    if arg.width != 0 {
                        return self.fail(
                            format!("{path}.args[{arg_idx}]"),
                            "real VPI compile-call arguments must have zero width",
                        );
                    }
                } else {
                    self.validate_width(arg.width, &format!("{path}.args[{arg_idx}].width"))?;
                }
            }
        }

        let mut storage_names = HashSet::new();
        for (idx, container) in self.model.containers.iter().enumerate() {
            if !storage_names.insert(container.c_name.as_str()) {
                return self.fail("containers", "duplicate container storage name");
            }
            validate_container_element(&container.element, &format!("containers[{idx}].element"))?;
            if !container.element.is_packed()
                && !matches!(
                    container.kind,
                    IrContainerKind::Dynamic
                        | IrContainerKind::Queue { .. }
                        | IrContainerKind::Associative { .. }
                )
            {
                return self.fail(
                    format!("containers[{idx}].element"),
                    "non-packed elements require a descriptor-backed container",
                );
            }
            if let IrContainerKind::Associative {
                key: IrAssocKey::Integral { width, .. },
            } = container.kind
            {
                self.validate_width(width, &format!("containers[{idx}].key"))?;
            }
            if let Some((class, field)) = container.class_field {
                if self
                    .model
                    .classes
                    .get(class)
                    .and_then(|layout| layout.fields.get(field))
                    .is_none()
                {
                    return self.fail(
                        format!("containers[{idx}].class_field"),
                        "class property is out of bounds",
                    );
                }
            }
            if let Some(receiver) = &container.receiver {
                if container.class_field.is_none() || !receiver.is_plain_receiver() {
                    return self.fail(
                        format!("containers[{idx}].receiver"),
                        "a receiver qualifies only a class property container and must be a plain handle read",
                    );
                }
                let mut plain = receiver;
                while let IrChandleExpr::Required { handle, .. } = plain {
                    plain = handle;
                }
                if let IrChandleExpr::Read(object) = plain {
                    if self.model.objects.get(*object).map(|object| &object.ty)
                        != Some(&IrObjectType::Chandle)
                    {
                        return self.fail(
                            format!("containers[{idx}].receiver"),
                            "receiver object is not a handle",
                        );
                    }
                }
            }
            if let Some(size) = container.initial_size {
                if size == 0 {
                    return self.fail(
                        format!("containers[{idx}].initial_size"),
                        "initial container size must be non-zero",
                    );
                }
                if !matches!(container.kind, IrContainerKind::Dynamic) {
                    return self.fail(
                        format!("containers[{idx}].initial_size"),
                        "only dynamic containers may have an initial size",
                    );
                }
            }
        }
        for object in &self.model.objects {
            if !storage_names.insert(object.c_name.as_str()) {
                return self.fail(
                    "objects",
                    format!("duplicate object storage name `{}`", object.c_name),
                );
            }
            if let Some(initial) = &object.initial {
                if object.ty != IrObjectType::String {
                    return self.fail("objects", "non-string initializer");
                }
                initial.validate(self.model, None)?;
                let mut result = Ok(());
                initial.expressions(&mut |expr| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(expr, &[], "object.initial"));
                });
                result?;
            }
        }
        for (idx, signal) in self.model.signals.iter().enumerate() {
            let path = format!("signals[{idx}]");
            self.validate_type(&signal.ty, &format!("{path}.ty"))?;
            if let Some(value) = &signal.fixed_default {
                self.validate_storage_default(value, signal.ty.width(), signal.ty.signed(), &path)?;
            }
            if !signal.net_alias.is_empty() {
                if !matches!(signal.ty, IrType::Packed { .. }) {
                    return self.fail(
                        format!("{path}.net_alias"),
                        "true-net alias bindings require packed signal storage",
                    );
                }
                if signal.alias.is_some() {
                    return self.fail(
                        format!("{path}.net_alias"),
                        "true-net alias storage cannot also be a variable alias",
                    );
                }
                if signal.net_driver.is_some() {
                    return self.fail(
                        format!("{path}.net_alias"),
                        "true-net alias storage cannot also be an inout driver",
                    );
                }
                let mut signal_bits = HashSet::new();
                for (binding_idx, binding) in signal.net_alias.iter().enumerate() {
                    let binding_path = format!("{path}.net_alias[{binding_idx}]");
                    let group = self.model.net_groups.get(binding.group).ok_or_else(|| {
                        IrValidationError::new(
                            format!("{binding_path}.group"),
                            format!("net-group index {} is out of bounds", binding.group),
                        )
                    })?;
                    if binding.slot >= group.n_drivers {
                        return self.fail(
                            format!("{binding_path}.slot"),
                            format!(
                                "driver slot {} is out of bounds for group {}",
                                binding.slot, binding.group
                            ),
                        );
                    }
                    if binding.group_bit >= group.width {
                        return self.fail(
                            format!("{binding_path}.group_bit"),
                            format!(
                                "group bit {} is out of bounds for group {}",
                                binding.group_bit, binding.group
                            ),
                        );
                    }
                    if binding.signal_bit >= signal.ty.width() {
                        return self.fail(
                            format!("{binding_path}.signal_bit"),
                            format!(
                                "signal bit {} is out of bounds for signal width {}",
                                binding.signal_bit,
                                signal.ty.width()
                            ),
                        );
                    }
                    if !signal_bits.insert(binding.signal_bit) {
                        return self.fail(
                            format!("{binding_path}.signal_bit"),
                            "signal bit has more than one canonical alias binding",
                        );
                    }
                }
            }
            if signal.alias.is_some() && signal.net_driver.is_some() {
                return self.fail(
                    format!("{path}.alias"),
                    "net driver cannot also be a variable alias",
                );
            }
            if let Some((group, slot)) = signal.net_driver {
                let net = self.model.net_groups.get(group).ok_or_else(|| {
                    IrValidationError::new(
                        format!("{path}.net_driver"),
                        format!("net-group index {group} is out of bounds"),
                    )
                })?;
                if slot >= net.n_drivers {
                    return self.fail(
                        format!("{path}.net_driver"),
                        format!("driver slot {slot} is out of bounds for group {group}"),
                    );
                }
                if signal.ty.width() != net.width || signal.ty.signed() != net.signed {
                    return self.fail(
                        format!("{path}.net_driver"),
                        "signal type does not match its net group",
                    );
                }
            } else if let Some(target) = signal.alias {
                let canonical = self.model.signals.get(target).ok_or_else(|| {
                    IrValidationError::new(format!("{path}.alias"), "alias target is out of bounds")
                })?;
                if target == idx
                    || canonical.alias.is_some()
                    || canonical.net_driver.is_some()
                    || canonical.ty != signal.ty
                    || canonical.c_name != signal.c_name
                    || (!signal.omit && canonical.omit)
                {
                    return self.fail(
                        format!("{path}.alias"),
                        "alias must name matching canonical variable storage",
                    );
                }
            } else if !signal.omit && !storage_names.insert(signal.c_name.as_str()) {
                return self.fail(
                    format!("{path}.c_name"),
                    format!("active storage name `{}` is not unique", signal.c_name),
                );
            }
        }

        for (idx, group) in self.model.net_groups.iter().enumerate() {
            let path = format!("net_groups[{idx}]");
            self.validate_width(group.width, &format!("{path}.width"))?;
            if group.n_drivers == 0 {
                return self.fail(format!("{path}.n_drivers"), "net group has no drivers");
            }
            if group.n_drivers > LLG_MAX_NET_DRIVERS {
                return self.fail(
                    format!("{path}.n_drivers"),
                    "net group driver count exceeds the runtime integer range",
                );
            }
            if group.driver_strengths.len() != group.n_drivers {
                return self.fail(
                    format!("{path}.driver_strengths"),
                    "net group strength count does not match its driver count",
                );
            }
            if let Some((strength_idx, _)) = group
                .driver_strengths
                .iter()
                .enumerate()
                .find(|(_, (strength0, strength1))| *strength0 > 7 || *strength1 > 7)
            {
                return self.fail(
                    format!("{path}.driver_strengths[{strength_idx}]"),
                    "net driver strength is outside the IEEE 1800 strength scale",
                );
            }
            if let Some(view) = group.strength_view {
                let valid = self.model.signals.get(view).is_some_and(|signal| {
                    signal.net_driver.is_none()
                        && signal.ty.two_state()
                        && u64::from(signal.ty.width()) == u64::from(group.width) * 8
                });
                if !valid {
                    return self.fail(
                        format!("{path}.strength_view"),
                        "net strength view must be a two-state signal of eight bits per net bit",
                    );
                }
            }
        }

        for (idx, assertion) in self.model.assertions.iter().enumerate() {
            let path = format!("assertions[{idx}]");
            let Some(clock) = self.model.signals.get(assertion.clock_signal) else {
                return self.fail(
                    format!("{path}.clock_signal"),
                    "assertion clock signal index is out of bounds",
                );
            };
            if clock.omit || clock.ty.width() == 0 {
                return self.fail(
                    format!("{path}.clock_signal"),
                    "assertion clock must be an active packed signal",
                );
            }
            if let Some(disable) = assertion.disable_signal {
                let Some(signal) = self.model.signals.get(disable) else {
                    return self.fail(
                        format!("{path}.disable_signal"),
                        "assertion disable signal index is out of bounds",
                    );
                };
                if signal.omit || signal.ty.width() == 0 {
                    return self.fail(
                        format!("{path}.disable_signal"),
                        "assertion disable must be an active packed signal",
                    );
                }
            }
            if let Some(condition) = &assertion.abort_condition {
                self.validate_expr(condition, &[], &format!("{path}.abort_condition"))?;
                if condition.is_real() {
                    return self.fail(
                        format!("{path}.abort_condition"),
                        "assertion abort condition must be packed",
                    );
                }
            } else if assertion.abort_reject || assertion.abort_sync {
                return self.fail(&path, "assertion abort flags require an abort condition");
            }
            if let Some(antecedent) = &assertion.antecedent {
                self.validate_expr(antecedent, &[], &format!("{path}.antecedent"))?;
                if antecedent.is_real() {
                    return self.fail(
                        format!("{path}.antecedent"),
                        "assertion antecedent must be packed",
                    );
                }
            }
            if let Some(consequent) = &assertion.consequent {
                self.validate_expr(consequent, &[], &format!("{path}.consequent"))?;
                if consequent.is_real() {
                    return self.fail(
                        format!("{path}.consequent"),
                        "assertion consequent must be packed",
                    );
                }
            }
            let has_sequence =
                assertion.antecedent_sequence.is_some() || assertion.consequent_sequence.is_some();
            if has_sequence && assertion.consequent_sequence.is_none() {
                return self.fail(&path, "sequence assertion must have a consequent automaton");
            }
            if has_sequence && (assertion.antecedent.is_some() || assertion.consequent.is_some()) {
                return self.fail(
                    &path,
                    "sequence assertion cannot mix direct and automaton expressions",
                );
            }
            if !has_sequence && assertion.consequent.is_none() {
                return self.fail(&path, "direct assertion must have a consequent expression");
            }
            for (name, sequence) in [
                (
                    "antecedent_sequence",
                    assertion.antecedent_sequence.as_ref(),
                ),
                (
                    "consequent_sequence",
                    assertion.consequent_sequence.as_ref(),
                ),
            ] {
                let Some(sequence) = sequence else { continue };
                if sequence.states == 0
                    || sequence.start >= sequence.states
                    || sequence.accept >= sequence.states
                {
                    return self.fail(
                        format!("{path}.{name}"),
                        "sequence automaton has invalid state or transition storage",
                    );
                }
                for (label, clock) in [
                    ("leading_clock", sequence.leading_clock),
                    ("trailing_clock", sequence.trailing_clock),
                ] {
                    if let Some(clock) = clock {
                        if self
                            .model
                            .signals
                            .get(clock)
                            .is_none_or(|signal| signal.omit || signal.ty.width() == 0)
                        {
                            return self.fail(
                                format!("{path}.{name}.{label}"),
                                "sequence clock must be active packed storage",
                            );
                        }
                    }
                }
                if sequence.initializer_slots.len() != sequence.initializers.len()
                    || sequence
                        .initializer_slots
                        .iter()
                        .any(|slot| *slot as usize >= sequence.locals.len())
                {
                    return Err(IrValidationError::new(
                        format!("{path}.{name}.initializer_slots"),
                        "invalid initializer slot",
                    ));
                }
                if !sequence.first_match_states.is_empty() {
                    return self.fail(
                        format!("{path}.{name}.first_match_states"),
                        "first_match requires scoped transitions",
                    );
                }
                let mut declarations = std::collections::HashSet::new();
                for local in &sequence.locals {
                    if local.declaration == 0 || !declarations.insert(local.declaration) {
                        return self.fail(
                            format!("{path}.{name}.locals"),
                            "invalid or duplicate local declaration identity",
                        );
                    }
                }
                let mut entries = std::collections::HashSet::new();
                let mut exits = std::collections::HashSet::new();
                for transition in &sequence.transitions {
                    if transition.enter_scope.is_some() && transition.exit_scope.is_some() {
                        return self.fail(
                            format!("{path}.{name}.scope"),
                            "one edge cannot both enter and exit a scope",
                        );
                    }
                    for (scope, set) in [
                        (transition.enter_scope, &mut entries),
                        (transition.exit_scope, &mut exits),
                    ] {
                        if let Some(scope) = scope {
                            if scope == 0 {
                                return self.fail(
                                    format!("{path}.{name}.scope"),
                                    "zero scope identity is reserved",
                                );
                            }
                            set.insert(scope);
                        }
                    }
                }
                if entries != exits {
                    return self.fail(format!("{path}.{name}.scope"), "unpaired first_match scope");
                }
                crate::sim::ir::assertions::validate_sequence_joins(
                    sequence.states,
                    &sequence.transitions,
                    &sequence.joins,
                )?;
                for (transition_index, transition) in sequence.transitions.iter().enumerate() {
                    if transition.from >= sequence.states || transition.to >= sequence.states {
                        return self.fail(
                            format!("{path}.{name}.transitions[{transition_index}]"),
                            "sequence transition state is out of bounds",
                        );
                    }
                    if transition
                        .delay
                        .max
                        .is_some_and(|max| max < transition.delay.min)
                    {
                        return self.fail(
                            format!("{path}.{name}.transitions[{transition_index}].delay"),
                            "sequence transition delay range is inverted",
                        );
                    }
                    if let Some(clock_signal) = transition.clock_signal {
                        let Some(clock) = self.model.signals.get(clock_signal) else {
                            return self.fail(
                                format!(
                                    "{path}.{name}.transitions[{transition_index}].clock_signal"
                                ),
                                "sequence transition clock signal is out of bounds",
                            );
                        };
                        if clock.omit || clock.ty.width() == 0 {
                            return self.fail(
                                format!(
                                    "{path}.{name}.transitions[{transition_index}].clock_signal"
                                ),
                                "sequence transition clock must be active packed storage",
                            );
                        }
                    }
                    if transition
                        .atom
                        .is_some_and(|atom| atom as usize >= sequence.atoms.len())
                    {
                        return self.fail(
                            format!("{path}.{name}.transitions[{transition_index}].atom"),
                            "sequence transition atom is out of bounds",
                        );
                    }
                    match (transition.match_start, transition.match_count) {
                        (None, 0) => {}
                        (Some(start), count) => {
                            let Some(end) = start.checked_add(count) else {
                                return self.fail(
                                    format!(
                                        "{path}.{name}.transitions[{transition_index}].match_count"
                                    ),
                                    "sequence match-item range overflows",
                                );
                            };
                            if end as usize > sequence.match_items.len() {
                                return self.fail(
                                    format!(
                                        "{path}.{name}.transitions[{transition_index}].match_start"
                                    ),
                                    "sequence match-item range is out of bounds",
                                );
                            }
                        }
                        (None, _) => {
                            return self.fail(
                                format!(
                                    "{path}.{name}.transitions[{transition_index}].match_count"
                                ),
                                "non-empty sequence match-item range has no start",
                            );
                        }
                    }
                }
                for (atom_index, atom) in sequence.atoms.iter().enumerate() {
                    self.validate_expr(atom, &[], &format!("{path}.{name}.atoms[{atom_index}]"))?;
                    if atom.is_real() {
                        return self.fail(
                            format!("{path}.{name}.atoms[{atom_index}]"),
                            "sequence atom must be packed",
                        );
                    }
                }
                for (local_index, local) in sequence.locals.iter().enumerate() {
                    if local.width == 0 {
                        return self.fail(
                            format!("{path}.{name}.locals[{local_index}].width"),
                            "local assertion variable must have a packed width",
                        );
                    }
                }
                let root_domain = (assertion.clock_signal, assertion.posedge);
                let leading_domain = sequence
                    .leading_clock
                    .map(|clock| (clock, sequence.leading_posedge))
                    .unwrap_or(root_domain);
                let mut outgoing = std::collections::HashMap::new();
                for (index, transition) in sequence.transitions.iter().enumerate() {
                    outgoing
                        .entry(transition.from)
                        .or_insert_with(Vec::new)
                        .push((index, transition));
                }
                let mut pending = vec![(sequence.start, leading_domain)];
                let mut reached = std::collections::HashSet::new();
                while let Some((state, from_domain)) = pending.pop() {
                    if !reached.insert((state, from_domain)) {
                        continue;
                    }
                    for (index, transition) in outgoing.get(&state).into_iter().flatten() {
                        let to_domain = transition
                            .clock_signal
                            .map(|clock| (clock, transition.clock_posedge))
                            .unwrap_or(root_domain);
                        if from_domain != to_domain
                            && !matches!(
                                (transition.delay.min, transition.delay.max),
                                (0, Some(0)) | (1, Some(1))
                            )
                        {
                            return self.fail(
                                format!("{path}.{name}.transitions[{index}].delay"),
                                "cross-clock sequence boundaries require an exact ##0 or ##1 delay",
                            );
                        }
                        pending.push((transition.to, to_domain));
                    }
                }
                for (item_index, item) in sequence.match_items.iter().enumerate() {
                    self.validate_expr(
                        item,
                        &[],
                        &format!("{path}.{name}.match_items[{item_index}]"),
                    )?;
                    if item.is_real() {
                        return self.fail(
                            format!("{path}.{name}.match_items[{item_index}]"),
                            "sequence match item must be a packed expression",
                        );
                    }
                }
                for (initializer_index, initializer) in sequence.initializers.iter().enumerate() {
                    self.validate_expr(
                        initializer,
                        &[],
                        &format!("{path}.{name}.initializers[{initializer_index}]"),
                    )?;
                    if initializer.is_real() {
                        return self.fail(
                            format!("{path}.{name}.initializers[{initializer_index}]"),
                            "sequence local initializer must be packed",
                        );
                    }
                }
            }
        }

        for (idx, clock) in self.model.sampled_clocks.iter().enumerate() {
            let path = format!("sampled_clocks[{idx}]");
            if let crate::sim::ir::IrSampledClockKind::Edge { signal, .. } = &clock.kind {
                let Some(signal) = self.model.signals.get(*signal) else {
                    return self.fail(
                        format!("{path}.signal"),
                        "sampled clock signal index is out of bounds",
                    );
                };
                if signal.omit || signal.ty.width() == 0 {
                    return self.fail(
                        format!("{path}.signal"),
                        "sampled clock must be an active packed signal",
                    );
                }
            }
            if let Some(gate) = &clock.gate {
                self.validate_expr(gate, &[], &format!("{path}.gate"))?;
                if gate.is_real() {
                    return self.fail(format!("{path}.gate"), "sampled clock gate must be packed");
                }
            }
        }

        for (idx, domain) in self.model.sampled_domains.iter().enumerate() {
            let path = format!("sampled_domains[{idx}]");
            if domain.clock >= self.model.sampled_clocks.len() {
                return self.fail(
                    format!("{path}.clock"),
                    "sampled domain clock index is out of bounds",
                );
            }
            self.validate_expr(&domain.sample, &[], &format!("{path}.sample"))?;
            if domain.sample.is_real() {
                return self.fail(format!("{path}.sample"), "sampled value must be packed");
            }
        }

        for (idx, array) in self.model.arrays.iter().enumerate() {
            let mut elements = HashSet::new();
            for (element, signal) in &array.net_elements {
                let signal = self.model.signals.get(*signal).ok_or_else(|| {
                    IrValidationError::new(
                        "array.net_elements",
                        "net signal index is out of bounds",
                    )
                })?;
                if array.real
                    || *element >= array.total
                    || !elements.insert(*element)
                    || (signal.net_driver.is_none() && signal.net_alias.is_empty())
                    || signal.ty.width() != array.elem_width
                {
                    return self.fail(
                        "array.net_elements",
                        "invalid resolved array element binding",
                    );
                }
            }
            if let Some(net) = &array.net {
                let mut next = 0u64;
                let mut covered = elements.len() as u64;
                for run in &net.constant_cells {
                    let end = run.first.checked_add(run.count);
                    if array.real
                        || array.two_state
                        || run.count == 0
                        || run.first < next
                        || end.is_none_or(|end| end > array.total)
                    {
                        return self.fail("array.net", "invalid undriven net-cell run");
                    }
                    next = run.first + run.count;
                    covered += run.count;
                }
                let in_run = |element: u64| {
                    let position = net
                        .constant_cells
                        .partition_point(|run| run.first + run.count <= element);
                    net.constant_cells
                        .get(position)
                        .is_some_and(|run| run.first <= element)
                };
                if covered != array.total || elements.iter().any(|element| in_run(*element)) {
                    return self.fail(
                        "array.net",
                        "net-array cells must be published or undriven exactly once",
                    );
                }
            }

            let path = format!("arrays[{idx}]");
            for value in array
                .element_default
                .iter()
                .chain(array.element_uninitialized.iter())
            {
                if value.width != array.elem_width
                    || value.signed != array.signed
                    || value.real.is_some()
                {
                    return self.fail(&path, "array element default has the wrong type");
                }
                self.validate_expr(
                    &IrExpr::new(
                        IrExprKind::Const(value.clone()),
                        array.elem_width,
                        array.signed,
                        None,
                    ),
                    &[],
                    &path,
                )?;
            }

            if array.real {
                if array.elem_width != 0 {
                    return self.fail(
                        format!("{path}.elem_width"),
                        "real array elements must use zero width",
                    );
                }
            } else {
                self.validate_width(array.elem_width, &format!("{path}.elem_width"))?;
            }
            if array.dims.is_empty() {
                return self.fail(format!("{path}.dims"), "array has no dimensions");
            }
            let mut total = 1u64;
            for (dim_idx, (left, right)) in array.dims.iter().copied().enumerate() {
                let extent = (i64::from(left) - i64::from(right))
                    .unsigned_abs()
                    .checked_add(1)
                    .ok_or_else(|| {
                        IrValidationError::new(
                            format!("{path}.dims[{dim_idx}]"),
                            "dimension extent overflows u64",
                        )
                    })?;
                total = total.checked_mul(extent).ok_or_else(|| {
                    IrValidationError::new(
                        format!("{path}.dims[{dim_idx}]"),
                        "dimension product overflows u64",
                    )
                })?;
            }
            if total > LLG_MAX_FIXED_ARRAY_CELLS {
                return self.fail(
                    format!("{path}.total"),
                    format!(
                        "fixed-array cell count {total} exceeds selected cell-wise storage limit {LLG_MAX_FIXED_ARRAY_CELLS}"
                    ),
                );
            }
            if total != array.total {
                return self.fail(
                    format!("{path}.total"),
                    format!(
                        "stored total {} does not match dimension product {total}",
                        array.total
                    ),
                );
            }
        }

        self.validate_native_tables()?;
        for (index, access) in self.model.native_accesses.iter().enumerate() {
            let path = format!("native_accesses[{index}]");
            if access.name != format!("_llg_access_{index}") {
                return self.fail(
                    &path,
                    "native access identity does not match its table index",
                );
            }
            if !access.item_path.is_empty()
                && !matches!(
                    access.kind,
                    IrNativeAccessKind::ValueItem { .. } | IrNativeAccessKind::ElementItem { .. }
                )
            {
                return self.fail(&path, "only native value items carry an item path");
            }
            if matches!(access.receiver, IrChandleExpr::ContainerElement { .. })
                != matches!(access.kind, IrNativeAccessKind::ElementItem { .. })
            {
                return self.fail(
                    &path,
                    "container element locators address element items only",
                );
            }
            match access.kind {
                IrNativeAccessKind::ClassField { class, field } => {
                    if !self
                        .model
                        .classes
                        .get(class)
                        .is_some_and(|class| field < class.fields.len())
                    {
                        return self.fail(&path, "class member reference is out of bounds");
                    }
                }
                IrNativeAccessKind::InterfaceMember { interface, member } => {
                    if !self
                        .model
                        .virtual_interfaces
                        .get(interface)
                        .is_some_and(|interface| member < interface.members.len())
                    {
                        return self.fail(&path, "interface member reference is out of bounds");
                    }
                }
                IrNativeAccessKind::ElementItem { ty } => {
                    self.validate_element_leaf(&access.receiver, &access.item_path, ty, &path)?;
                }
                IrNativeAccessKind::ValueItem { value, ty } => {
                    self.validate_native_leaf(value, &access.item_path, ty, &path)?;
                    if access.receiver != IrChandleExpr::Null {
                        return self.fail(&path, "native value item has no receiver");
                    }
                }
            }
            let function = access
                .function
                .map(|index| {
                    self.model.funcs.get(index).ok_or_else(|| {
                        IrValidationError::new(&path, "function reference is out of bounds")
                    })
                })
                .transpose()?;
            let formals = function.map_or(&[][..], |function| function.formals.as_slice());
            self.chandle_return
                .set(function.map(|function| function.ret_chandle));
            self.string_return
                .set(function.map(|function| function.ret_string));
            access
                .receiver
                .validate(self.model, formals, self.chandle_return.get())?;
            let mut result = Ok(());
            access.receiver.expressions(&mut |expr| {
                result = result
                    .clone()
                    .and_then(|_| self.validate_expr(expr, formals, &path));
            });
            result?;
        }
        for (index, allocation) in self.model.class_allocations.iter().enumerate() {
            let path = format!("class_allocations[{index}]");
            if allocation.class >= self.model.classes.len() || allocation.local.is_empty() {
                return self.fail(&path, "invalid class allocation identity");
            }
            let function = allocation
                .function
                .map(|index| {
                    self.model.funcs.get(index).ok_or_else(|| {
                        IrValidationError::new(&path, "function reference is out of bounds")
                    })
                })
                .transpose()?;
            let formals = function.map_or(&[][..], |function| function.formals.as_slice());
            self.chandle_return
                .set(function.map(|function| function.ret_chandle));
            self.string_return
                .set(function.map(|function| function.ret_string));
            self.validate_stmts(&allocation.body, formals, &format!("{path}.body"))?;
        }

        for (idx, func) in self.model.funcs.iter().enumerate() {
            self.function.set(Some(func));
            self.chandle_return.set(Some(func.ret_chandle));
            self.string_return.set(Some(func.ret_string));
            let path = format!("funcs[{idx}]");
            if (func.ret_chandle || func.ret_string) && func.ret.is_some()
                || func.ret_chandle && func.ret_string
            {
                return self.fail(&path, "function has incompatible return types");
            }
            if let Some(value) = &func.return_default {
                let ty = func.ret.as_ref().ok_or_else(|| {
                    IrValidationError::new(&path, "void function has a default value")
                })?;
                self.validate_storage_default(value, ty.width(), ty.signed(), &path)?;
            }
            if let Some(signal_index) = func.return_signal {
                let signal = self.model.signals.get(signal_index).ok_or_else(|| {
                    IrValidationError::new(
                        format!("{path}.return_signal"),
                        "return signal is out of bounds",
                    )
                })?;
                let Some(return_type) = func.ret else {
                    return self.fail(
                        format!("{path}.return_signal"),
                        "function without a numeric return cannot own a return signal",
                    );
                };
                if func.automatic || func.ret_chandle || func.ret_string {
                    return self.fail(
                        format!("{path}.return_signal"),
                        "return signal requires a static numeric function",
                    );
                }
                if signal.ty != return_type || signal.net_driver.is_some() || signal.alias.is_some()
                {
                    return self.fail(
                        format!("{path}.return_signal"),
                        "return signal must be unaliased storage with the function return type",
                    );
                }
            }
            if let Some(ret) = &func.ret {
                self.validate_type(ret, &format!("{path}.ret"))?;
            }
            for (formal_idx, formal) in func.formals.iter().enumerate() {
                if let Some(array) = formal.fixed_array {
                    if self
                        .model
                        .arrays
                        .get(array)
                        .is_none_or(|array| !array.sparse())
                        || formal.width != 0
                        || formal.real
                        || formal.string
                        || formal.chandle
                        || formal.fixed_shape.is_some()
                        || formal.fixed_default.is_some()
                    {
                        return self.fail(&path, "invalid descriptor formal storage");
                    }
                }
                if let Some(array) = formal.real_array {
                    if self.model.arrays.get(array).is_none_or(|array| !array.real)
                        || formal.width != 0
                        || formal.real
                        || formal.string
                        || formal.chandle
                        || formal.fixed_array.is_some()
                        || formal.native_value.is_some()
                        || formal.fixed_shape.is_some()
                        || formal.fixed_default.is_some()
                    {
                        return self.fail(&path, "invalid real-array formal storage");
                    }
                }
                if let Some(container) = formal.container {
                    // A `ref` container formal aliases caller storage, so it
                    // is an activation binding (SIM-008).
                    if self
                        .model
                        .containers
                        .get(container)
                        .is_none_or(|storage| formal.is_ref() && !storage.activation)
                        || formal.width != 0
                        || formal.real
                        || formal.string
                        || formal.chandle
                        || formal.event
                        || formal.fixed_array.is_some()
                        || formal.native_value.is_some()
                        || formal.real_array.is_some()
                        || formal.fixed_shape.is_some()
                        || formal.fixed_default.is_some()
                    {
                        return self.fail(&path, "invalid container formal storage");
                    }
                }
                if let Some(value) = &formal.fixed_default {
                    self.validate_storage_default(value, formal.width, formal.signed, &path)?;
                }
                if let Some(shape) = &formal.fixed_shape {
                    validate_container_element(
                        shape,
                        &format!("{path}.formals[{formal_idx}].fixed_shape"),
                    )?;
                    if shape.fixed_packed_width() != Some(formal.width) {
                        return self
                            .fail(&path, "fixed formal shape disagrees with its payload width");
                    }
                }
                if formal.const_ref && !formal.is_ref() {
                    return self.fail(
                        format!("{path}.formals[{formal_idx}].const_ref"),
                        "const qualification requires ref mode",
                    );
                }
                if formal.ref_static && !formal.is_ref() {
                    return self.fail(
                        format!("{path}.formals[{formal_idx}].ref_static"),
                        "ref static qualification requires ref mode",
                    );
                }
                if formal.fixed_array.is_none()
                    && formal.native_value.is_none()
                    && formal.container.is_none()
                    && formal.real_array.is_none()
                    && !formal.chandle
                    && !formal.event
                    && !formal.real
                    && !formal.string
                {
                    self.validate_width(
                        formal.width,
                        &format!("{path}.formals[{formal_idx}].width"),
                    )?;
                } else if formal.real && formal.width != 0 {
                    return self.fail(
                        format!("{path}.formals[{formal_idx}].width"),
                        "real formal must use zero width",
                    );
                }
            }
            for (local_idx, local) in func.locals.iter().enumerate() {
                let local_path = format!("{path}.locals[{local_idx}]");
                if let Some(value) = &local.fixed_default {
                    self.validate_storage_default(value, local.width, local.signed, &local_path)?;
                }
                if !local.real && !local.string {
                    self.validate_width(local.width, &format!("{local_path}.width"))?;
                } else if local.width != 0 {
                    return self.fail(
                        format!("{local_path}.width"),
                        "real local must use zero width",
                    );
                }
                if let Some(initial) = &local.initial {
                    self.validate_expr(initial, &func.formals, &format!("{local_path}.initial"))?;
                    if initial.is_real() != local.real
                        || initial.width != local.width
                        || initial.signed != local.signed
                    {
                        return self.fail(
                            format!("{local_path}.initial"),
                            "local initializer type disagrees with its declaration",
                        );
                    }
                }
            }
            self.validate_pre_fns(&func.pre_fns, &func.formals, &path)?;
            self.validate_stmts(&func.body, &func.formals, &format!("{path}.body"))?;
        }

        self.chandle_return.set(None);
        self.string_return.set(None);
        self.function.set(None);
        for (idx, process) in self.model.processes.iter().enumerate() {
            let path = format!("processes[{idx}]");
            for (write_idx, write) in process.writes.iter().enumerate() {
                if !self.valid_dependency(write) {
                    return self.fail(
                        format!("{path}.writes[{write_idx}]"),
                        "process write must name active storage",
                    );
                }
            }
            self.validate_pre_fns(&process.pre_fns, &[], &path)?;
            self.validate_stmts(&process.body, &[], &format!("{path}.body"))?;
        }

        for (idx, step) in self.model.init_steps.iter().enumerate() {
            self.validate_init_step(step, &format!("init_steps[{idx}]"))?;
        }

        self.validate_spawns(&self.model.spawns, "spawns")?;
        self.validate_spawns(&self.model.final_spawns, "final_spawns")?;
        let normal: HashSet<&str> = self.model.spawns.iter().map(String::as_str).collect();
        if let Some(name) = self
            .model
            .final_spawns
            .iter()
            .find(|name| normal.contains(name.as_str()))
        {
            return self.fail(
                "final_spawns",
                format!("process `{name}` is registered as both normal and final"),
            );
        }
        Ok(())
    }
}
