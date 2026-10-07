//! Names.

use super::*;

impl<'a> Codegen<'a> {
    /// Readable hierarchy from source components, never from C symbols.
    pub(in super::super) fn source_path(&self, path: &str) -> String {
        self.c_paths.get(path).map_or_else(
            || self.display_path(path).to_owned(),
            |parts| parts.join("."),
        )
    }

    fn storage_source_name(&self, id: NodeId) -> String {
        let mut parts = vec![self.node(id).name.clone()];
        let mut current = self.node(id).parent;
        while let Some(parent) = current {
            let node = self.node(parent);
            if matches!(
                node.kind,
                NodeKind::ModuleInst { .. }
                    | NodeKind::GenScopeArray
                    | NodeKind::GenScope
                    | NodeKind::FuncTask { .. }
                    | NodeKind::ClassDef
                    | NodeKind::Package
            ) && !node.name.is_empty()
            {
                parts.push(match node.kind {
                    NodeKind::ModuleInst { is_top: true, .. } => strip_lib(&node.name),
                    _ => node.name.clone(),
                });
            }
            current = node.parent;
        }
        parts.reverse();
        parts.join(".")
    }

    fn aggregate_storage_label(
        &self,
        mut matches: impl FnMut(&AggregateMemberInfo) -> bool,
    ) -> Option<String> {
        for node in sorted_node_ids(&self.unpacked_aggregates) {
            if let Some(leaf) = self.unpacked_aggregates[&node]
                .leaves
                .iter()
                .find(|leaf| matches(leaf))
            {
                let mut name = self.storage_source_name(node);
                for part in &leaf.path {
                    match part {
                        AggregatePathPart::Member(member) => {
                            name.push('.');
                            name.push_str(member);
                        }
                        AggregatePathPart::Index(index) => name.push_str(&format!("[{index}]")),
                    }
                }
                return Some(name);
            }
        }
        None
    }

    fn signal_source_node(&self, signal: usize) -> Option<NodeId> {
        self.sig_globals
            .iter()
            .filter_map(|(node, info)| (info.ir == signal).then_some(*node))
            .chain(
                self.static_formals
                    .iter()
                    .chain(self.static_task_locals.iter())
                    .filter_map(|((_, node), info)| (info.ir == signal).then_some(*node)),
            )
            .chain(
                self.proc_local_instances
                    .iter()
                    .filter_map(|((_, node), info)| {
                        info.static_signal
                            .as_ref()
                            .filter(|info| info.ir == signal)
                            .map(|_| *node)
                    }),
            )
            .min_by_key(|node| node.index())
    }

    pub(in super::super) fn signal_label(&self, signal: usize) -> String {
        self.aggregate_storage_label(|leaf| {
            leaf.signal.as_ref().is_some_and(|info| info.ir == signal)
        })
        .or_else(|| {
            self.model
                .signals
                .get(signal)?
                .hdl_name
                .as_ref()
                .map(|name| name.replace('\u{1f}', "."))
        })
        .or_else(|| {
            self.signal_source_node(signal)
                .map(|node| self.storage_source_name(node))
        })
        .unwrap_or_else(|| "unnamed signal storage".to_owned())
    }

