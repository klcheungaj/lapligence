//! Names.

use super::*;

impl<'a> Codegen<'a> {

    /// `lib@`-stripped name of a signal, with its scope path when available
    /// (`"tb.bus"`, `"tb.u0.bus"`).
    pub(super) fn display_name(&self, id: NodeId) -> String {
        let node = self.node(id);
        match node.parent {
            Some(p) => {
                if self.is_runtime_environment(p) {
                    let namespace = if self.is_compilation_unit(p) {
                        "$unit".to_string()
                    } else {
                        strip_lib(&self.node(p).name)
                    };
                    return format!("{namespace}::{}", node.name);
                }
                let scope = self.db.instance_path(p);
                if scope.is_empty() {
                    strip_lib(&node.name)
                } else {
                    format!("{}.{}", scope, node.name)
                }
            }
            None => strip_lib(&node.name),
        }
    }

    /// Full HDL hierarchy for waveform metadata, with ASCII unit-separator
    /// bytes between components.  The separator is not legal inside a source
    /// identifier, unlike `.`, so an escaped identifier such as `\a.b` cannot
    /// be mistaken for two scopes by the C waveform runtime.  Generate-scope
    /// spelling is retained verbatim (`g[0]`, not `g_0_`).
    pub(in super::super) fn waveform_name_for(&self, id: NodeId) -> String {
        self.waveform_name(id)
    }

    pub(super) fn waveform_name(&self, id: NodeId) -> String {
        const SEPARATOR: &str = "\u{1f}";

        let root_name = match self.kind(id) {
            NodeKind::ModuleInst { is_top: true, .. } => strip_lib(&self.node(id).name),
            _ => self.node(id).name.clone(),
        };
        let mut parts = vec![root_name];
        let mut current = self.node(id).parent;
        while let Some(scope_id) = current {
            let scope = self.node(scope_id);
            if matches!(
                scope.kind,
                NodeKind::ModuleInst { .. } | NodeKind::GenScopeArray | NodeKind::GenScope
            ) {
                // A frontend can library-qualify top design units (`work@tb`).
                // Other name components are source identifiers, where
                // `@` is legal in an escaped spelling and must be preserved.
                let name = match &scope.kind {
                    NodeKind::ModuleInst { is_top: true, .. } => strip_lib(&scope.name),
                    _ => scope.name.clone(),
                };
                if !name.is_empty() {
                    parts.push(name);
                }
            } else if self.is_runtime_environment(scope_id) {
                let name = if self.is_compilation_unit(scope_id) {
                    "$unit"
                } else {
                    scope.name.as_str()
                };
                if !name.is_empty() {
                    parts.push(name.to_owned());
                }
            }
            current = scope.parent;
        }
        parts.reverse();
        parts.join(SEPARATOR)
    }

    /// Resolve a `$dumpvars` argument to the owned HDL identity used by the
    /// waveform catalog.  This deliberately consumes semantic targets rather
    /// than reconstructing a path from a generated C name.
    pub(in super::super) fn waveform_selection_name(&self, node: NodeId) -> Result<String, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::ScopeRef { target }) => self.waveform_selection_name(*target),
            NodeKind::Expr(ExprKind::Ref { target }) => target
                .map(|target| self.waveform_selection_name(target))
                .unwrap_or_else(|| {
                    Err(format!(
                        "`$dumpvars` reference `{}` has no resolved target",
                        self.node(node).name
                    ))
                }),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .next()
                .copied()
                .map(|target| self.waveform_selection_name(target))
                .unwrap_or_else(|| {
                    Err(format!(
                        "`$dumpvars` hierarchy `{}` has no resolved target",
                        self.node(node).name
                    ))
                }),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.waveform_selection_name(*operand)
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let target = self.waveform_array_target(*base).ok_or_else(|| {
                    format!(
                        "cannot resolve `$dumpvars` array selection `{}`",
                        self.node(node).name
                    )
                })?;
                let info = self.array_of(*base).ok_or_else(|| {
                    format!(
                        "array `{}` is not represented in the waveform catalog",
                        self.node(target).name
                    )
                })?;
                if indices.len() != info.dims.len() {
                    return Err(format!(
                        "`$dumpvars` array selection `{}` requires {} declared indices",
                        self.node(target).name,
                        info.dims.len()
                    ));
                }
                let mut name = self.waveform_name(target);
                for (dimension, index) in indices.iter().enumerate() {
                    let value = self.eval_bound_i128(*index).map_err(|error| {
                        format!(
                            "`$dumpvars` array index for `{}` must be a resolved constant: {error}",
                            self.node(target).name
                        )
                    })?;
                    let (left, right) = info.dims.get(dimension).copied().ok_or_else(|| {
                        format!(
                            "internal error while resolving `$dumpvars` array `{}`",
                            self.node(target).name
                        )
                    })?;
                    let low = i128::from(left.min(right));
                    let high = i128::from(left.max(right));
                    if value < low || value > high {
                        return Err(format!(
                            "`$dumpvars` array index {value} for `{}` is outside declared bounds [{left}:{right}]",
                            self.node(target).name
                        ));
                    }
                    name.push('[');
                    name.push_str(&value.to_string());
                    name.push(']');
                }
                Ok(name)
            }
            NodeKind::ModuleInst { .. }
            | NodeKind::GenScopeArray
            | NodeKind::GenScope
            | NodeKind::Port { .. }
            | NodeKind::Net { .. }
            | NodeKind::Var { .. }
            | NodeKind::Array { .. } => Ok(self.waveform_name(node)),
            _ => Err(format!(
                "`$dumpvars` argument `{}` is not a scope or dumpable storage reference",
                self.node(node).name
            )),
        }
    }

    fn waveform_array_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Array { .. } => Some(node),
            NodeKind::Expr(ExprKind::Ref { target }) => {
                target.and_then(|target| self.waveform_array_target(target))
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .copied()
                .find_map(|target| self.waveform_array_target(target)),
            _ => None,
        }
    }
}
