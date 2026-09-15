//! Budget.

use super::*;

/// Consume one shared response-budget slot for each serialized item in a
/// declaration/content array.  Arrays do not have a separate marker shape,
/// so the containing record is marked and the single global budget marker
/// stops all subsequent traversal when the prefix is exhausted.
pub(super) fn collect_budgeted<I, U, F>(
    items: I,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
    map: F,
) -> Vec<U>
where
    I: IntoIterator,
    F: Fn(I::Item) -> U,
{
    let mut collected = Vec::new();
    for item in items {
        if !budget.take_regular() {
            *truncated = true;
            budget.take_budget_marker();
            break;
        }
        collected.push(map(item));
    }
    collected
}

pub(super) fn take_module_record(
    root_id: &str,
    modules: &mut Vec<ExplorerModule>,
    budget: &mut InstanceBudget,
    more_definitions: bool,
) -> bool {
    if budget.take_module(more_definitions) {
        return true;
    }
    if more_definitions && budget.take_module_marker() {
        modules.push(budget_module_marker(root_id));
        return false;
    }
    // A quota of one deliberately spends its only slot on a real definition,
    // leaving no room for a catalog marker. Surface a later omitted definition
    // on the deterministic real prefix instead of silently dropping it.
    if let Some(last) = modules.last_mut() {
        last.is_budget_truncated = true;
    }
    false
}

fn budget_module_marker(root_id: &str) -> ExplorerModule {
    ExplorerModule {
        id: format!("module:{root_id}:<budget-truncated>"),
        name: "<budget-truncated>".to_owned(),
        uri: None,
        range: None,
        content_source: Some("budget-truncated".to_owned()),
        is_budget_truncated: true,
        ports: Vec::new(),
        params: Vec::new(),
        signals: Vec::new(),
    }
}

pub(super) fn compatibility_module(
    root_id: &str,
    module: &ModuleDef,
    representative: Option<&InstanceModel>,
    budget: &mut InstanceBudget,
) -> ExplorerModule {
    let mut is_budget_truncated = false;
    let (content_source, ports, params, signals) = if let Some(instance) = representative {
        (
            Some("elaborated".to_owned()),
            collect_budgeted(
                instance.ports.iter(),
                budget,
                &mut is_budget_truncated,
                port,
            ),
            collect_budgeted(
                instance.params.iter(),
                budget,
                &mut is_budget_truncated,
                parameter,
            ),
            instance_signals(instance, budget, &mut is_budget_truncated),
        )
    } else {
        (None, Vec::new(), Vec::new(), Vec::new())
    };
    ExplorerModule {
        id: module_id(root_id, module),
        name: clean_name(&module.name).to_owned(),
        uri: source_uri(module.file.as_deref()),
        range: source_range(module.line, module.col, module.end_line, module.end_col),
        content_source,
        is_budget_truncated,
        ports,
        params,
        signals,
    }
}

pub(super) const MAX_INSTANCE_NODES: usize = 10_000;
/// Keep one hierarchy slot out of the ordinary catalog budget. Additional
/// roots consume ordinary slots when available, but this floor lets the first
/// root survive a catalog/content truncation in a shared response.
pub(super) const GUARANTEED_HIERARCHY_ROOT_SLOTS: usize = 1;
/// The complete workspace response can contain several analysis roots. Keep
/// a small bounded pool for their first hierarchy records without taking a
/// material share of the module catalog budget.
const RESPONSE_HIERARCHY_ROOT_SLOTS: usize = 64;
/// Keep enough definition records for ordinary editor workspaces even when a
/// very large hierarchy consumes every ordinary response slot. Fair
/// multi-workspace quotas reserve one marker slot per workspace when the quota
/// is large enough to leave a useful entry.
const RESPONSE_MODULE_CATALOG_SLOTS: usize = 256;
/// Independent from the serialized-node budget, this guard bounds native
/// call-stack use while walking malformed source/elaboration trees. Normal
/// hierarchies rarely approach it; a deeper hierarchy is represented by the
/// single shared truncation marker even when node budget remains.
pub(super) const MAX_SAFE_HIERARCHY_DEPTH: usize = 64;
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const COMPATIBILITY_TERMINAL_SLOTS: usize = 1;
pub(super) const GRAPH_TERMINAL_SLOTS: usize = 2;