    /// Diagnostic spelling of canonical storage, including source array bounds.
    pub(in super::super) fn dependency_label(&self, dependency: &IrDependency) -> String {
        match dependency {
            IrDependency::Scalar(name) | IrDependency::Real(name) => self
                .model
                .signals
                .iter()
                .position(|signal| signal.c_name == *name)
                .map_or_else(
                    || "unnamed signal storage".to_owned(),
                    |signal| self.signal_label(signal),
                ),
            IrDependency::PackedRange {
                storage,
                lsb,
                width,
            } => {
                // A writer-analysis row interval names flattened cells.
                if let IrDependency::ArrayContents(array) = storage.as_ref() {
                    let cell = |index: u64| {
                        self.dependency_label(&IrDependency::ArrayElement {
                            array: *array,
                            index,
                        })
                    };
                    let first = u64::from(*lsb);
                    let last = first + u64::from(*width).saturating_sub(1);
                    return format!("{} through {}", cell(first), cell(last));
                }
                let ranges = match storage.as_ref() {
                    IrDependency::Scalar(name) => self
                        .model
                        .signals
                        .iter()
                        .position(|signal| signal.c_name == *name)
                        .and_then(|signal| {
                            sorted_node_ids(&self.unpacked_aggregates)
                                .into_iter()
                                .find_map(|node| {
                                    self.unpacked_aggregates[&node]
                                        .leaves
                                        .iter()
                                        .find(|leaf| {
                                            leaf.signal
                                                .as_ref()
                                                .is_some_and(|info| info.ir == signal)
                                        })
                                        .map(|leaf| leaf.member.packed_ranges.as_slice())
                                })
                                .or_else(|| {
                                    self.signal_source_node(signal)
                                        .and_then(|node| self.db.packed_dimensions(node))
                                })
                        }),
                    IrDependency::ArrayElement { array, .. } => self
                        .array_globals
                        .iter()
                        .filter_map(|(node, info)| (info.ir == *array).then_some(*node))
                        .min_by_key(|node| node.index())
                        .and_then(|node| self.db.packed_dimensions(node)),
                    _ => None,
                };
                let range = ranges.and_then(|dimensions| match dimensions {
                    [range] => Some(range),
                    _ => None,
                });
                let (base, direction) = range.map_or((i128::from(*lsb), "+"), |range| {
                    if range.left >= range.right {
                        (range.right + i128::from(*lsb), "+")
                    } else {
                        (range.right - i128::from(*lsb), "-")
                    }
                });
                let label = self.dependency_label(storage);
                if ranges.is_some_and(|ranges| ranges.len() > 1) {
                    return format!("{label} (packed bits {lsb} +: {width})");
                }
                if *width == 1 {
                    format!("{label}[{base}]")
                } else {
                    format!("{label}[{base} {direction}: {width}]")
                }
            }
            IrDependency::ArrayElement { array, index } => self
                .model
                .arrays
                .get(*array)
                .and_then(|array| array.waveform_element_name(*index))
                .map_or_else(
                    || "unnamed array element".to_owned(),
                    |name| name.replace('\u{1f}', "."),
                ),
            IrDependency::ArrayContents(array) => self.model.arrays.get(*array).map_or_else(
                || "unnamed array storage".to_owned(),
                |array| array.hdl_name.replace('\u{1f}', "."),
            ),
            IrDependency::ContainerContents(container)
            | IrDependency::ContainerShape(container) => {
                let name = self
                    .container_globals
                    .iter()
                    .filter_map(|(node, info)| (info.ir == *container).then_some(*node))
                    .min_by_key(|node| node.index())
                    .map_or_else(
                        || "unnamed container storage".to_owned(),
                        |node| self.storage_source_name(node),
                    );
                if matches!(dependency, IrDependency::ContainerShape(_)) {
                    let method = if self
                        .model
                        .containers
                        .get(*container)
                        .is_some_and(|container| {
                            matches!(container.kind, IrContainerKind::Associative { .. })
                        }) {
                        "num"
                    } else {
                        "size"
                    };
                    format!("{name}.{method}()")
                } else {
                    name
                }
            }
            IrDependency::SharedCell { .. } => "fork-shared automatic storage".to_owned(),
            IrDependency::RefFormal { .. } => "`ref` formal storage".to_owned(),
            IrDependency::NativeAccess(_) => {
                "class property or interface member storage".to_owned()
            }
            IrDependency::Object(object) => self
                .aggregate_storage_label(|leaf| leaf.object == Some(*object))
                .or_else(|| {
                    self.object_globals
                        .iter()
                        .chain(self.class_static_objects.iter())
                        .filter_map(|(node, index)| (*index == *object).then_some(*node))
                        .chain(
                            self.static_string_formals
                                .iter()
                                .chain(self.static_string_task_locals.iter())
                                .filter_map(|((_, node), index)| {
                                    (*index == *object).then_some(*node)
                                }),
                        )
                        .min_by_key(|node| node.index())
                        .map(|node| self.storage_source_name(node))
                })
                .unwrap_or_else(|| "unnamed object storage".to_owned()),
        }
    }

