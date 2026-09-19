//! Runtime tasks which borrow packed operands or consume a published literal path.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn descriptor(&mut self, expression: &IrExpr) -> Result<String, String> {
        let value = self.expression(expression)?;
        let descriptor = self.scalar("uint32_t", format!("llg_file_descriptor({})", value.code));
        self.discard(value);
        Ok(descriptor)
    }

    pub(super) fn file_control(
        &mut self,
        operation: IrFileOp,
        expression: Option<&IrExpr>,
    ) -> Result<(), String> {
        let descriptor = expression.map(|value| self.descriptor(value)).transpose()?;
        self.line(match (operation, descriptor) {
            (IrFileOp::Close, Some(value)) => format!("llg_file_close({value});"),
            (IrFileOp::Rewind, Some(value)) => format!("llg_file_rewind({value});"),
            (IrFileOp::Flush, Some(value)) => format!("llg_file_flush({value}, 0);"),
            (IrFileOp::Flush, None) => "llg_file_flush(0, 1);".to_owned(),
            _ => return Err("file control task is missing its descriptor".to_owned()),
        });
        Ok(())
    }

    pub(super) fn memory_task(&mut self, statement: &IrStmt) -> Result<(), String> {
        let IrStmt::Memory {
            write,
            path,
            array,
            radix,
            start,
            finish,
        } = statement
        else {
            return Err("expected a memory task".to_owned());
        };
        let path = self.string(path)?;
        let array = self.ctx.model.array(*array);
        if array.real || array.dims.len() != 1 {
            return Err("memory task requires a one-dimensional packed array".to_owned());
        }
        let start_value = start
            .as_ref()
            .map(|expression| self.expression(expression))
            .transpose()?;
        let finish_value = finish
            .as_ref()
            .map(|expression| self.expression(expression))
            .transpose()?;
        let first = start_value
            .as_ref()
            .map(|value| value.code.as_str())
            .unwrap_or("(sv4_t)SV4_EMPTY");
        let last = finish_value
            .as_ref()
            .map(|value| value.code.as_str())
            .unwrap_or("(sv4_t)SV4_EMPTY");
        let (left, right) = array.dims[0];
        let runtime = if *write {
            "llg_memory_write"
        } else {
            "llg_memory_read"
        };
        let radix = match radix {
            IrMemoryRadix::Binary => 2,
            IrMemoryRadix::Hex => 16,
        };
        self.line(format!("{runtime}({}, {}, {}ULL, {}u, {}, {}, (const int32_t[]){{ {left}, {right} }}, 1, {first}, {last}, {}, {}, {radix});",
            path.take_string(), array.c_name, array.total, array.elem_width, u8::from(array.signed),
            u8::from(array.two_state), u8::from(start.is_some()), u8::from(finish.is_some())));
        self.native_discard(path);
        if let Some(value) = start_value {
            self.discard(value);
        }
        if let Some(value) = finish_value {
            self.discard(value);
        }
        Ok(())
    }

    pub(super) fn monitor_task(&mut self, statement: &IrStmt) -> Result<(), String> {
        let IrStmt::MonitorSet {
            strobe,
            fmt,
            eval,
            n_args,
            reads,
            scope,
            descriptor,
            ..
        } = statement
        else {
            return Err("expected a monitor task".to_owned());
        };
        let descriptor = descriptor
            .as_ref()
            .map(|value| self.descriptor(value))
            .transpose()?;
        let scope = c_string_literal(scope);
        let prefix = if descriptor.is_some() {
            "llg_file_"
        } else {
            "llg_"
        };
        let descriptor = descriptor
            .map(|value| format!("{value}, "))
            .unwrap_or_default();
        if *strobe {
            self.line(format!(
                "{prefix}strobe_typed({descriptor}{fmt}, {n_args}, {eval}, {scope});"
            ));
        } else {
            let mut pointers = Vec::new();
            for read in reads {
                let read = match read {
                    IrDependency::PackedRange { storage, .. } => storage.as_ref(),
                    other => other,
                };
                // The monitor keeps this address after its registering scope.
                // Only model/static storage is eligible, never an automatic.
                let (kind, address) = match read {
                    IrDependency::Scalar(name) | IrDependency::Real(name) => {
                        let binding = self
                            .lookup(name)
                            .ok_or_else(|| format!("unknown monitor dependency {name}"))?;
                        if binding.automatic {
                            return Err(pending("automatic monitor dependencies"));
                        }
                        (
                            if binding.width == 0 {
                                "LLG_FMT_REAL"
                            } else {
                                "LLG_FMT_PACKED"
                            },
                            binding.address,
                        )
                    }
                    IrDependency::ArrayContents(index) => (
                        "LLG_FMT_PACKED",
                        format!("&{}_llg_contents_dep", self.ctx.model.array(*index).c_name),
                    ),
                    IrDependency::ArrayElement { array, index } => {
                        let array = self.ctx.model.array(*array);
                        if array.real {
                            ("LLG_FMT_REAL", format!("&{}[{index}]", array.c_name))
                        } else {
                            (
                                "LLG_FMT_PACKED",
                                format!("&{}_llg_element_deps[{index}]", array.c_name),
                            )
                        }
                    }
                    IrDependency::ContainerContents(index) => (
                        "LLG_FMT_PACKED",
                        format!(
                            "&{}_llg_contents_dep",
                            self.ctx.model.containers[*index].c_name
                        ),
                    ),
                    IrDependency::ContainerShape(index) => (
                        "LLG_FMT_PACKED",
                        format!(
                            "&{}_llg_shape_dep",
                            self.ctx.model.containers[*index].c_name
                        ),
                    ),
                    IrDependency::Object(index)
                        if self.ctx.model.objects[*index].ty == IrObjectType::String =>
                    {
                        (
                            "LLG_FMT_PACKED",
                            format!("&{}_llg_dep", self.ctx.model.objects[*index].c_name),
                        )
                    }
                    _ => return Err(pending("this monitor dependency")),
                };
                pointers.push(format!("{{ {kind}, {address} }}"));
            }
            let pointer = if pointers.is_empty() {
                "NULL".to_owned()
            } else {
                let name = self.name("monitor_reads");
                self.line(format!(
                    "llg_display_read_t {name}[] = {{ {} }};",
                    pointers.join(", ")
                ));
                name
            };
            self.line(format!("{prefix}monitor_with_typed_reads({descriptor}{fmt}, {n_args}, {eval}, {scope}, {pointer}, {});", reads.len()));
        }
        Ok(())
    }

    pub(super) fn pca_task(&mut self, statement: &IrStmt) -> Result<(), String> {
        let (sig, enable, site, expression, operation) = match statement {
            IrStmt::PcaAssign {
                sig,
                enable,
                site,
                value,
            } => (*sig, *enable, *site, value, "assign"),
            IrStmt::PcaDrive {
                sig,
                enable,
                site,
                value,
            } => (*sig, *enable, *site, value, "drive"),
            _ => return Err("expected a procedural continuous assignment".to_owned()),
        };
        let signal = self.ctx.model.signal(sig);
        let enable = &self.ctx.model.signal(enable).c_name;
        let value = self.expression(expression)?;
        let value = self.convert(
            value,
            signal.ty.width(),
            signal.ty.signed(),
            signal.ty.two_state(),
            matches!(signal.ty, IrType::Real { shortreal: true }),
        );
        let suffix = if signal.ty.width() == 0 { "_d" } else { "" };
        self.line(format!(
            "llg_pca_{operation}{suffix}(&{}, &{enable}, {site}ULL, {});",
            signal.c_name, value.code
        ));
        self.discard(value);
        Ok(())
    }
}
