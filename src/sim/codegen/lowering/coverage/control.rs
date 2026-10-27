//! Assertion coverage items and the coverage access functions of IEEE
//! 1800-2009 40.3.2 (`$coverage_control`, `$coverage_get_max`,
//! `$coverage_get`, `$coverage_merge`, `$coverage_save`).
//!
//! The item table lists every concurrent assertion statement instance
//! (`expect` and sequence end points excluded) and every immediate or
//! deferred assertion statement lowered; the instance table lists the
//! elaborated instances that a scope argument selects. Both are dropped by
//! generation unless a coverage access function or the VPI catalog can read
//! them.
use super::*;
use crate::sim::ir::{
    IrAssertionCoverage, IrAssertionItem, IrAssertionItemForm, IrAssertionItemKind,
    IrConcurrentAssertionKind, IrCoverageApiCall, IrCoverageApiFunction, IrCoverageInstance,
    IrCoverageScope,
};

/// An immediate or deferred assertion statement recorded while lowering.
pub(in crate::sim::codegen::lowering) struct ImmediateAssertionItem {
    kind: IrAssertionItemKind,
    form: IrAssertionItemForm,
    label: String,
    scope: String,
}

/// The `$coverage_*` instance hierarchy in depth-first order, and the
/// index of each instance node.
pub(in crate::sim::codegen::lowering) struct CoverageInstances {
    table: Vec<IrCoverageInstance>,
    index: HashMap<NodeId, u32>,
}

/// The hierarchical name of a labelled assertion. The display scope of a
/// labelled statement already ends with its label.
fn full_name(scope: &str, label: &str) -> String {
    if label.is_empty() {
        String::new()
    } else if scope == label || scope.ends_with(&format!(".{label}")) {
        scope.to_owned()
    } else if scope.is_empty() {
        label.to_owned()
    } else {
        format!("{scope}.{label}")
    }
}

