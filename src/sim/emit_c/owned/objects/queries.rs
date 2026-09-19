//! Array and enum queries evaluate their runtime operands once into owners.
use super::*;

impl Frame<'_, '_> {
    fn query_integer(&mut self, number: i128, width: u32, signed: bool) -> Value {
        let count = width.div_ceil(64) as usize;
        let mut bits = vec![if number < 0 { u64::MAX } else { 0 }; count];
        if count > 0 {
            bits[0] = number as u64;
        }
        if count > 1 {
            bits[1] = (number >> 64) as u64;
        }
        if !width.is_multiple_of(64) {
            bits[count - 1] &= (1u64 << (width % 64)) - 1;
        }
        let constant = IrConst {
            bits,
            x: Vec::new(),
            z: Vec::new(),
            width,
            signed,
            fill: None,
            real: None,
        };
        self.value(emit_const(&constant), width, signed)
    }

    pub(in crate::sim::emit_c::owned) fn array_query(
        &mut self,
        query: &IrArrayQuery,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        let dimensions = match &query.target {
            IrArrayQueryTarget::Static { dimensions }
            | IrArrayQueryTarget::Container { dimensions, .. }
            | IrArrayQueryTarget::String { dimensions, .. } => dimensions,
        };
        if dimensions.is_empty() {
            return Err("array query requires dimensions".to_owned());
        }
        let selector = if let Some(index) = &query.dimension {
            let value = self.expression(index)?;
            let index = self.name("query_dimension");
            self.line(format!("int64_t {index} = 0;"));
            self.line(format!(
                "if (!sv4_to_index_i64({}, &{index})) {index} = 0;",
                value.code
            ));
            self.discard(value);
            index
        } else {
            "1".to_owned()
        };
        let result = self.value(
            format!(
                "sv4_x({}, {})",
                expression.width,
                u8::from(expression.signed)
            ),
            expression.width,
            expression.signed,
        );
        for (index, dimension) in dimensions.iter().enumerate() {
            self.line(format!("if ({selector} == {}) {{", index + 1));
            let value = if let (Some(left), Some(right)) = (dimension.left, dimension.right) {
                let number = match query.kind {
                    IrArrayQueryKind::Left => left,
                    IrArrayQueryKind::Right => right,
                    IrArrayQueryKind::Low => left.min(right),
                    IrArrayQueryKind::High => left.max(right),
                    IrArrayQueryKind::Increment => {
                        if left >= right {
                            1
                        } else {
                            -1
                        }
                    }
                    IrArrayQueryKind::Size => left
                        .checked_sub(right)
                        .and_then(|n| n.checked_abs())
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(|| "array query extent overflows".to_owned())?,
                };
                self.query_integer(number, expression.width, expression.signed)
            } else {
                self.dynamic_array_query(query, expression)?
            };
            let value = self.convert(value, expression.width, expression.signed, false, false);
            self.line(format!("sv4_move(&{}, &{});", result.code, value.code));
            self.discard(value);
            self.line("}");
        }
        Ok(result)
    }

    fn dynamic_array_query(
        &mut self,
        query: &IrArrayQuery,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        use IrArrayQueryKind::*;
        let width = expression.width;
        let signed = expression.signed;
        if query.kind == Increment {
            return Ok(self.query_integer(-1, width, signed));
        }
        match &query.target {
            IrArrayQueryTarget::Container { container, .. } => {
                let container = self.ctx.model.containers[*container].clone();
                let name = &container.c_name;
                let generic = if container.element.is_packed() {
                    ""
                } else {
                    "_value"
                };
                let prefix = match container.kind {
                    IrContainerKind::Dynamic => "llg_dyn",
                    IrContainerKind::Queue { .. } => "llg_queue",
                    _ => "llg_assoc",
                };
                if matches!(container.kind, IrContainerKind::Associative { .. }) {
                    if !matches!(
                        container.kind,
                        IrContainerKind::Associative {
                            key: IrAssocKey::Integral { .. }
                        }
                    ) {
                        return Err(
                            "array query requires a typed integral associative index".to_owned()
                        );
                    }
                    return Ok(match query.kind {
                        Left => self.query_integer(0, width, signed),
                        Right => self.value(
                            format!("sv4_fill(1, {width}, {})", u8::from(signed)),
                            width,
                            signed,
                        ),
                        Size => self.value(
                            format!(
                                "sv4_from_u64({prefix}{generic}_count(&{name}), {width}, {})",
                                u8::from(signed)
                            ),
                            width,
                            signed,
                        ),
                        Low | High => {
                            let key = self.query_integer(0, width, signed);
                            let function = if query.kind == Low { "first" } else { "last" };
                            self.line(format!("if (!{prefix}{generic}_{function}_integral(&{name}, &{})) sv4_replace(&{}, sv4_x({width}, {}));", key.code, key.code, u8::from(signed)));
                            key
                        }
                        Increment => unreachable!("handled above"),
                    });
                }
                if matches!(query.kind, Left | Low) {
                    return Ok(self.query_integer(0, width, signed));
                }
                let size = self.value(
                    format!(
                        "sv4_from_u64({prefix}{generic}_size(&{name}), {width}, {})",
                        u8::from(signed)
                    ),
                    width,
                    signed,
                );
                if query.kind == Size {
                    return Ok(size);
                }
                let one = self.query_integer(1, width, signed);
                let result = self.value(
                    format!("sv4_sub({}, {})", size.code, one.code),
                    width,
                    signed,
                );
                self.discard(size);
                self.discard(one);
                Ok(result)
            }
            IrArrayQueryTarget::String { value, .. } => {
                if matches!(query.kind, Left | Low) {
                    return Ok(self.query_integer(0, width, signed));
                }
                let text = self.string(value)?;
                let size = self.value(format!("llg_string_len({})", text.take_string()), 32, true);
                self.native_discard(text);
                if query.kind == Size {
                    return Ok(size);
                }
                let one = self.query_integer(1, 32, true);
                let result = self.value(format!("sv4_sub({}, {})", size.code, one.code), 32, true);
                self.discard(size);
                self.discard(one);
                Ok(result)
            }
            IrArrayQueryTarget::Static { .. } => {
                Err("fixed array query has a missing bound".to_owned())
            }
        }
    }

