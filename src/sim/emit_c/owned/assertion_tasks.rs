//! Assertion control arguments and deferred actions retain explicit owners.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn assertion_control(
        &mut self,
        kind: IrAssertionControlKind,
        args: &[IrExpr],
        scopes: &[String],
    ) -> Result<(), String> {
        let kind = match kind {
            IrAssertionControlKind::On => "LLG_ASSERTION_CONTROL_ON",
            IrAssertionControlKind::Off => "LLG_ASSERTION_CONTROL_OFF",
            IrAssertionControlKind::Kill => "LLG_ASSERTION_CONTROL_KILL",
            IrAssertionControlKind::Control => "LLG_ASSERTION_CONTROL_FULL",
        };
        let mut values = Vec::new();
        for arg in args {
            let value = self.expression(arg)?;
            if value.width == 0 {
                return Err("assertion control arguments must be integral".to_owned());
            }
            values.push(value);
        }
        let array = if values.is_empty() {
            "NULL".to_owned()
        } else {
            let name = self.name("assertion_args");
            // This is a borrowed descriptor view. The registered slots remain
            // sole owners until the control operation returns or exits.
            self.line(format!(
                "sv4_t {name}[] = {{ {} }};",
                values
                    .iter()
                    .map(|v| v.code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            name
        };
        let scopes_ptr = if scopes.is_empty() {
            "NULL".to_owned()
        } else {
            let name = self.name("assertion_scopes");
            self.line(format!(
                "const char* const {name}[] = {{ {} }};",
                scopes
                    .iter()
                    .map(|s| c_string_literal(s))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            name
        };
        self.line(format!(
            "(void)llg_assertion_control({kind}, {array}, {}, {scopes_ptr}, {});",
            values.len(),
            scopes.len()
        ));
        for value in values {
            self.discard(value);
        }
        Ok(())
    }

    pub(super) fn deferred_assertion(&mut self, stmt: &IrStmt) -> Result<(), String> {
        let IrStmt::DeferredImmediateAssertion {
            kind,
            condition,
            if_true,
            if_false,
            label,
            location,
            scope,
            identity,
        } = stmt
        else {
            return Err("expected a deferred assertion".to_owned());
        };
        let kind = match kind {
            IrImmediateAssertionKind::Assert => "LLG_ASSERTION_ASSERT",
            IrImmediateAssertionKind::Assume => "LLG_ASSERTION_ASSUME",
            IrImmediateAssertionKind::Cover => "LLG_ASSERTION_COVER",
        };
        let (label, location, scope) = (
            c_string_literal(label),
            c_string_literal(location),
            c_string_literal(scope),
        );
        self.line(format!(
            "if (llg_deferred_assertion_enabled({kind}, {label}, {scope})) {{"
        ));
        let condition = self.condition(condition)?;
        self.line(format!("if ({condition}) {{"));
        for (passed, action) in [(true, if_true.as_ref()), (false, if_false.as_ref())] {
            if !passed {
                self.line("} else {");
            }
            let (callback, frame) = if let Some(action) = action {
                let values = self.prepare_captures(
                    action.captures().iter().map(|c| (c.storage(), c.initial())),
                )?;
                let frame = self.name("deferred_frame");
                self.publish_captures(&frame, values);
                (action.c_name(), frame)
            } else {
                ("NULL", "NULL".to_owned())
            };
            // Enqueue consumes the new frame reference, even if coalesced or
            // rejected. Do not release it again in the issuing process.
            self.line(format!("llg_deferred_assertion_scoped({kind}, {}, {identity}ULL, {label}, {location}, {scope}, {callback}, {frame});", u8::from(passed)));
        }
        self.line("} }");
        Ok(())
    }
}