/// Create the budget used by the complete `llg/moduleExplorer` response.
/// Keeping construction here prevents the LSP layer from depending on the
/// accounting details while still allowing it to share one budget across
/// independent analysis roots.
pub(crate) fn new_response_budget() -> InstanceBudget {
    InstanceBudget::with_reserved_slots(
        GRAPH_TERMINAL_SLOTS,
        RESPONSE_HIERARCHY_ROOT_SLOTS,
        RESPONSE_MODULE_CATALOG_SLOTS,
    )
}

/// The serialized instance budget is shared by one complete snapshot.  Keep
/// terminal slots available so a cycle can still be represented when the
/// ordinary expansion slots have been consumed; a budget marker uses one of
/// the same slots and then stops all remaining hierarchy traversal.  Catalog
/// admission is independent of that hierarchy stop because the catalog is
/// serialized after hierarchy expansion.
pub(crate) struct InstanceBudget {
    pub(super) remaining: usize,
    pub(super) terminal_slots: usize,
    pub(super) hierarchy_root_slots: usize,
    pub(super) future_hierarchy_root_slots: usize,
    /// Total unspent catalog capacity, including capacity reserved for
    /// workspaces that have not been serialized yet.
    pub(super) module_catalog_slots: usize,
    /// Unspent catalog capacity assigned to the current workspace when fair
    /// workspace quotas are active.
    pub(super) module_catalog_workspace_slots: usize,
    pub(super) future_module_catalog_slots: usize,
    pub(super) workspace_catalog_quotas: bool,
    pub(super) module_catalog_marker_reserved: bool,
    pub(super) hierarchy_stopped: bool,
    pub(super) budget_marker_emitted: bool,
    /// This flag is scoped to the current workspace in fair-quota mode.
    pub(super) module_marker_emitted: bool,
}

impl InstanceBudget {
    pub(super) fn new(terminal_slots: usize) -> Self {
        Self::with_reserved_slots(terminal_slots, GUARANTEED_HIERARCHY_ROOT_SLOTS, 2)
    }

    pub(super) fn with_reserved_slots(
        terminal_slots: usize,
        hierarchy_root_slots: usize,
        module_catalog_slots: usize,
    ) -> Self {
        let terminal_slots = terminal_slots.min(MAX_INSTANCE_NODES);
        let hierarchy_root_slots =
            hierarchy_root_slots.min(MAX_INSTANCE_NODES.saturating_sub(terminal_slots));
        let module_catalog_slots = module_catalog_slots.min(
            MAX_INSTANCE_NODES
                .saturating_sub(terminal_slots)
                .saturating_sub(hierarchy_root_slots),
        );
        Self {
            remaining: MAX_INSTANCE_NODES,
            terminal_slots,
            hierarchy_root_slots,
            future_hierarchy_root_slots: 0,
            module_catalog_slots,
            module_catalog_workspace_slots: module_catalog_slots,
            future_module_catalog_slots: 0,
            workspace_catalog_quotas: false,
            module_catalog_marker_reserved: module_catalog_slots > 1,
            hierarchy_stopped: false,
            budget_marker_emitted: false,
            module_marker_emitted: false,
        }
    }

    pub(super) fn should_stop(&self) -> bool {
        self.hierarchy_stopped || self.remaining == 0
    }

    fn reserved_root_slots(&self) -> usize {
        self.hierarchy_root_slots
            .saturating_add(self.future_hierarchy_root_slots)
    }