    pub(in crate::sim::emit_c::owned) fn enum_query(
        &mut self,
        query: &IrEnumQuery,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        match query.method {
            IrEnumMethod::First | IrEnumMethod::Last => {
                let member = if query.method == IrEnumMethod::First {
                    query.members.first()
                } else {
                    query.members.last()
                }
                .ok_or_else(|| "enum has no members".to_owned())?;
                self.expression(&member.value)
            }
            IrEnumMethod::Num => Ok(self.query_integer(
                query.members.len() as i128,
                expression.width,
                expression.signed,
            )),
            IrEnumMethod::Next | IrEnumMethod::Prev => {
                let receiver = self.expression(
                    query
                        .receiver
                        .as_deref()
                        .ok_or_else(|| "enum navigation has no receiver".to_owned())?,
                )?;
                let step = if let Some(step) = &query.step {
                    self.expression(step)?
                } else {
                    self.query_integer(1, 32, false)
                };
                let mut members = Vec::new();
                for member in &query.members {
                    members.push(self.expression(&member.value)?);
                }
                let default = self.expression(&query.default)?;
                let list = if members.is_empty() {
                    "NULL".to_owned()
                } else {
                    format!(
                        "(const sv4_t[]){{ {} }}",
                        members
                            .iter()
                            .map(|v| v.code.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let result = self.value(
                    format!(
                        "sv4_enum_navigate({}, {}, {list}, {}, {}, {})",
                        receiver.code,
                        step.code,
                        members.len(),
                        default.code,
                        if query.method == IrEnumMethod::Next {
                            1
                        } else {
                            -1
                        }
                    ),
                    expression.width,
                    expression.signed,
                );
                self.discard(receiver);
                self.discard(step);
                self.discard(default);
                for member in members {
                    self.discard(member);
                }
                Ok(result)
            }
        }
    }

    pub(in crate::sim::emit_c::owned) fn dynamic_cast(
        &mut self,
        cast: &IrDynamicCast,
    ) -> Result<Value, String> {
        if self.read_only_callback {
            return Err(pending("casts in read-only callbacks"));
        }
        if let (Some(address), Some(source), Some(expected)) =
            (&cast.class_target, &cast.class_source, cast.class_expected)
        {
            let target = self.native_address(address, super::super::native::NativeKind::Chandle)?;
            let source = self.chandle(source)?;
            let success = self.scalar("int", format!("llg_class_is_a({source}, {expected})"));
            self.line(format!("if ({success}) *({}) = {source};", target.address));
            return Ok(self.value(format!("sv4_from_u64({success}, 1, 0)"), 1, false));
        }
        if cast.class_target.is_some() || cast.class_source.is_some() {
            return Err("incomplete class cast metadata".to_owned());
        }
        let target = self.target(&cast.lhs)?;
        let source = self.expression(&cast.rhs)?;
        let success = self.scalar(
            "int",
            if cast.valid_values.is_empty() {
                "1"
            } else {
                "0"
            }
            .to_owned(),
        );
        for member in &cast.valid_values {
            let member = self.expression(member)?;
            let compare = self.value(
                format!("sv4_case_eq({}, {})", source.code, member.code),
                1,
                false,
            );
            self.line(format!("{success} |= {};", compare.truth()));
            self.discard(compare);
            self.discard(member);
        }
        let source = self.convert(
            source,
            cast.target_width,
            cast.target_signed,
            cast.target_two_state,
            cast.target_shortreal,
        );
        self.line(format!("if ({success}) {{"));
        let copy = if source.width == 0 {
            self.value(source.code.clone(), 0, source.signed)
        } else {
            self.value(
                format!("sv4_clone(&{})", source.code),
                source.width,
                source.signed,
            )
        };
        self.store(&target, copy, false, "0")?;
        self.line("}");
        self.discard(source);
        self.release_target(target);
        Ok(self.value(format!("sv4_from_u64({success}, 1, 0)"), 1, false))
    }
}
