//! Deferred effects of static initializers.
//!
//! SV 13.4.4 lets a function schedule work that becomes active only after
//! it returns (a `fork`-`join_none`, a nonblocking assignment or a
//! nonblocking event trigger) only when the calling thread was created by an
//! initial or always procedure or a fork of one. A static declaration
//! initializer runs before any process exists (SV 6.8, 10.5), so such a
//! statement must not execute under one; the text requires an error at
//! compile time or at run time (S22-D3 takes run time).
//!
//! Reaching such a statement is not executing it, so the error is raised when
//! the statement runs. Call-graph reachability only chooses where the checks
//! go: `prepare_static_effect_sites` selects the statements a static
//! initializer can reach, through calls, constructors, property initializers,
//! `randomize` callbacks and virtual overrides, and only those get
//! `IrObjectStmt::StaticEffectCheck`. Only initializers that reach one record
//! themselves in the static schedule. A design with neither has no check.
//! Verilog-2001 initializers run as active-region processes, where these
//! statements are legal, so that edition selects nothing.

use super::*;
use crate::sim::ir::IrObjectStmt;

/// A statement to check, with the subroutine it is reached in.
#[derive(Clone, Copy)]
pub(in super::super) struct StaticEffectSite {
    what: &'static str,
    subroutine: NodeId,
}

/// Subroutines reachable from a set of roots, with their deferred statements.
#[derive(Default)]
struct StaticEffectGraph {
    callees: HashMap<NodeId, Vec<NodeId>>,
    sites: HashMap<NodeId, StaticEffectSite>,
}

impl StaticEffectGraph {
    /// The subroutines that have a site or call one that does.
    fn reaching(&self) -> HashSet<NodeId> {
        let mut callers: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for (caller, callees) in &self.callees {
            for callee in callees {
                callers.entry(*callee).or_default().push(*caller);
            }
        }
        let mut pending = self
            .sites
            .values()
            .map(|site| site.subroutine)
            .collect::<Vec<_>>();
        let mut reaching = HashSet::new();
        while let Some(subroutine) = pending.pop() {
            if reaching.insert(subroutine) {
                pending.extend(callers.get(&subroutine).into_iter().flatten().copied());
            }
        }
        reaching
    }
}

