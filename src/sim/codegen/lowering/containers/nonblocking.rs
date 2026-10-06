//! Nonblocking writes to fixed-array views of string, record and handle
//! elements (SIM-007). The source elements and the destination position are
//! captured when the assignment executes; the runtime commits them in the
//! NBA region (SV 10.4.2). Automatic arrays are not legal NBA targets.

use super::*;

impl Codegen<'_> {
    /// `a <= src`, `a[i] <= v` or `a[l:r] <= src` where `a` is a persistent
    /// fixed-array view; `None` when `lhs` is not such a target.
    pub(in super::super) fn lower_fixed_view_nba(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let zero = || pattern_key_expr(0, 64, true, false);
        // Whole array or slice target.
        let target = match self.fixed_view_slice(path, lhs)? {
            Some((target, start, count)) => Some((target, Some(start), count)),
            None => self
                .container_of(self.p30_unwrap_cast(lhs))
                .map(|container| container.ir)
                .filter(|container| self.fixed_view_ranges.contains_key(container))
                .and_then(|container| {
                    let count = self.model.containers[container].initial_size?;
                    Some((container, None, count))
                }),
        };
        if let Some((target, dst_start, count)) = target {
            self.require_persistent_view(path, target)?;
            let mut statements = Vec::new();
            let (src, src_start) = match self.fixed_view_slice(path, rhs)? {
                Some((src, start, _)) => (src, start),
                None => match self.container_of(self.p30_unwrap_cast(rhs)) {
                    Some(src) => (src.ir, zero()),
                    None => {
                        let temporary = self.container_temporary_like(target);
                        self.model.containers[temporary].initial_size = Some(count);
                        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                            temporary,
                        ))));
                        statements.push(self.lower_container_into(path, lhs, temporary, rhs)?);
                        (temporary, zero())
                    }
                },
            };
            // A whole-array source replaces the target; a slice source or
            // target writes `count` positions.
            let dst_start = match dst_start {
                None if self.fixed_view_slice(path, rhs)?.is_some() => Some(zero()),
                other => other,
            };
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::Nonblocking {
                target,
                dst_start,
                src,
                src_start,
                count,
            })));
            return Ok(Some(IrStmt::Block(statements)));
        }
        // One element.
        let Some((target, indices)) = self.container_element_path(lhs) else {
            return Ok(None);
        };
        if indices.len() != 1 || !self.fixed_view_ranges.contains_key(&target) {
            return Ok(None);
        }
        self.require_persistent_view(path, target)?;
        let temporary = self.container_temporary_like(target);
        self.model.containers[temporary].initial_size = None;
        let declare = IrStmt::Container(Box::new(IrContainerStmt::Declare(temporary)));
        let value = self.lower_container_source_values(path, temporary, vec![rhs])?;
        let index = self.lower_container_top_index(path, target, indices[0])?;
        let width = index.width.max(32) + 1;
        let index = IrExpr::convert_to(index, width, true);
        Ok(Some(IrStmt::Block(vec![
            declare,
            value,
            IrStmt::Container(Box::new(IrContainerStmt::Nonblocking {
                target,
                dst_start: Some(index),
                src: temporary,
                src_start: zero(),
                count: 1,
            })),
        ])))
    }

    fn require_persistent_view(&self, path: &str, target: usize) -> Result<(), String> {
        if self.model.containers[target].is_global_storage() {
            return Ok(());
        }
        Err(format!(
            "nonblocking assignment to an automatic array in `{path}` is illegal"
        ))
    }
}
