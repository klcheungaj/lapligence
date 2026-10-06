//! Reference descriptors borrow stable variables and retain movable queue
//! cells. Descriptor graphs live in registered scopes, never in C stack
//! storage that a stackless suspension would invalidate.
use super::stores::Selection;
use super::*;

impl Frame<'_, '_> {
    pub(super) fn reference_address(&self, address: &str) -> Result<String, String> {
        // Forwarded references are canonical formal descriptors (rN). There
        // are no executable, string-keyed reference bindings in this frame.
        if let Some(function) = self.ctx.func {
            for (index, formal) in function.formals.iter().enumerate() {
                if formal.is_ref()
                    && !formal.string
                    && !formal.chandle
                    && address == format!("r{index}")
                {
                    return Ok(address.to_owned());
                }
            }
        }
        Err(pending(&format!("unbound typed reference {address}")))
    }

    /// Whether the real reference formal named `address` is a shortreal.
    pub(super) fn real_reference_is_short(&self, address: &str) -> Result<bool, String> {
        self.ctx
            .func
            .and_then(|function| {
                function
                    .formals
                    .iter()
                    .enumerate()
                    .find(|(index, formal)| {
                        formal.is_ref() && formal.real && address == format!("r{index}")
                    })
                    .map(|(_, formal)| formal.shortreal)
            })
            .ok_or_else(|| pending(&format!("unbound real reference {address}")))
    }