impl Codegen<'_> {
    /// Choose the statements that get a check, before any subroutine body is
    /// lowered. Initializers are recorded later, while they are lowered, so
    /// this reads the static declarations from the database and
    /// `static_effect_scope` proves the selection covers what was recorded.
    pub(in super::super) fn prepare_static_effect_sites(&mut self) {
        if self.db.edition() != LanguageEdition::SystemVerilog2009 {
            return;
        }
        let mut roots = self
            .db
            .var_initializers()
            .iter()
            .filter(|(declaration, _)| {
                self.db.variable_lifetime(**declaration) == VariableLifetime::Static
            })
            .map(|(_, initializer)| *initializer)
            .collect::<Vec<_>>();
        // Hash order must not reach which statement names a subroutine.
        roots.sort_unstable_by_key(|root| root.index());
        let graph = self.static_effect_graph(&roots);
        self.static_effect_reaching = graph.reaching();
        self.static_effect_sites = graph.sites;
    }

    /// The check that runs before statement `statement`, when a static
    /// initializer can reach it.
    pub(in super::super) fn static_effect_check(&self, statement: NodeId) -> Option<IrStmt> {
        let site = self.static_effect_sites.get(&statement)?;
        Some(IrStmt::Object(Box::new(IrObjectStmt::StaticEffectCheck {
            what: site.what.to_owned(),
            location: self.source_location(statement),
            subroutine: self.node(site.subroutine).name.clone(),
        })))
    }

    /// The name and location to record around the initializer of
    /// `declaration`, when one of its calls can reach a checked statement.
    pub(super) fn static_effect_scope(&self, declaration: u32) -> Option<(String, String)> {
        if self.static_effect_sites.is_empty() {
            return None;
        }
        let mut callees = Vec::new();
        let sources = self.initializer_sources.get(&declaration)?;
        for source in sources {
            self.walk_static_effects(*source, &mut callees, &mut |_, _| {});
        }
        if !callees
            .iter()
            .any(|callee| self.static_effect_reaching.contains(callee))
        {
            return None;
        }
        let declaration = NodeId::from_index(declaration as usize);
        Some((
            self.node(declaration).name.clone(),
            self.source_location(declaration),
        ))
    }

    /// Every initializer recorded while lowering must have been covered by
    /// the selection made before it: a missing check would hide the error.
    pub(super) fn verify_static_effect_sites(&self) -> Result<(), String> {
        if self.db.edition() != LanguageEdition::SystemVerilog2009 {
            return Ok(());
        }
        let mut roots = self
            .initializer_sources
            .values()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        roots.sort_unstable_by_key(|root| root.index());
        match self
            .static_effect_graph(&roots)
            .sites
            .keys()
            .find(|statement| !self.static_effect_sites.contains_key(statement))
        {
            Some(statement) => Err(format!(
                "statement at {} is reachable from a static initializer but has no check",
                self.source_location(*statement)
            )),
            None => Ok(()),
        }
    }

    fn static_effect_graph(&self, roots: &[NodeId]) -> StaticEffectGraph {
        let mut graph = StaticEffectGraph::default();
        let mut pending = Vec::new();
        for root in roots {
            self.walk_static_effects(*root, &mut pending, &mut |_, _| {});
        }
        while let Some(subroutine) = pending.pop() {
            if graph.callees.contains_key(&subroutine) {
                continue;
            }
            let mut callees = Vec::new();
            let sites = &mut graph.sites;
            self.walk_static_effects(subroutine, &mut callees, &mut |statement, what| {
                sites.insert(statement, StaticEffectSite { what, subroutine });
            });
            pending.extend(callees.iter().copied());
            graph.callees.insert(subroutine, callees);
        }
        graph
    }

    /// Collect what code under `root` calls and the deferred statements it
    /// holds. A fork branch is not walked: it runs only after its fork, which
    /// is itself a deferred statement.
    fn walk_static_effects(
        &self,
        root: NodeId,
        callees: &mut Vec<NodeId>,
        site: &mut impl FnMut(NodeId, &'static str),
    ) {
        let mut stack = vec![root];
        while let Some(current) = stack.pop() {
            let what = match self.kind(current) {
                NodeKind::Stmt(StmtKind::Fork { .. }) => Some("fork"),
                NodeKind::Stmt(StmtKind::Assign {
                    blocking: false, ..
                }) => Some("nonblocking assignment"),
                NodeKind::Stmt(StmtKind::EventTrigger {
                    blocking: false, ..
                }) => Some("nonblocking event trigger"),
                _ => None,
            };
            if let Some(what) = what {
                site(current, what);
                continue;
            }
            self.static_effect_callees(current, callees);
            stack.extend(self.node(current).children.iter().copied());
        }
    }

    /// Everything a call or class construction at `node` can run.
    fn static_effect_callees(&self, node: NodeId, callees: &mut Vec<NodeId>) {
        if let NodeKind::Expr(ExprKind::NewClass { .. }) = self.kind(node) {
            self.static_effect_construction(node, callees);
        }
        if matches!(
            self.kind(node),
            NodeKind::MethodCall { name, .. } | NodeKind::SysCall { name }
                if name == "randomize"
        ) {
            self.static_effect_randomize(callees);
        }
        if let Some(callee) = self.static_effect_callee(node) {
            callees.push(callee);
            self.static_effect_overrides(callee, callees);
        }
    }

    /// A construction runs the constructors of the class and its bases and
    /// the property initializers of each, explicit or not.
    fn static_effect_construction(&self, node: NodeId, callees: &mut Vec<NodeId>) {
        let Some(class) = self.class_node_for_type_expr(node) else {
            return;
        };
        for class in self.class_chain(class) {
            callees.extend(self.class_constructor(class));
            let metadata = self.db.class_metadata(class);
            if let Some(call) = metadata.and_then(|metadata| metadata.base_constructor) {
                self.walk_static_effects(call, callees, &mut |_, _| {});
            }
            for member in self.node(class).children.iter().copied() {
                if self.db.variable_lifetime(member) != VariableLifetime::Static {
                    if let Some(initializer) = self.db.var_initializer(member) {
                        self.walk_static_effects(initializer, callees, &mut |_, _| {});
                    }
                }
            }
        }
    }

    /// `randomize` calls `pre_randomize` and `post_randomize` of the object's
    /// dynamic class, which the call site does not name (SV 18.6.2).
    fn static_effect_randomize(&self, callees: &mut Vec<NodeId>) {
        for class in self.db.classes() {
            callees.extend(
                self.class_method_nodes(*class)
                    .into_iter()
                    .filter(|method| {
                        matches!(
                            self.node(*method).name.as_str(),
                            "pre_randomize" | "post_randomize"
                        )
                    }),
            );
        }
    }

    /// The overrides a virtual call to `callee` can dispatch to: a method of
    /// the same name in a class derived from its class, or in any class when
    /// that class is an interface class, whose implementers are not bases.
    fn static_effect_overrides(&self, callee: NodeId, callees: &mut Vec<NodeId>) {
        let Some(owner) = self.static_effect_method_class(callee) else {
            return;
        };
        let interface = self
            .db
            .class_metadata(owner)
            .is_some_and(|metadata| metadata.is_interface);
        let name = self.node(callee).name.as_str();
        for class in self.db.classes() {
            if !interface && !self.class_chain(*class).contains(&owner) {
                continue;
            }
            callees.extend(
                self.class_method_nodes(*class)
                    .into_iter()
                    .filter(|method| self.node(*method).name == name && *method != callee),
            );
        }
    }

    /// The class of a virtual method, seen through its prototype.
    fn static_effect_method_class(&self, method: NodeId) -> Option<NodeId> {
        let parent = self.node(method).parent?;
        let owner = if matches!(self.kind(parent), NodeKind::FuncTask { .. }) {
            self.node(parent).parent?
        } else {
            parent
        };
        let is_virtual = self.method_virtual_slots.contains_key(&method)
            || self
                .node(method)
                .children
                .iter()
                .any(|child| self.method_virtual_slots.contains_key(child));
        (is_virtual && self.db.class_metadata(owner).is_some()).then_some(owner)
    }

    /// The resolved subroutine a call, method call or class construction
    /// at `node` runs.
    fn static_effect_callee(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => callee.or_else(|| {
                let instance = self.owner_instance(node)?;
                self.resolve_callee_env(instance, name, *is_task, None)
                    .ok()
                    .map(|(subroutine, _)| subroutine)
            }),
            NodeKind::MethodCall { callee, .. } => *callee,
            NodeKind::Expr(ExprKind::NewClass { constructor, .. }) => *constructor,
            _ => None,
        }
        .filter(|subroutine| matches!(self.kind(*subroutine), NodeKind::FuncTask { .. }))
    }
}