impl Codegen<'_> {
    /// Record an immediate (`deferred` false) or deferred assertion
    /// statement as an assertion coverage item.
    pub(in crate::sim::codegen::lowering) fn record_assertion_item(
        &mut self,
        statement: NodeId,
        kind: IrImmediateAssertionKind,
        deferred: bool,
        label: &str,
        scope: String,
    ) {
        let kind = match kind {
            IrImmediateAssertionKind::Assert => IrAssertionItemKind::Assert,
            IrImmediateAssertionKind::Assume => IrAssertionItemKind::Assume,
            IrImmediateAssertionKind::Cover => IrAssertionItemKind::Cover,
        };
        let form = if deferred {
            IrAssertionItemForm::Deferred
        } else {
            IrAssertionItemForm::Immediate
        };
        self.immediate_assertion_items
            .entry(statement.index() as u64)
            .or_insert_with(|| ImmediateAssertionItem {
                kind,
                form,
                label: label.to_owned(),
                scope,
            });
    }

    /// The instance hierarchy: every module, interface and program
    /// instance below the top instances, parents first.
    fn coverage_instances(&mut self) -> &CoverageInstances {
        if self.coverage_instances.is_none() {
            let mut instances = CoverageInstances {
                table: Vec::new(),
                index: HashMap::new(),
            };
            for node in self.design_nodes() {
                let NodeKind::ModuleInst { def_name, .. } = self.kind(node) else {
                    continue;
                };
                let parent = self
                    .owning_inst(node)
                    .and_then(|parent| instances.index.get(&parent).copied());
                let index = instances.table.len() as u32;
                instances.table.push(IrCoverageInstance {
                    path: self.instance_path_of(node),
                    definition: strip_lib(def_name),
                    parent,
                });
                instances.index.insert(node, index);
            }
            self.coverage_instances = Some(instances);
        }
        self.coverage_instances
            .as_ref()
            .expect("the coverage instance table was just built")
    }

    /// The instance index of the statement `identity`'s owning instance.
    fn coverage_item_instance(&mut self, identity: u64) -> Option<u32> {
        let node = NodeId::from_index(usize::try_from(identity).ok()?);
        if node.index() >= self.db.node_count() {
            return None;
        }
        let owner = self.owning_inst(node)?;
        self.coverage_instances().index.get(&owner).copied()
    }

    /// Build the assertion coverage tables of `model` from its concurrent
    /// assertions and the immediate assertions lowered.
    pub(in crate::sim::codegen::lowering) fn populate_assertion_coverage(
        &mut self,
        model: &mut IrModel,
    ) {
        let mut items = Vec::new();
        for assertion in &model.assertions {
            if assertion.endpoint.is_some() {
                continue;
            }
            let (kind, form) = match assertion.kind {
                IrConcurrentAssertionKind::Assert => {
                    (IrAssertionItemKind::Assert, IrAssertionItemForm::Concurrent)
                }
                IrConcurrentAssertionKind::Assume => {
                    (IrAssertionItemKind::Assume, IrAssertionItemForm::Concurrent)
                }
                IrConcurrentAssertionKind::Cover => {
                    (IrAssertionItemKind::Cover, IrAssertionItemForm::Concurrent)
                }
                IrConcurrentAssertionKind::CoverSequence => (
                    IrAssertionItemKind::Cover,
                    IrAssertionItemForm::CoverSequence,
                ),
                // `expect` is a procedural statement, not an assertion
                // item (S46-D9).
                IrConcurrentAssertionKind::Expect => continue,
            };
            items.push(IrAssertionItem {
                identity: assertion.identity,
                kind,
                form,
                name: assertion.label.clone(),
                full_name: full_name(&assertion.scope, &assertion.label),
                instance: None,
                file: None,
                line: 0,
                column: 0,
            });
        }
        for (identity, item) in std::mem::take(&mut self.immediate_assertion_items) {
            items.push(IrAssertionItem {
                identity,
                kind: item.kind,
                form: item.form,
                full_name: full_name(&item.scope, &item.label),
                name: item.label,
                instance: None,
                file: None,
                line: 0,
                column: 0,
            });
        }
        items.sort_by_key(|item| item.identity);
        items.dedup_by_key(|item| item.identity);
        for item in &mut items {
            item.instance = self.coverage_item_instance(item.identity);
            if let Some(node) = usize::try_from(item.identity)
                .ok()
                .filter(|index| *index < self.db.node_count())
                .map(NodeId::from_index)
            {
                let node = self.node(node);
                item.file = node.file.as_deref().map(str::to_owned);
                item.line = node.line;
                item.column = node.col;
            }
        }
        let instances = self.coverage_instances().table.clone();
        model.assertion_coverage = Some(IrAssertionCoverage { items, instances });
    }

    /// `$coverage_control`, `$coverage_get_max`, `$coverage_get`,
    /// `$coverage_merge` or `$coverage_save` (40.3.2).
    pub(in crate::sim::codegen::lowering) fn lower_coverage_api(
        &mut self,
        path: &str,
        name: &str,
        call: NodeId,
    ) -> Result<IrExpr, String> {
        let function = match name {
            "$coverage_control" => IrCoverageApiFunction::Control,
            "$coverage_get_max" => IrCoverageApiFunction::GetMax,
            "$coverage_get" => IrCoverageApiFunction::Get,
            "$coverage_merge" => IrCoverageApiFunction::Merge,
            _ => IrCoverageApiFunction::Save,
        };
        let args = self.node(call).children.clone();
        let integral = function.integral_arguments();
        if args.len() != integral + 1 {
            return Err(self.coverage_error(
                call,
                format!(
                    "{name} takes {} arguments (IEEE 1800-2009 40.3.2)",
                    integral + 1
                ),
            ));
        }
        let mut lowered = Vec::with_capacity(integral);
        for arg in &args[..integral] {
            let value = self.lower_expr(path, *arg)?;
            if value.is_real() {
                return Err(self.coverage_error(
                    *arg,
                    format!("the integral arguments of {name} must not be real"),
                ));
            }
            lowered.push(value);
        }
        let last = args[integral];
        let (scope, text) = match function {
            IrCoverageApiFunction::Merge | IrCoverageApiFunction::Save => {
                (None, Some(self.lower_string(path, last)?))
            }
            _ => (Some(self.coverage_scope_argument(path, name, last)?), None),
        };
        self.coverage_api_used = true;
        let site = self.source_site(name, path, call);
        let site = site
            .strip_prefix(&format!("{name} at "))
            .unwrap_or(&site)
            .to_owned();
        Ok(IrExpr::new(
            IrExprKind::SysFunc(Box::new(IrSysFunc::CoverageApi(Box::new(
                IrCoverageApiCall {
                    function,
                    args: lowered,
                    scope,
                    name: text,
                    site,
                },
            )))),
            32,
            true,
            None,
        ))
    }

    /// The `modules_or_instance` argument: a hierarchical reference to an
    /// instance, `$root`, or a string naming a module definition (Table
    /// 40-2).
    fn coverage_scope_argument(
        &mut self,
        path: &str,
        name: &str,
        argument: NodeId,
    ) -> Result<IrCoverageScope, String> {
        let target = match self.kind(argument) {
            NodeKind::Expr(ExprKind::ScopeRef { target }) => Some(*target),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if matches!(self.kind(*target), NodeKind::ModuleInst { .. }) => Some(*target),
            _ => None,
        };
        let Some(target) = target else {
            return Ok(IrCoverageScope::Definition(
                self.lower_string(path, argument)
                    .map_err(|_| self.coverage_scope_error(name, argument))?,
            ));
        };
        if self.db.semantic_detail(target) == Some("Root") {
            return Ok(IrCoverageScope::Root);
        }
        match self.coverage_instances().index.get(&target).copied() {
            Some(index) => Ok(IrCoverageScope::Instance(index)),
            None => Err(self.coverage_scope_error(name, argument)),
        }
    }

    fn coverage_scope_error(&self, name: &str, argument: NodeId) -> String {
        self.coverage_error(
            argument,
            format!(
                "the last argument of {name} must name a module instance, `$root` or a \
                 module definition string (IEEE 1800-2009 40.3.2.1)"
            ),
        )
    }
}