    pub(in super::super) fn display_path<'p>(&'p self, path: &'p str) -> &'p str {
        self.display_paths.get(path).map_or(path, String::as_str)
    }

    /// `%l` text for code owned by `scope` (SV §33.7): `library.cell` of the
    /// nearest enclosing instance's bound definition, or `library.$unit`
    /// outside a design element, matching the frontend formatter. Captured
    /// databases always carry the library; synthetic ones fall back to the
    /// default library name.
    pub(in super::super) fn library_binding(&self, scope: NodeId) -> String {
        const DEFAULT_SOURCE_LIBRARY: &str = "work";
        let mut current = Some(scope);
        while let Some(id) = current {
            let library = self.db.source_library(id).unwrap_or(DEFAULT_SOURCE_LIBRARY);
            if let NodeKind::ModuleInst { def_name, .. } = self.kind(id) {
                return format!("{library}.{}", strip_lib(def_name));
            }
            if self.is_runtime_environment(id) {
                return format!("{library}.$unit");
            }
            current = self.node(id).parent;
        }
        format!("{DEFAULT_SOURCE_LIBRARY}.$unit")
    }

    /// Replace `%l`/`%L` in a literal runtime format with the static library
    /// binding of `path`'s scope. Other specifications, including `%%`, are
    /// copied unchanged; a path without a registered scope keeps the format.
    pub(in super::super) fn bind_library_format(&self, path: &str, format: Vec<u8>) -> Vec<u8> {
        let Some(scope) = self.scope_nodes.get(path) else {
            return format;
        };
        if !format.contains(&b'%') {
            return format;
        }
        let binding = self.library_binding(*scope).replace('%', "%%");
        let mut bound = Vec::with_capacity(format.len());
        let mut index = 0;
        while index < format.len() {
            if format[index] != b'%' {
                bound.push(format[index]);
                index += 1;
                continue;
            }
            // Same specification grammar as display lowering: flags, width
            // and precision digits, then one conversion character.
            let mut end = index + 1;
            while end < format.len() && matches!(format[end], b'-' | b'.' | b'0'..=b'9') {
                end += 1;
            }
            if end < format.len() && matches!(format[end], b'l' | b'L') {
                bound.extend_from_slice(binding.as_bytes());
            } else {
                bound.extend_from_slice(&format[index..(end + 1).min(format.len())]);
            }
            index = end + 1;
        }
        bound
    }

    pub(in super::super) fn c_path_ident(&self, path: &str) -> String {
        match self.c_paths.get(path) {
            Some(parts) => path_ident(&parts.iter().map(String::as_str).collect::<Vec<_>>()),
            None => ident(path),
        }
    }

    pub(in super::super) fn c_name(&self, prefix: &str, path: &str, names: &[&str]) -> String {
        let mut parts = match self.c_paths.get(path) {
            Some(parts) => parts.iter().map(String::as_str).collect::<Vec<_>>(),
            None => vec![path],
        };
        parts.extend_from_slice(names);
        scoped_name(prefix, &parts)
    }

    pub(in super::super) fn global_name(&self, path: &str, name: &str) -> String {
        if self.c_paths.contains_key(path) {
            self.c_name("G", path, &[name])
        } else {
            global_name(path, name)
        }
    }

    pub(in super::super) fn real_global_name(&self, path: &str, name: &str) -> String {
        if self.c_paths.contains_key(path) {
            self.c_name("D", path, &[name])
        } else {
            real_global_name(path, name)
        }
    }

    pub(in super::super) fn event_global_name(&self, path: &str, name: &str) -> String {
        if self.c_paths.contains_key(path) {
            self.c_name("E", path, &[name])
        } else {
            event_global_name(path, name)
        }
    }

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