    /// Divide the root and module-catalog reserves across known workspace
    /// snapshots.
    /// Call [`Self::begin_workspace`] before serializing each one.
    pub(crate) fn prepare_workspaces(&mut self) {
        self.future_hierarchy_root_slots = self
            .future_hierarchy_root_slots
            .saturating_add(self.hierarchy_root_slots);
        self.hierarchy_root_slots = 0;
        if !self.workspace_catalog_quotas {
            self.workspace_catalog_quotas = true;
            self.future_module_catalog_slots = self
                .future_module_catalog_slots
                .saturating_add(self.module_catalog_slots);
            self.module_catalog_workspace_slots = 0;
            self.module_catalog_marker_reserved = false;
            self.module_marker_emitted = false;
        }
    }

    /// Assign a fair share of the still-reserved hierarchy and module-catalog
    /// capacity to the next workspace. Unused capacity from the previous
    /// workspace is returned to each pool before the split.
    pub(crate) fn begin_workspace(&mut self, remaining_workspaces: usize) {
        self.future_hierarchy_root_slots = self
            .future_hierarchy_root_slots
            .saturating_add(self.hierarchy_root_slots);
        self.hierarchy_root_slots = 0;
        if !self.workspace_catalog_quotas {
            self.workspace_catalog_quotas = true;
            self.future_module_catalog_slots = self
                .future_module_catalog_slots
                .saturating_add(self.module_catalog_slots);
        } else {
            self.future_module_catalog_slots = self
                .future_module_catalog_slots
                .saturating_add(self.module_catalog_workspace_slots);
        }
        self.module_catalog_workspace_slots = 0;
        self.module_catalog_marker_reserved = false;
        self.module_marker_emitted = false;
        if remaining_workspaces != 0 && self.future_hierarchy_root_slots != 0 {
            let quota = self
                .future_hierarchy_root_slots
                .div_ceil(remaining_workspaces);
            self.future_hierarchy_root_slots -= quota;
            self.hierarchy_root_slots = quota;
        }
        if remaining_workspaces != 0 && self.future_module_catalog_slots != 0 {
            let quota = self
                .future_module_catalog_slots
                .div_ceil(remaining_workspaces);
            self.future_module_catalog_slots -= quota;
            self.module_catalog_workspace_slots = quota;
            // A quota of one is more useful as a real catalog entry than as
            // a truncation marker. Larger quotas retain one slot for that
            // marker when another definition remains.
            self.module_catalog_marker_reserved = quota > 1;
        }
    }

    pub(super) fn take_regular(&mut self) -> bool {
        if self.hierarchy_stopped
            || self.remaining
                <= self
                    .terminal_slots
                    .saturating_add(self.reserved_root_slots())
                    .saturating_add(self.module_catalog_slots)
        {
            return false;
        }
        self.remaining -= 1;
        true
    }

    pub(super) fn can_take_root(&self) -> bool {
        (self.hierarchy_root_slots > 0
            && self.remaining
                > self
                    .future_hierarchy_root_slots
                    .saturating_add(self.module_catalog_slots))
            || (!self.hierarchy_stopped
                && self.remaining
                    > self
                        .terminal_slots
                        .saturating_add(self.future_hierarchy_root_slots)
                        .saturating_add(self.module_catalog_slots))
    }

    pub(super) fn take_root(&mut self) -> bool {
        if !self.hierarchy_stopped
            && self.remaining
                > self
                    .terminal_slots
                    .saturating_add(self.reserved_root_slots())
                    .saturating_add(self.module_catalog_slots)
        {
            self.remaining -= 1;
            return true;
        }
        if self.hierarchy_root_slots > 0
            && self.remaining
                > self
                    .future_hierarchy_root_slots
                    .saturating_add(self.module_catalog_slots)
        {
            self.remaining -= 1;
            self.hierarchy_root_slots -= 1;
            return true;
        }
        false
    }