    /// A `double*` naming the actual's real storage cell for a real `ref`
    /// formal. An invalid element index binds a call-owned scratch cell that
    /// reads the default 0.0 and absorbs writes, so the callee never
    /// dereferences null and never writes outside the array.
    pub(super) fn real_reference_argument(&mut self, lhs: &IrLhs) -> Result<String, String> {
        if let IrLhs::Ref {
            addr, bit: None, ..
        } = lhs
        {
            return self.reference_address(addr);
        }
        let target = self.target(lhs)?;
        if target.width != 0 || target.selection.is_some() || target.net.is_some() {
            return Err("real reference argument requires real variable storage".to_owned());
        }
        let pointer = if target.valid == "1" {
            self.scalar("double*", target.binding.address.clone())
        } else {
            let scope = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(double), NULL)".to_owned(),
            );
            let scratch = self.scalar(
                "double*",
                format!("(double*)llg_value_scope_object({scope})"),
            );
            self.line(format!("*{scratch} = 0.0;"));
            self.scalar(
                "double*",
                format!(
                    "({}) ? {} : {scratch}",
                    target.valid, target.binding.address
                ),
            )
        };
        self.release_target(target);
        Ok(pointer)
    }

    pub(super) fn reference_argument(
        &mut self,
        lhs: &IrLhs,
        read: &IrExpr,
        width: u32,
        signed: bool,
        two_state: bool,
    ) -> Result<String, String> {
        // Call markers own these descriptors until invocation/copyout finishes.
        self.reference_argument_with_scopes(lhs, read, width, signed, two_state, &mut Vec::new())
    }

    pub(super) fn reference_argument_with_scopes(
        &mut self,
        lhs: &IrLhs,
        read: &IrExpr,
        width: u32,
        signed: bool,
        two_state: bool,
        scopes: &mut Vec<String>,
    ) -> Result<String, String> {
        if let IrLhs::Stream {
            parts,
            width: total,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        } = lhs
        {
            if *total != width || parts.is_empty() {
                return Err("composite reference shape mismatch".into());
            }
            let count = parts.len();
            let allocation = self.scalar(
                "llg_value_scope_t*",
                format!("llg_value_scope_begin_object(sizeof(llg_ref_t*) * {count}, NULL)"),
            );
            scopes.push(allocation.clone());
            let table = self.scalar(
                "llg_ref_t**",
                format!("(llg_ref_t**)llg_value_scope_object({allocation})"),
            );
            for (index, (part, part_width)) in parts.iter().enumerate() {
                let pointer = self.reference_argument_with_scopes(
                    part,
                    read,
                    *part_width,
                    false,
                    false,
                    scopes,
                )?;
                self.line(format!("{table}[{index}] = {pointer};"));
            }
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_composite_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let composite = self.scalar(
                "llg_ref_composite_t*",
                format!("(llg_ref_composite_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!(
                "*{composite} = (llg_ref_composite_t){{ .count = {count}, .parts = {table} }};"
            ));
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let pointer = self.scalar(
                "llg_ref_t*",
                format!("(llg_ref_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!("*{pointer} = (llg_ref_t){{ .kind = LLG_REF_COMPOSITE, .width = {width}, .is_signed = {}, .two_state = {}, .retained = {composite} }};", u8::from(signed), u8::from(two_state)));
            return Ok(pointer);
        }
        if let IrLhs::TaggedSelect {
            target,
            steps,
            location,
            ..
        } = lhs
        {
            let root_width = match target.as_ref() {
                IrLhs::Whole(index) => self.ctx.model.signals[*index].ty.width(),
                IrLhs::WholeRef { width, .. }
                | IrLhs::Ref { width, .. }
                | IrLhs::Stream { width, .. } => *width,
                IrLhs::ArrayElem { arr, .. } => self.ctx.model.arrays[*arr].elem_width,
                _ => return Err("tagged reference view requires a whole storage root".into()),
            };
            let parent = self
                .reference_argument_with_scopes(target, read, root_width, false, false, scopes)?;
            let mut selectors = Vec::with_capacity(steps.len());
            for step in steps {
                if step.selection.base.is_real() {
                    return Err("tagged reference selector must be integral".into());
                }
                selectors.push(self.expression(&step.selection.base)?);
            }
            let plan = self.scalar(
                "sv4_select_plan_t",
                format!("sv4_select_plan_init({root_width})"),
            );
            let check_count = steps.iter().filter(|step| step.guard.is_some()).count();
            let checks = if check_count == 0 {
                "NULL".to_owned()
            } else {
                let allocation = self.scalar(
                    "llg_value_scope_t*",
                    format!(
                        "llg_value_scope_begin_object(sizeof(llg_ref_tag_check_t) * {check_count}, NULL)"
                    ),
                );
                scopes.push(allocation.clone());
                self.scalar(
                    "llg_ref_tag_check_t*",
                    format!("(llg_ref_tag_check_t*)llg_value_scope_object({allocation})"),
                )
            };
            let mut check_index = 0;
            for (index, step) in steps.iter().enumerate() {
                if let Some(guard) = &step.guard {
                    self.line(format!(
                        "{checks}[{check_index}] = (llg_ref_tag_check_t){{ .receiver_plan = {plan}, .tag_width = {}, .member_index = {}, .member_name = {} }};",
                        guard.tag_width,
                        guard.member_index,
                        c_string_literal(&guard.member_name)
                    ));
                    check_index += 1;
                }
                self.line(format!(
                    "sv4_select_plan_step(&{plan}, {}, {});",
                    selectors[index].code, step.selection.width
                ));
            }
            for selector in selectors {
                self.discard(selector);
            }
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_view_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let view = self.scalar(
                "llg_ref_view_t*",
                format!("(llg_ref_view_t*)llg_value_scope_object({allocation})"),
            );
            // Const-ref is enforced by the generated formal contract; the
            // shared runtime view representation also serves writable refs.
            self.line(format!(
                "*{view} = (llg_ref_view_t){{ .parent = (llg_ref_t*){parent}, .plan = {plan}, .tag_check_count = {check_count}, .tag_checks = {checks}, .location = {} }};",
                c_string_literal(location)
            ));
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let pointer = self.scalar(
                "llg_ref_t*",
                format!("(llg_ref_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!(
                "*{pointer} = (llg_ref_t){{ .kind = LLG_REF_TAGGED_VIEW, .width = {width}, .is_signed = {}, .two_state = {}, .retained = {view} }};",
                u8::from(signed),
                u8::from(two_state)
            ));
            return Ok(pointer);
        }
        // A member or packed selection of a flattened array element (for
        // example `records[i].field`) is a view below that element. The
        // element selectors are evaluated once, when the parent binds.
        if let IrLhs::ArrayElem {
            arr,
            indices,
            elem_sel: IrElemSel::PackedChain(steps),
        } = lhs
        {
            let view = IrLhs::PackedSelect {
                target: Box::new(IrLhs::ArrayElem {
                    arr: *arr,
                    indices: indices.clone(),
                    elem_sel: IrElemSel::Whole,
                }),
                steps: steps.clone(),
                signed,
                two_state,
            };
            return self
                .reference_argument_with_scopes(&view, read, width, signed, two_state, scopes);
        }
        if let IrLhs::PackedSelect { target, steps, .. } = lhs {
            let root_width = match target.as_ref() {
                IrLhs::Whole(index) => self.ctx.model.signals[*index].ty.width(),
                IrLhs::WholeRef { width, .. }
                | IrLhs::Ref { width, .. }
                | IrLhs::Stream { width, .. } => *width,
                IrLhs::ArrayElem { arr, .. } => self.ctx.model.arrays[*arr].elem_width,
                _ => return Err("reference view requires a whole storage root".into()),
            };
            let parent = self
                .reference_argument_with_scopes(target, read, root_width, false, false, scopes)?;
            let Some(Selection::PackedChain(plan, _)) =
                self.selection(&IrElemSel::PackedChain(steps.clone()), root_width)?
            else {
                return Err("reference view has no selection plan".into());
            };
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_view_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let view = self.scalar(
                "llg_ref_view_t*",
                format!("(llg_ref_view_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!(
                "*{view} = (llg_ref_view_t){{ .parent = (llg_ref_t*){parent}, .plan = {plan}, .tag_check_count = 0, .tag_checks = NULL, .location = NULL }};"
            ));
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let pointer = self.scalar(
                "llg_ref_t*",
                format!("(llg_ref_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!("*{pointer} = (llg_ref_t){{ .kind = LLG_REF_VIEW, .width = {width}, .is_signed = {}, .two_state = {}, .retained = {view} }};", u8::from(signed), u8::from(two_state)));
            return Ok(pointer);
        }
        if let IrExprKind::Container(expression) = read.kind() {
            if let IrContainerExpr::Get { container, index } = expression.as_ref() {
                let container_name = self.container_name(*container)?;
                let mut container = self.ctx.model.containers[*container].clone();
                container.c_name = container_name;
                if matches!(container.kind, IrContainerKind::Queue { .. })
                    && container.element.is_packed()
                {
                    let index = self.expression(index)?;
                    let pointer = self.scalar(
                        "llg_ref_t*",
                        format!(
                            "llg_ref_queue(&{}, sv4_to_index({}))",
                            container.c_name, index.code
                        ),
                    );
                    self.discard(index);
                    return Ok(pointer);
                }
                // Dynamic-array elements and integral-keyed entries bind a
                // retained element cell (SV 13.5.2).
                let binder = match container.kind {
                    IrContainerKind::Dynamic => Some("llg_ref_dyn"),
                    IrContainerKind::Associative { .. } => Some("llg_ref_assoc_integral"),
                    IrContainerKind::Queue { .. } => None,
                };
                if let Some(binder) = binder.filter(|_| container.element.is_packed()) {
                    let index = self.expression(index)?;
                    let pointer = self.scalar(
                        "llg_ref_t*",
                        format!("{binder}(&{}, {})", container.c_name, index.code),
                    );
                    self.discard(index);
                    return Ok(pointer);
                }
            }
            if let IrContainerExpr::GetString { container, key } = expression.as_ref() {
                let container_name = self.container_name(*container)?;
                let key = self.string(key)?;
                let code = key.code();
                let pointer = self.scalar(
                    "llg_ref_t*",
                    format!("llg_ref_assoc_string(&{container_name}, ({code}).data, ({code}).len)"),
                );
                self.native_discard(key);
                return Ok(pointer);
            }
        }
        if let IrLhs::Ref {
            addr, bit: None, ..
        } = lhs
        {
            return self.reference_address(addr);
        }
        let mut target = self.target(lhs)?;
        scopes.append(&mut target.reference_scopes);
        if target.width != width || width == 0 || target.net.is_some() {
            return Err("reference argument requires matching packed variable storage".to_owned());
        }
        let two_state = two_state || target.binding.two_state;
        let selection = match &target.selection {
            None => ".kind = LLG_REF_WHOLE".to_owned(),
            Some(Selection::PackedChain(..)) => {
                return Err("a packed selection is not a legal ref actual".to_owned());
            }
            Some(Selection::Bit(index)) => format!(".kind = LLG_REF_BIT, .index = {index}"),
            Some(Selection::Part(left, right)) => format!(".kind = LLG_REF_PART, .left = {left}LL, .right = {right}LL"),
            Some(Selection::Indexed(base, width, negative)) => format!(".kind = LLG_REF_INDEXED, .index = sv4_to_index({}), .indexed_width = {width}, .indexed_negative = {}", base.code, u8::from(*negative)),
        };
        // The call's C activation survives coroutine suspension. A process/root
        // value scope owns the descriptor so early cancellation never escapes
        // with a pointer to an expired inner C block.
        let scope = self.scalar(
            "llg_value_scope_t*",
            "llg_value_scope_begin_object(sizeof(llg_ref_t), NULL)".to_owned(),
        );
        scopes.push(scope.clone());
        let pointer = self.scalar(
            "llg_ref_t*",
            format!("(llg_ref_t*)llg_value_scope_object({scope})"),
        );
        self.line(format!("*{pointer} = (llg_ref_t){{ .base = ({} ? {} : NULL), .width = {width}, .is_signed = {}, .two_state = {}, {selection} }};",
            target.valid, target.binding.address, u8::from(signed), u8::from(two_state)));
        self.release_target(target);
        Ok(pointer)
    }
}
