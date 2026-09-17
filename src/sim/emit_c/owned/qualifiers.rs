//! Qualified branching evaluates diagnostic candidates once, then executes only
//! the first selected body. Priority keeps its short-circuit evaluation order.
use super::*;

impl Frame<'_, '_> {
    fn qualifier_diagnostic(&mut self, check: &IrUniquePriorityCheck, count: &str, fallback: bool) {
        let location = match check.origin() {
            Some(crate::sim::semantic::Origin::Source { path, line, column, .. }) => format!("{path}:{line}:{column}"),
            Some(crate::sim::semantic::Origin::Synthetic { reason }) => format!("<synthetic: {reason}>"),
            None => "<unknown>".to_owned(),
        };
        if let Some(kind) = check.kind_code() {
            self.line(format!("llg_unique_priority_check({kind}, {count}, {}, {});", u8::from(fallback), c_string_literal(&location)));
        }
    }

    pub(super) fn qualified_if(&mut self, condition: &IrExpr, then_: &[IrStmt],
        els: Option<&[IrStmt]>, check: &IrUniquePriorityCheck) -> Result<(), String> {
        let mut branches = vec![(condition, then_)];
        let mut fallback = els;
        while let Some([IrStmt::If { cond, then_, els, check }]) = fallback {
            if !check.is_none() { break; }
            branches.push((cond, then_.as_slice()));
            fallback = els.as_deref();
        }
        let selected = self.scalar("int", "-1".to_owned());
        let count = self.scalar("int", "0".to_owned());
        for (i, (cond, _)) in branches.iter().enumerate() {
            if check.is_priority() { self.line(format!("if ({selected} < 0) {{")); }
            let hit = self.condition(cond)?;
            self.line(format!("if ({hit}) {{ ++{count}; if ({selected} < 0) {selected} = {i}; }}"));
            if check.is_priority() { self.line("}"); }
        }
        self.qualifier_diagnostic(check, &count, fallback.is_some());
        for (i, (_, body)) in branches.iter().enumerate() {
            self.line(format!("{}if ({selected} == {i})", if i == 0 { "" } else { "else " }));
            self.block(body)?;
        }
        if let Some(body) = fallback { self.line("else"); self.block(body)?; }
        Ok(())
    }

    pub(super) fn qualified_case(&mut self, sel: &IrExpr, kind: IrCaseKind,
        items: &[IrCaseItem], check: &IrUniquePriorityCheck) -> Result<(), String> {
        let selector = self.expression(sel)?;
        let selected = self.scalar("int", "-1".to_owned());
        let count = self.scalar("int", "0".to_owned());
        let mut branches = Vec::new();
        for (index, item) in items.iter().enumerate().filter(|(_, item)| !item.exprs.is_empty()) {
            branches.push((index, &item.body));
            if check.is_priority() { self.line(format!("if ({selected} < 0) {{")); }
            let hit = self.scalar("int", "0".to_owned());
            for expression in &item.exprs {
                // A comma-separated group is one candidate, not one candidate
                // per matching expression. Later groups are still diagnosed.
                self.line(format!("if (!{hit}) {{"));
                let value = self.expression(expression)?;
                if kind == IrCaseKind::Inside {
                    self.line(format!("{hit} = {};", value.truth())); self.discard(value);
                } else if selector.width == 0 || value.width == 0 {
                    self.line(format!("{hit} = ({} == {});", selector.real(), value.real())); self.discard(value);
                } else {
                    let code = format!("{}({}, {})", kind.cmp_fn(), selector.code, value.code);
                    let result = self.replace(value, code, 1, false);
                    self.line(format!("{hit} = {};", result.truth())); self.discard(result);
                }
                self.line("}");
            }
            self.line(format!("if ({hit}) {{ ++{count}; if ({selected} < 0) {selected} = {index}; }}"));
            if check.is_priority() { self.line("}"); }
        }
        self.discard(selector);
        let fallback = items.iter().find(|item| item.exprs.is_empty());
        self.qualifier_diagnostic(check, &count, fallback.is_some());
        for (i, (index, body)) in branches.iter().enumerate() {
            self.line(format!("{}if ({selected} == {index})", if i == 0 { "" } else { "else " }));
            self.block(body)?;
        }
        if let Some(item) = fallback {
            if !branches.is_empty() { self.line("else"); }
            self.block(&item.body)?;
        }
        Ok(())
    }
}