    pub(super) fn take_module(&mut self, more_definitions: bool) -> bool {
        // The catalog is serialized after hierarchy expansion. A hierarchy or
        // content marker may therefore have stopped hierarchy traversal, but
        // it must not prevent a reserved catalog entry from being emitted.
        if self.workspace_catalog_quotas {
            if self.module_catalog_workspace_slots == 0
                || self.remaining <= self.reserved_root_slots()
                || (more_definitions
                    && self.module_catalog_marker_reserved
                    && self.module_catalog_workspace_slots == 1)
            {
                return false;
            }
            self.remaining -= 1;
            self.module_catalog_slots -= 1;
            self.module_catalog_workspace_slots -= 1;
            if self.module_catalog_workspace_slots == 0 {
                self.module_catalog_marker_reserved = false;
            }
            return true;
        }
        if self.remaining
            > self
                .terminal_slots
                .saturating_add(self.reserved_root_slots())
                .saturating_add(self.module_catalog_slots)
        {
            self.remaining -= 1;
            return true;
        }
        // Keep the last catalog slot for an explicit marker if there are more
        // definitions than the reserved prefix can represent. The final
        // definition may consume that slot because no marker is needed after
        // the catalog is complete.
        if self.module_catalog_slots > usize::from(more_definitions)
            && self.remaining > self.reserved_root_slots()
        {
            self.remaining -= 1;
            self.module_catalog_slots -= 1;
            return true;
        }
        false
    }

    pub(super) fn take_terminal(&mut self) -> bool {
        if self.hierarchy_stopped
            || self.remaining
                <= self
                    .reserved_root_slots()
                    .saturating_add(self.module_catalog_slots)
        {
            self.hierarchy_stopped = true;
            return false;
        }
        self.remaining -= 1;
        self.terminal_slots = self.terminal_slots.saturating_sub(1);
        if self.remaining == 0 {
            self.hierarchy_stopped = true;
        }
        true
    }

    pub(super) fn take_budget_marker(&mut self) -> bool {
        if self.hierarchy_stopped || self.budget_marker_emitted || self.remaining == 0 {
            self.hierarchy_stopped = true;
            return false;
        }
        // A truncation marker is itself a serialized hierarchy node. It may
        // consume an ordinary slot while there is room, or one of the
        // reserved terminal slots at the normal budget boundary. Do not
        // consume the final hierarchy slot: a later root must remain
        // representable even when this marker stops ordinary traversal.
        let ordinary_slots = self.remaining.saturating_sub(
            self.terminal_slots
                .saturating_add(self.reserved_root_slots())
                .saturating_add(self.module_catalog_slots),
        );
        if ordinary_slots > 0 {
            self.remaining -= 1;
        } else if self.terminal_slots > 0 {
            self.remaining -= 1;
            self.terminal_slots = self.terminal_slots.saturating_sub(1);
        } else if self.hierarchy_root_slots > 1 {
            // A cycle may have consumed every terminal slot before ordinary
            // traversal reaches its boundary. Spend one extra root slot for
            // the marker, but retain a root floor for the response.
            self.remaining -= 1;
            self.hierarchy_root_slots -= 1;
        } else {
            self.hierarchy_stopped = true;
            return false;
        }
        self.budget_marker_emitted = true;
        self.hierarchy_stopped = true;
        true
    }

    pub(super) fn take_module_marker(&mut self) -> bool {
        if self.workspace_catalog_quotas {
            if self.module_marker_emitted
                || !self.module_catalog_marker_reserved
                || self.module_catalog_workspace_slots == 0
                || self.remaining <= self.reserved_root_slots()
            {
                return false;
            }
            self.remaining -= 1;
            self.module_catalog_slots -= 1;
            self.module_catalog_workspace_slots -= 1;
            self.module_catalog_marker_reserved = false;
            self.module_marker_emitted = true;
            return true;
        }
        if self.module_marker_emitted
            || self.module_catalog_slots == 0
            || self.remaining <= self.reserved_root_slots()
        {
            return false;
        }
        self.remaining -= 1;
        self.module_catalog_slots -= 1;
        self.module_marker_emitted = true;
        true
    }
}
