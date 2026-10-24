//! `randsequence` (IEEE 1800-2009 18.17).
//!
//! A randsequence statement lowers to ordinary IR in the calling procedure,
//! so a code block may suspend, `disable` and kills need no special
//! handling, and every random draw comes from the calling process's stream.
//!
//! A production that can neither reach a recursive production nor needs one
//! shared body (see [`RANDSEQUENCE_INLINE_ELEMENT_LIMIT`]) is *inline*: each
//! generation expands its rules in place, with structured `if`/`case`/loops
//! and forward jumps for `return`. Every other production is *segmented*: it
//! has one body whose production-list elements are states of a dispatch loop
//! (`pc`), and generating it stores the continuation state in the
//! production's return-state variable. A generation of a recursive production
//! first pushes the production's live automatic values (formals, implicit
//! variables, return state, repeat counters) onto a heap queue and pops them
//! when it ends, so recursion never uses the C stack; the total depth is
//! checked against [`RANDSEQUENCE_DEPTH_LIMIT`].
//!
//! Random stability and the order of draws are decisions S16-D1..D10 in
//! `docs/lrm_decisions.md`.

use super::*;
use crate::core::db::{CaseItem, ProductionDecl, ProductionItem, ProductionKind, ProductionRule};
use crate::sim::ir::{
    IrContainer, IrContainerElement, IrContainerExpr, IrContainerKind, IrContainerStmt,
    IrStringExpr,
};

/// A production generated from more than one place is expanded at each
/// place while its expansion has at most this many production-list elements;
/// a larger one keeps one shared body in the dispatch loop.
pub(in crate::sim::codegen) const RANDSEQUENCE_INLINE_ELEMENT_LIMIT: usize = 64;

/// Maximum number of simultaneously active generations of recursive
/// productions in one execution of a randsequence statement. Exceeding it is
/// a fatal run-time error located at the generating production item.
pub(in crate::sim::codegen) const RANDSEQUENCE_DEPTH_LIMIT: u32 = 10_000;

/// Static facts about the productions a randsequence statement can generate.
pub(in crate::sim::codegen) struct Grammar {
    decls: HashMap<NodeId, ProductionDecl>,
    /// Declaration order of `decls`, for deterministic output.
    order: Vec<NodeId>,
    rules: HashMap<NodeId, ProductionRule>,
    items: HashMap<NodeId, ProductionItem>,
    /// Number of items (and the statement itself) generating each production.
    sites: HashMap<NodeId, usize>,
    recursive: HashSet<NodeId>,
    segmented: HashSet<NodeId>,
}

/// Where a `return` in a code block goes.
enum ReturnTarget {
    /// An inline generation: assign the result and jump past the body.
    Label {
        label: String,
        used: bool,
        result: Option<NodeId>,
    },
    /// A segmented generation: assign the result and dispatch its exit.
    Exit {
        production: NodeId,
        result: Option<NodeId>,
    },
}

/// Dispatch states of segmented productions.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum State {
    /// The statement's top generation finished.
    Done,
    Entry(NodeId),
    Exit(NodeId),
    /// The rest of a rule after the element (or generated item) `.0`.
    After(NodeId),
    /// A repeat element's loop test.
    Loop(NodeId),
}

/// A recursive production's saved automatic values.
struct Frame {
    container: usize,
    width: u32,
    entries: Vec<FrameEntry>,
    /// Native string storage, saved in a string queue of its own because a
    /// string has no packed form.
    strings: Option<StringFrame>,
}

struct StringFrame {
    container: usize,
    /// The string references, in push order.
    references: Vec<NodeId>,
}

struct FrameEntry {
    read: IrExpr,
    write: IrLhs,
    /// Real storage is saved through its IEEE bits.
    real: bool,
    width: u32,
}

/// Lowering state of one randsequence statement (innermost last).
pub(in crate::sim::codegen) struct RandseqState {
    grammar: std::rc::Rc<Grammar>,
    returns: Vec<ReturnTarget>,
    /// Hidden locals declared at the start of the statement.
    locals: Vec<IrStmt>,
    pc: Option<String>,
    /// Label at the end of the dispatch loop body.
    next: String,
    next_used: bool,
    states: HashMap<State, u32>,
    arms: Vec<(u32, Vec<IrStmt>)>,
    returns_to: HashMap<NodeId, String>,
    /// Repeat counters of elements whose item is segmented.
    counters: HashMap<NodeId, (String, u32, bool)>,
    /// The continuation state of each single-site segmented production.
    continuations: HashMap<NodeId, u32>,
    depth: Option<String>,
    frames: HashMap<NodeId, Frame>,
}

fn int_expr(value: i64, width: u32, signed: bool) -> Result<IrExpr, String> {
    let bits = if width >= 64 {
        value as u64
    } else {
        (value as u64) & ((1u64 << width) - 1)
    };
    let constant = IrConst::packed(vec![bits], vec![], vec![], width, signed, None)
        .map_err(|error| error.to_string())?;
    Ok(IrExpr::new(
        IrExprKind::Const(constant),
        width,
        signed,
        None,
    ))
}

fn local_read(name: &str, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(IrExprKind::LocalRead(name.to_owned()), width, signed, None)
}

fn local_lhs(name: &str, width: u32, signed: bool) -> IrLhs {
    IrLhs::WholeRef {
        addr: format!("&{name}"),
        width,
        signed,
        two_state: true,
        shortreal: false,
    }
}

fn assign(lhs: IrLhs, rhs: IrExpr) -> IrStmt {
    IrStmt::Assign {
        lhs,
        rhs,
        nba: false,
    }
}

fn int_local(name: &str, init: Option<IrExpr>) -> IrStmt {
    IrStmt::DeclLocal {
        name: name.to_owned(),
        width: 32,
        signed: true,
        two_state: true,
        init: init.map(Box::new),
    }
}

fn if_then(cond: IrExpr, then_: Vec<IrStmt>, els: Option<Vec<IrStmt>>) -> IrStmt {
    IrStmt::If {
        cond,
        then_,
        els,
        check: IrUniquePriorityCheck::None,
    }
}

/// `case (sel)` over constant 32-bit labels; unmatched selects nothing.
fn index_case(sel: IrExpr, arms: Vec<(u32, Vec<IrStmt>)>) -> Result<IrStmt, String> {
    let mut items = Vec::with_capacity(arms.len());
    for (label, body) in arms {
        items.push(IrCaseItem {
            exprs: vec![int_expr(i64::from(label), 32, true)?],
            body,
        });
    }
    Ok(IrStmt::Case {
        sel,
        kind: IrCaseKind::Exact,
        items,
        check: IrUniquePriorityCheck::None,
    })
}

/// The productions an element generates, in source order.
/// One step of a sequence that `rand join` interleaves.
#[derive(Clone, Copy)]
enum JoinStep {
    /// A production item or code block of the joined rule.
    Element(NodeId),
    /// The joined rule is itself a `rand join`: one step.
    Join,
    /// The rule's weight code block, after its last element.
    Weight(NodeId),
}

fn element_items(kind: &ProductionKind, out: &mut Vec<NodeId>) {
    match kind {
        ProductionKind::If {
            then_item,
            else_item,
            ..
        } => {
            out.push(*then_item);
            out.extend(*else_item);
        }
        ProductionKind::Repeat { item, .. } => out.push(*item),
        ProductionKind::Case { items, default, .. } => {
            out.extend(items.iter().map(|item| item.item));
            out.extend(*default);
        }
        ProductionKind::Item(_)
        | ProductionKind::CodeBlock { .. }
        | ProductionKind::Declaration(_)
        | ProductionKind::Rule(_) => {}
    }
}

impl Grammar {
    fn production_of(&self, item: NodeId) -> NodeId {
        self.items[&item]
            .production
            .expect("grammar items are resolved")
    }

    /// The production items of `production`'s rules, in source order.
    fn rule_items(&self, db: &crate::core::db::Db, rule: &ProductionRule) -> Vec<NodeId> {
        let mut items = Vec::new();
        for element in &rule.elements {
            match db.node_kind(*element) {
                NodeKind::Production(ProductionKind::Item(_)) => items.push(*element),
                NodeKind::Production(kind) => element_items(kind, &mut items),
                _ => {}
            }
        }
        items
    }

    fn callees(&self, db: &crate::core::db::Db, production: NodeId) -> Vec<NodeId> {
        let mut callees = Vec::new();
        for rule in &self.decls[&production].rules {
            for item in self.rule_items(db, &self.rules[rule]) {
                callees.push(self.production_of(item));
            }
        }
        callees
    }

    /// Whether `item` generates a segmented production.
    fn segmented_item(&self, item: NodeId) -> bool {
        self.segmented.contains(&self.production_of(item))
    }
}

impl<'c, 'a> EmitCtx<'c, 'a> {
    fn production_location(&self, node: NodeId) -> String {
        self.cg.source_location(node)
    }

    fn production_name(&self, production: NodeId) -> String {
        self.cg.node(production).name.clone()
    }

    /// Analyse the productions reachable from `top` (an item).
    fn randsequence_grammar(&mut self, top: NodeId) -> Result<Grammar, String> {
        let db = self.cg.db;
        let mut grammar = Grammar {
            decls: HashMap::new(),
            order: Vec::new(),
            rules: HashMap::new(),
            items: HashMap::new(),
            sites: HashMap::new(),
            recursive: HashSet::new(),
            segmented: HashSet::new(),
        };
        let mut pending = vec![top];
        while let Some(item) = pending.pop() {
            let NodeKind::Production(ProductionKind::Item(info)) = db.node_kind(item) else {
                return Err(format!(
                    "randsequence production item at {} is malformed",
                    self.production_location(item)
                ));
            };
            let production = info.production.ok_or_else(|| {
                format!(
                    "randsequence production item at {} names no production",
                    self.production_location(item)
                )
            })?;
            grammar.items.insert(item, info.clone());
            *grammar.sites.entry(production).or_default() += 1;
            if grammar.decls.contains_key(&production) {
                continue;
            }
            let NodeKind::Production(ProductionKind::Declaration(decl)) = db.node_kind(production)
            else {
                return Err(format!(
                    "randsequence production at {} is malformed",
                    self.production_location(production)
                ));
            };
            if let Some(index) = decl.non_input.iter().position(|flag| *flag) {
                return Err(format!(
                    "randsequence production `{}` at {}: formal `{}` is not an input; llg \
                     supports only input production arguments",
                    self.production_name(production),
                    self.production_location(production),
                    self.cg.node(decl.formals[index]).name
                ));
            }
            grammar.decls.insert(production, decl.as_ref().clone());
            grammar.order.push(production);
            for rule in &decl.rules {
                let NodeKind::Production(ProductionKind::Rule(info)) = db.node_kind(*rule) else {
                    return Err(format!(
                        "randsequence rule at {} is malformed",
                        self.production_location(*rule)
                    ));
                };
                grammar.rules.insert(*rule, info.as_ref().clone());
                let items = grammar.rule_items(db, info);
                pending.extend(items.into_iter().rev());
            }
        }
        grammar.order.sort_by_key(|production| production.index());
        self.find_recursion(&mut grammar);
        self.segment(&mut grammar, top)?;
        self.check_joins(&grammar)?;
        Ok(grammar)
    }

    /// Mark productions on a cycle of the generation graph (Tarjan).
    fn find_recursion(&self, grammar: &mut Grammar) {
        struct Tarjan<'g> {
            edges: &'g HashMap<NodeId, Vec<NodeId>>,
            index: HashMap<NodeId, usize>,
            low: HashMap<NodeId, usize>,
            stack: Vec<NodeId>,
            on_stack: HashSet<NodeId>,
            next: usize,
            recursive: HashSet<NodeId>,
        }
        impl Tarjan<'_> {
            fn visit(&mut self, node: NodeId) {
                self.index.insert(node, self.next);
                self.low.insert(node, self.next);
                self.next += 1;
                self.stack.push(node);
                self.on_stack.insert(node);
                for callee in self.edges[&node].clone() {
                    if !self.index.contains_key(&callee) {
                        self.visit(callee);
                        let low = self.low[&node].min(self.low[&callee]);
                        self.low.insert(node, low);
                    } else if self.on_stack.contains(&callee) {
                        let low = self.low[&node].min(self.index[&callee]);
                        self.low.insert(node, low);
                    }
                }
                if self.low[&node] == self.index[&node] {
                    let mut component = Vec::new();
                    while let Some(member) = self.stack.pop() {
                        self.on_stack.remove(&member);
                        component.push(member);
                        if member == node {
                            break;
                        }
                    }
                    if component.len() > 1 || self.edges[&node].contains(&node) {
                        self.recursive.extend(component);
                    }
                }
            }
        }
        let db = self.cg.db;
        let edges = grammar
            .order
            .iter()
            .map(|production| (*production, grammar.callees(db, *production)))
            .collect::<HashMap<_, _>>();
        let mut tarjan = Tarjan {
            edges: &edges,
            index: HashMap::new(),
            low: HashMap::new(),
            stack: Vec::new(),
            on_stack: HashSet::new(),
            next: 0,
            recursive: HashSet::new(),
        };
        for production in &grammar.order {
            if !tarjan.index.contains_key(production) {
                tarjan.visit(*production);
            }
        }
        grammar.recursive = tarjan.recursive;
    }

    /// Choose the segmented productions: recursive ones, large ones generated
    /// from several places, and every production that generates one of them.
    /// Productions a `rand join` interleaves stay inline (only recursion
    /// segments them, which `check_joins` rejects).
    fn segment(&self, grammar: &mut Grammar, top: NodeId) -> Result<(), String> {
        let db = self.cg.db;
        let mut joined = HashSet::new();
        for rule in grammar.rules.values() {
            if rule.rand_join.is_some() {
                let mut pending = rule
                    .elements
                    .iter()
                    .map(|item| grammar.production_of(*item))
                    .collect::<Vec<_>>();
                while let Some(production) = pending.pop() {
                    if joined.insert(production) {
                        pending.extend(grammar.callees(db, production));
                    }
                }
            }
        }
        // Element counts of inline expansions, callees first.
        let mut size: HashMap<NodeId, usize> = HashMap::new();
        let mut segmented = grammar.recursive.clone();
        let mut state: HashMap<NodeId, bool> = HashMap::new();
        fn visit(
            production: NodeId,
            grammar: &Grammar,
            db: &crate::core::db::Db,
            joined: &HashSet<NodeId>,
            state: &mut HashMap<NodeId, bool>,
            size: &mut HashMap<NodeId, usize>,
            segmented: &mut HashSet<NodeId>,
        ) {
            if state.contains_key(&production) {
                return;
            }
            state.insert(production, false);
            let callees = grammar.callees(db, production);
            for callee in &callees {
                visit(*callee, grammar, db, joined, state, size, segmented);
            }
            state.insert(production, true);
            if segmented.contains(&production) {
                return;
            }
            if callees.iter().any(|callee| segmented.contains(callee)) {
                segmented.insert(production);
                return;
            }
            let mut total = 0usize;
            for rule in &grammar.decls[&production].rules {
                let rule = &grammar.rules[rule];
                total = total.saturating_add(rule.elements.len());
                for item in grammar.rule_items(db, rule) {
                    let callee = grammar.production_of(item);
                    total = total.saturating_add(size.get(&callee).copied().unwrap_or(1));
                }
            }
            size.insert(production, total);
            if grammar.sites.get(&production).copied().unwrap_or(0) > 1
                && total > RANDSEQUENCE_INLINE_ELEMENT_LIMIT
                && !joined.contains(&production)
            {
                segmented.insert(production);
            }
        }
        let first = grammar.production_of(top);
        visit(
            first,
            grammar,
            db,
            &joined,
            &mut state,
            &mut size,
            &mut segmented,
        );
        // Recursive productions are segmented above; a production that
        // generates a segmented one is segmented too.
        loop {
            let before = segmented.len();
            for production in &grammar.order {
                if !segmented.contains(production)
                    && grammar
                        .callees(db, *production)
                        .iter()
                        .any(|callee| segmented.contains(callee))
                {
                    segmented.insert(*production);
                }
            }
            if segmented.len() == before {
                break;
            }
        }
        grammar.segmented = segmented;
        Ok(())
    }

    /// `rand join` interleaves the production lists of its items, so their
    /// automatic values are live at the same time: each must be generated
    /// inline and none may generate another (or itself). Each item names a
    /// production of one rule whose elements are production items and code
    /// blocks: 18.17.5 interleaves "production sequences" without saying how
    /// alternatives or if, case and repeat items take part, and
    /// industry-standard tools reject them (decision S16-D5).
    fn check_joins(&self, grammar: &Grammar) -> Result<(), String> {
        let db = self.cg.db;
        let reaches = |from: NodeId| {
            let mut seen = HashSet::new();
            let mut pending = grammar.callees(db, from);
            while let Some(production) = pending.pop() {
                if seen.insert(production) {
                    pending.extend(grammar.callees(db, production));
                }
            }
            seen
        };
        for rule_node in &grammar
            .order
            .iter()
            .flat_map(|production| grammar.decls[production].rules.clone())
            .collect::<Vec<_>>()
        {
            let rule = &grammar.rules[rule_node];
            if rule.rand_join.is_none() {
                continue;
            }
            let productions = rule
                .elements
                .iter()
                .map(|item| grammar.production_of(*item))
                .collect::<Vec<_>>();
            for (index, (item, production)) in rule.elements.iter().zip(&productions).enumerate() {
                let name = self.production_name(*production);
                let rules = &grammar.decls[production].rules;
                let control = rules
                    .iter()
                    .flat_map(|rule| &grammar.rules[rule].elements)
                    .any(|element| {
                        matches!(
                            self.cg.kind(*element),
                            NodeKind::Production(
                                ProductionKind::If { .. }
                                    | ProductionKind::Case { .. }
                                    | ProductionKind::Repeat { .. }
                            )
                        )
                    });
                if rules.len() != 1 || control {
                    let what = if rules.len() != 1 {
                        "has more than one rule"
                    } else {
                        "contains an if, case or repeat item"
                    };
                    return Err(format!(
                        "rand join at {}: production `{name}` {what}; a production that rand \
                         join interleaves must have one rule of production items and code blocks",
                        self.production_location(*item)
                    ));
                }
                if grammar.segmented.contains(production) {
                    return Err(format!(
                        "rand join at {}: production `{name}` is recursive or generates a \
                         recursive production; llg interleaves only non-recursive productions",
                        self.production_location(*item)
                    ));
                }
                if productions[..index].contains(production) {
                    return Err(format!(
                        "rand join at {}: production `{name}` is interleaved more than once; \
                         llg requires distinct productions",
                        self.production_location(*item)
                    ));
                }
                let reachable = reaches(*production);
                if let Some(other) = productions.iter().find(|other| reachable.contains(other)) {
                    return Err(format!(
                        "rand join at {}: production `{name}` generates `{}`, which the same \
                         rand join interleaves; llg does not support this",
                        self.production_location(*item),
                        self.production_name(*other)
                    ));
                }
            }
        }
        Ok(())
    }

    fn randseq(&mut self) -> &mut RandseqState {
        self.randseq
            .last_mut()
            .expect("randsequence lowering state")
    }

    fn grammar(&self) -> std::rc::Rc<Grammar> {
        self.randseq
            .last()
            .expect("randsequence lowering state")
            .grammar
            .clone()
    }

    pub(super) fn lower_randsequence(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let top = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::RandSequence { top, .. }) => *top,
            _ => unreachable!("non-randsequence passed to lower_randsequence"),
        };
        let Some(top) = top else {
            return Ok(Vec::new());
        };
        let grammar = std::rc::Rc::new(self.randsequence_grammar(top)?);
        let exit = self.new_label("rsx");
        let next = self.new_label("rsn");
        self.ctrl.push(CtrlScope::RandSequence {
            exit: exit.clone(),
            exit_used: false,
        });
        self.randseq.push(RandseqState {
            grammar,
            returns: Vec::new(),
            locals: Vec::new(),
            pc: None,
            next,
            next_used: false,
            states: HashMap::new(),
            arms: Vec::new(),
            returns_to: HashMap::new(),
            counters: HashMap::new(),
            continuations: HashMap::new(),
            depth: None,
            frames: HashMap::new(),
        });
        let lowered = self.lower_randsequence_body(top);
        let state = self.randseq.pop().expect("randsequence lowering state");
        let exit_used = match self.ctrl.pop() {
            Some(CtrlScope::RandSequence { exit_used, .. }) => exit_used,
            _ => unreachable!("randsequence scope stack imbalance"),
        };
        let (declarations, body) = lowered?;
        let mut statements = declarations;
        statements.extend(state.locals);
        statements.extend(body);
        if exit_used {
            statements.push(IrStmt::Label(exit));
        }
        Ok(vec![IrStmt::Block(statements)])
    }

    /// The variable declarations and the generation of the first production.
    fn lower_randsequence_body(
        &mut self,
        top: NodeId,
    ) -> Result<(Vec<IrStmt>, Vec<IrStmt>), String> {
        let grammar = self.grammar();
        // Every production's automatic variables live for the whole
        // statement; each generation resets or binds the ones it uses.
        let mut declarations = Vec::new();
        for production in grammar.order.clone() {
            let mut owners = vec![production];
            owners.extend(grammar.decls[&production].rules.iter().copied());
            for owner in owners {
                for child in self.cg.node(owner).children.clone() {
                    if matches!(
                        self.cg.kind(child),
                        NodeKind::Var { .. } | NodeKind::Array { .. }
                    ) && self.cg.node(child).parent == Some(owner)
                    {
                        declarations.extend(self.lower_variable_decl(child).map_err(|error| {
                            format!(
                                "randsequence production `{}` at {}: {error}",
                                self.production_name(production),
                                self.production_location(production)
                            )
                        })?);
                    }
                }
            }
        }
        let first = grammar.production_of(top);
        if !grammar.segmented.contains(&first) {
            return Ok((declarations, self.expand_item(top)?));
        }
        // Dispatch loop over the segmented productions' states.
        let pc = self.new_label("rspc");
        self.randseq().pc = Some(pc.clone());
        let mut body = self.call_segmented(top, State::Done)?;
        let segmented = grammar
            .order
            .iter()
            .copied()
            .filter(|production| grammar.segmented.contains(production))
            .collect::<Vec<_>>();
        for production in &segmented {
            self.lower_segmented_production(*production)?;
        }
        // Exits last: every generation site has recorded its continuation.
        for production in &segmented {
            self.segmented_exit(*production)?;
        }
        let done = self.state_label(State::Done);
        let exit = self.randsequence_exit();
        let state = self.randseq();
        let mut arms = std::mem::take(&mut state.arms);
        arms.push((done, vec![IrStmt::Goto(exit)]));
        arms.sort_by_key(|(label, _)| *label);
        let next = state.next.clone();
        let next_used = state.next_used;
        let mut loop_body = vec![index_case(local_read(&pc, 32, true), arms)?];
        if next_used {
            loop_body.push(IrStmt::Label(next));
        }
        let mut setup = vec![int_local(&pc, Some(int_expr(0, 32, true)?))];
        setup.append(&mut body);
        setup.push(IrStmt::Forever { body: loop_body });
        Ok((declarations, setup))
    }

    /// Jump to the end of the innermost randsequence statement.
    fn randsequence_exit(&mut self) -> String {
        for scope in self.ctrl.iter_mut().rev() {
            if let CtrlScope::RandSequence { exit, exit_used } = scope {
                *exit_used = true;
                return exit.clone();
            }
        }
        unreachable!("randsequence exit outside a randsequence statement")
    }

    fn state_label(&mut self, state: State) -> u32 {
        let states = &mut self.randseq().states;
        let next = u32::try_from(states.len()).expect("dispatch state count fits in u32");
        *states.entry(state).or_insert(next)
    }

    fn goto_state(&mut self, state: State) -> Result<IrStmt, String> {
        let label = self.state_label(state);
        let pc = self
            .randseq()
            .pc
            .clone()
            .expect("dispatch state outside a dispatch loop");
        Ok(assign(
            local_lhs(&pc, 32, true),
            int_expr(i64::from(label), 32, true)?,
        ))
    }

    fn add_arm(&mut self, state: State, body: Vec<IrStmt>) {
        let label = self.state_label(state);
        self.randseq().arms.push((label, body));
    }

    /// Lower the synthesized assignment `node` (a binding, reset or copy).
    fn production_assignment(&mut self, node: NodeId) -> Result<IrStmt, String> {
        let NodeKind::Expr(ExprKind::Operation { operands, .. }) = self.cg.kind(node) else {
            return Err(format!(
                "randsequence assignment at {} is malformed",
                self.production_location(node)
            ));
        };
        let (lhs, rhs) = match operands.as_slice() {
            [lhs, rhs] => (*lhs, *rhs),
            _ => {
                return Err(format!(
                    "randsequence assignment at {} is malformed",
                    self.production_location(node)
                ))
            }
        };
        self.lower_assignment_operands(lhs, rhs, true, Operation::Assignment, false)
    }

    /// Bind the input formals of the production `item` generates. Actuals
    /// are evaluated before any formal changes when a production generates
    /// itself, since they can read the formals they replace.
    fn bind_formals(&mut self, item: NodeId, from: Option<NodeId>) -> Result<Vec<IrStmt>, String> {
        let grammar = self.grammar();
        let info = &grammar.items[&item];
        let production = grammar.production_of(item);
        let mut statements = Vec::with_capacity(info.bindings.len());
        if from != Some(production) || info.bindings.len() < 2 {
            for binding in &info.bindings {
                statements.push(self.production_assignment(binding.assignment)?);
            }
            return Ok(statements);
        }
        let mut stores = Vec::with_capacity(info.bindings.len());
        for binding in &info.bindings {
            let NodeKind::Expr(ExprKind::Operation { operands, .. }) =
                self.cg.kind(binding.assignment)
            else {
                return Err("randsequence binding is malformed".to_owned());
            };
            let (lhs, rhs) = (operands[0], operands[1]);
            let name = self.new_label("rsa");
            if self.cg.is_string_expr(&self.path, lhs) {
                statements.push(IrStmt::DeclString {
                    name: name.clone(),
                    init: Some(self.cg.lower_string(&self.path, rhs)?),
                });
                stores.push(self.lower_string_format_target(lhs, IrStringExpr::LocalRead(name))?);
                continue;
            }
            if !self.frame_storable(lhs) {
                return Err(self.unsaveable(production, lhs));
            }
            let value = self.cg.lower_expr(&self.path, rhs)?;
            let target = self.cg.lower_lhs(&self.path, lhs)?;
            let (width, signed) = (value.width, value.signed);
            statements.push(IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed,
                two_state: false,
                init: Some(Box::new(value)),
            });
            stores.push(assign(
                target,
                IrExpr::new(IrExprKind::LocalRead(name), width, signed, None),
            ));
        }
        statements.extend(stores);
        // The temporaries end with the bindings.
        Ok(vec![IrStmt::Block(statements)])
    }

    /// Whether the variable `reference` reads is packed or real, so a
    /// recursive generation can save it in its packed frame.
    fn frame_storable(&self, reference: NodeId) -> bool {
        match self.cg.kind(reference) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => match self.cg.kind(*target) {
                NodeKind::Var { ty } | NodeKind::Array { ty } => {
                    is_real_kind(&ty.kind) || self.cg.fixed_value_width(*target).is_some()
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// The error for a value of recursive `production` that no frame can save.
    fn unsaveable(&self, production: NodeId, reference: NodeId) -> String {
        format!(
            "recursive randsequence production `{}` at {}: `{}` has a type llg cannot save \
             across a recursive generation (packed, real and string values are supported)",
            self.production_name(production),
            self.production_location(production),
            self.cg.node(reference).name
        )
    }

    /// Generate the inline production `item` names, in place.
    fn expand_item(&mut self, item: NodeId) -> Result<Vec<IrStmt>, String> {
        let grammar = self.grammar();
        let production = grammar.production_of(item);
        let decl = &grammar.decls[&production];
        let mut statements = self.bind_formals(item, None)?;
        if let Some(result) = decl.result {
            statements.push(self.production_assignment(result.reset)?);
        }
        let end = self.new_label("rse");
        self.randseq().returns.push(ReturnTarget::Label {
            label: end.clone(),
            used: false,
            result: decl.result.map(|result| result.reference),
        });
        let body = self.production_rules(production, false);
        let used = match self.randseq().returns.pop() {
            Some(ReturnTarget::Label { used, .. }) => used,
            _ => unreachable!("randsequence return stack imbalance"),
        };
        statements.extend(body?);
        if used {
            statements.push(IrStmt::Label(end));
        }
        if let Some(copy) = grammar.items[&item].result_copy {
            statements.push(self.production_assignment(copy)?);
        }
        Ok(vec![IrStmt::Block(statements)])
    }

    /// Rule selection, then the chosen rule; for a `segmented` production,
    /// the chosen rule's first dispatch arm.
    fn production_rules(
        &mut self,
        production: NodeId,
        segmented: bool,
    ) -> Result<Vec<IrStmt>, String> {
        let grammar = self.grammar();
        let decl = &grammar.decls[&production];
        let mut bodies = Vec::with_capacity(decl.rules.len());
        for rule in &decl.rules {
            bodies.push(if segmented {
                self.segmented_rule(production, *rule)?
            } else {
                self.inline_rule(*rule)?
            });
        }
        // A weight is only meaningful between alternatives (18.17.1), so a
        // single rule is generated without evaluating it (S16-D1).
        if bodies.len() == 1 {
            return Ok(bodies.pop().expect("one rule"));
        }
        let weights = decl
            .rules
            .iter()
            .map(|rule| grammar.rules[rule].weight)
            .collect::<Vec<_>>();
        let origin = self.cg.origin(production);
        let selection = self.lower_weighted_selection(
            &weights,
            origin,
            true,
            &format!(
                "randsequence production `{}`",
                self.production_name(production)
            ),
        )?;
        let name = self.new_label("rsr");
        let mut statements = vec![int_local(&name, Some(selection))];
        if segmented {
            // No rule (all weights zero or an invalid weight) ends it.
            statements.push(self.goto_state(State::Exit(production))?);
        }
        let arms = bodies
            .into_iter()
            .enumerate()
            .map(|(index, body)| (index as u32, body))
            .collect();
        statements.push(index_case(local_read(&name, 32, true), arms)?);
        Ok(statements)
    }

    fn rule_resets(&mut self, rule: &ProductionRule) -> Result<Vec<IrStmt>, String> {
        rule.resets
            .iter()
            .map(|reset| self.production_assignment(*reset))
            .collect()
    }

    fn inline_rule(&mut self, rule_node: NodeId) -> Result<Vec<IrStmt>, String> {
        let grammar = self.grammar();
        let rule = &grammar.rules[&rule_node];
        let mut statements = self.rule_resets(rule)?;
        if rule.rand_join.is_some() {
            statements.extend(self.lower_rand_join(rule_node)?);
        } else {
            for element in &rule.elements {
                statements.extend(self.inline_element(*element)?);
            }
        }
        if let Some(code) = rule.weight_code {
            statements.extend(self.code_block(code)?);
        }
        Ok(statements)
    }

    fn code_block(&mut self, code: NodeId) -> Result<Vec<IrStmt>, String> {
        match self.cg.kind(code) {
            NodeKind::Production(ProductionKind::CodeBlock { body }) => {
                let body = *body;
                self.lower_stmt(body)
            }
            _ => Err(format!(
                "randsequence code block at {} is malformed",
                self.production_location(code)
            )),
        }
    }

    /// One element of an inline production's rule.
    fn inline_element(&mut self, element: NodeId) -> Result<Vec<IrStmt>, String> {
        let kind = match self.cg.kind(element) {
            NodeKind::Production(kind) => kind.clone(),
            other => {
                return Err(format!(
                    "randsequence element at {} is malformed ({other:?})",
                    self.production_location(element)
                ))
            }
        };
        match kind {
            ProductionKind::CodeBlock { .. } => self.code_block(element),
            ProductionKind::Item(_) => self.expand_item(element),
            ProductionKind::If {
                condition,
                then_item,
                else_item,
            } => {
                let cond = self.cg.lower_boolean_expr(&self.path, condition)?;
                let then_ = self.expand_item(then_item)?;
                let els = else_item.map(|item| self.expand_item(item)).transpose()?;
                Ok(vec![if_then(cond, then_, els)])
            }
            ProductionKind::Repeat { count, item } => {
                let (mut statements, count) = self.repeat_count(element, count)?;
                let body = self.expand_item(item)?;
                statements.push(IrStmt::Repeat { count, body });
                Ok(statements)
            }
            ProductionKind::Case {
                selector,
                items,
                default,
            } => {
                let mut bodies = Vec::with_capacity(items.len() + 1);
                for item in &items {
                    bodies.push(self.expand_item(item.item)?);
                }
                if let Some(default) = default {
                    bodies.push(self.expand_item(default)?);
                }
                self.production_case(selector, &items, default.is_some(), bodies)
            }
            ProductionKind::Declaration(_) | ProductionKind::Rule(_) => Err(format!(
                "randsequence element at {} is malformed",
                self.production_location(element)
            )),
        }
    }

    /// A case production: an ordinary `case` (18.17.3) whose item bodies
    /// are the generations already lowered, in item order then the default.
    fn production_case(
        &mut self,
        selector: NodeId,
        items: &[crate::core::db::ProductionCaseItem],
        has_default: bool,
        bodies: Vec<Vec<IrStmt>>,
    ) -> Result<Vec<IrStmt>, String> {
        let mut case_items = items
            .iter()
            .map(|item| CaseItem {
                exprs: item.exprs.clone(),
                body: None,
            })
            .collect::<Vec<_>>();
        if has_default {
            case_items.push(CaseItem {
                exprs: Vec::new(),
                body: None,
            });
        }
        self.lower_case_items(
            DbCaseKind::Exact,
            IrUniquePriorityCheck::None,
            selector,
            &case_items,
            Some(bodies.into_iter()),
        )
    }

    /// The count of a repeat production, evaluated once (18.17.4). A
    /// negative or unknown count is a run-time error and generates nothing,
    /// like a procedural `repeat` of it (S16-D4).
    fn repeat_count(
        &mut self,
        element: NodeId,
        count: NodeId,
    ) -> Result<(Vec<IrStmt>, IrExpr), String> {
        let value = self.cg.lower_expr(&self.path, count)?;
        if value.is_real() || value.width == 0 {
            return Err(format!(
                "randsequence repeat at {}: the count must be integral",
                self.production_location(element)
            ));
        }
        let (width, signed) = (value.width, value.signed);
        let name = self.new_label("rsc");
        let read = local_read(&name, width, signed);
        let mut invalid = IrExpr::new(
            IrExprKind::SysFunc(Box::new(IrSysFunc::BitQuery {
                kind: crate::sim::ir::IrBitQuery::IsUnknown,
                arg: Box::new(read.clone()),
            })),
            1,
            false,
            None,
        );
        if signed {
            let negative = cmp_expr_ir(IrBinOp::Lt, read.clone(), int_expr(0, width, true)?);
            invalid = cmp_expr_ir(IrBinOp::LogOr, invalid, negative);
        }
        let error = self.runtime_error(
            element,
            "randsequence repeat count is negative or unknown; the production is not \
             generated",
        );
        Ok((
            vec![
                IrStmt::DeclLocal {
                    name,
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(value)),
                },
                if_then(invalid, vec![error], None),
            ],
            read,
        ))
    }

    /// A non-fatal run-time error located at `node` that fails the run.
    fn runtime_error(&self, node: NodeId, message: &str) -> IrStmt {
        IrStmt::Severity {
            level: crate::sim::ir::IrSeverityLevel::Error,
            fmt: c_string_literal(message),
            args: Vec::new(),
            scope: self.path.clone(),
            location: self.production_location(node),
            fatal_finish_number: None,
            runtime_failure: true,
            skip_finals: false,
        }
    }

    /// `rand join [(ratio)] S1 S2 ...` (18.17.5). Each item's production has
    /// one rule (`check_joins`); its production list is one sequence (a rand
    /// join rule is one element). Each step advances one sequence chosen by
    /// `llg_randsequence_join_pick` (S16-D5, S16-D6).
    fn lower_rand_join(&mut self, rule_node: NodeId) -> Result<Vec<IrStmt>, String> {
        let grammar = self.grammar();
        let rule = &grammar.rules[&rule_node];
        let mut statements = Vec::new();
        // The ratio is evaluated once, before the sequences are chosen.
        let ratio_name = self.new_label("rjr");
        let ratio = match rule.rand_join.flatten() {
            Some(expression) => {
                let value = self.cg.lower_expr(&self.path, expression)?;
                if value.is_real() {
                    value
                } else {
                    // Real expressions are signed, as the folded constant is.
                    IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(value),
                            shortreal: false,
                        },
                        0,
                        true,
                        None,
                    )
                }
            }
            None => IrExpr::new(IrExprKind::Const(IrConst::real(0.5)), 0, false, None),
        };
        statements.push(IrStmt::DeclLocal {
            name: ratio_name.clone(),
            width: 0,
            signed: false,
            two_state: false,
            init: Some(Box::new(ratio)),
        });
        let ratio_read = local_read(&ratio_name, 0, false);
        let real =
            |value: f64| IrExpr::new(IrExprKind::Const(IrConst::real(value)), 0, false, None);
        if rule.rand_join.flatten().is_some() {
            let in_range = cmp_expr_ir(
                IrBinOp::LogAnd,
                cmp_expr_ir(IrBinOp::Ge, ratio_read.clone(), real(0.0)),
                cmp_expr_ir(IrBinOp::Le, ratio_read.clone(), real(1.0)),
            );
            statements.push(if_then(
                in_range,
                Vec::new(),
                Some(vec![
                    self.runtime_error(
                        rule_node,
                        "rand join expression is not a real number in 0.0 to 1.0; 0.5 is used",
                    ),
                    assign(
                        IrLhs::WholeRef {
                            addr: format!("&{ratio_name}"),
                            width: 0,
                            signed: false,
                            two_state: false,
                            shortreal: false,
                        },
                        real(0.5),
                    ),
                ]),
            ));
        }
        struct Sequence {
            item: NodeId,
            production: NodeId,
            /// The production's one rule (`check_joins`).
            rule: NodeId,
            length: String,
            position: String,
            /// The rule's sequence steps.
            elements: Vec<JoinStep>,
        }
        let mut sequences = Vec::with_capacity(rule.elements.len());
        for item in &rule.elements {
            let production = grammar.production_of(*item);
            let decl = &grammar.decls[&production];
            let [chosen_rule] = decl.rules[..] else {
                return Err(format!(
                    "rand join at {}: production `{}` must have one rule",
                    self.production_location(*item),
                    self.production_name(production)
                ));
            };
            statements.extend(self.bind_formals(*item, None)?);
            if let Some(result) = decl.result {
                statements.push(self.production_assignment(result.reset)?);
            }
            let length = self.new_label("rjl");
            let position = self.new_label("rjp");
            let info = &grammar.rules[&chosen_rule];
            let mut elements = if info.rand_join.is_some() {
                vec![JoinStep::Join]
            } else {
                info.elements
                    .iter()
                    .copied()
                    .map(JoinStep::Element)
                    .collect::<Vec<_>>()
            };
            // The weight code block is the rule's last element (S16-D2) and
            // one step of the interleaving of its own (S16-D5).
            elements.extend(info.weight_code.map(JoinStep::Weight));
            statements.extend(self.rule_resets(info)?);
            statements.push(IrStmt::DeclLocal {
                name: length.clone(),
                width: 32,
                signed: false,
                two_state: true,
                init: Some(Box::new(int_expr(elements.len() as i64, 32, false)?)),
            });
            statements.push(IrStmt::DeclLocal {
                name: position.clone(),
                width: 32,
                signed: false,
                two_state: true,
                init: Some(Box::new(int_expr(0, 32, false)?)),
            });
            sequences.push(Sequence {
                item: *item,
                production,
                rule: chosen_rule,
                length,
                position,
                elements,
            });
        }
        let end = self.new_label("rje");
        let pick = self.new_label("rjk");
        let mut steps = Vec::with_capacity(sequences.len());
        for (index, sequence) in sequences.iter().enumerate() {
            let decl = &grammar.decls[&sequence.production];
            let abort = self.new_label("rja");
            self.randseq().returns.push(ReturnTarget::Label {
                label: abort.clone(),
                used: false,
                result: decl.result.map(|result| result.reference),
            });
            let lowered = (|| -> Result<Vec<IrStmt>, String> {
                let mut element_arms = Vec::with_capacity(sequence.elements.len());
                for (position, element) in sequence.elements.iter().enumerate() {
                    let body = match element {
                        JoinStep::Element(element) => self.inline_element(*element)?,
                        JoinStep::Join => self.lower_rand_join(sequence.rule)?,
                        JoinStep::Weight(code) => self.code_block(*code)?,
                    };
                    element_arms.push((position as u32, body));
                }
                let mut step = vec![index_case(
                    local_read(&sequence.position, 32, false),
                    element_arms,
                )?];
                step.push(assign(
                    local_lhs(&sequence.position, 32, false),
                    bin_expr(
                        IrBinOp::Add,
                        local_read(&sequence.position, 32, false),
                        int_expr(1, 32, false)?,
                    ),
                ));
                Ok(step)
            })();
            let used = match self.randseq().returns.pop() {
                Some(ReturnTarget::Label { used, .. }) => used,
                _ => unreachable!("randsequence return stack imbalance"),
            };
            let mut step = lowered?;
            if used {
                // `return` aborts this production: its remaining elements are
                // dropped from the interleaving (18.17.6).
                let skip = self.new_label("rjn");
                step.push(IrStmt::Goto(skip.clone()));
                step.push(IrStmt::Label(abort));
                step.push(assign(
                    local_lhs(&sequence.position, 32, false),
                    local_read(&sequence.length, 32, false),
                ));
                step.push(IrStmt::Label(skip));
            }
            steps.push((index as u32, vec![IrStmt::Block(step)]));
        }
        let lengths = sequences
            .iter()
            .map(|sequence| {
                bin_expr(
                    IrBinOp::Sub,
                    local_read(&sequence.length, 32, false),
                    local_read(&sequence.position, 32, false),
                )
            })
            .collect::<Vec<_>>();
        self.cg.model.random.threads = true;
        let mut loop_body = vec![int_local(
            &pick,
            Some(IrExpr::new(
                IrExprKind::SysFunc(Box::new(IrSysFunc::RandJoinPick {
                    lengths,
                    ratio: Box::new(ratio_read),
                })),
                32,
                true,
                None,
            )),
        )];
        loop_body.push(if_then(
            cmp_expr_ir(
                IrBinOp::Lt,
                local_read(&pick, 32, true),
                int_expr(0, 32, true)?,
            ),
            vec![IrStmt::Goto(end.clone())],
            None,
        ));
        loop_body.push(index_case(local_read(&pick, 32, true), steps)?);
        statements.push(IrStmt::Forever { body: loop_body });
        statements.push(IrStmt::Label(end));
        // Each interleaved production's value reaches the rule's implicit
        // variable when the interleaving ends; only the rule's weight code
        // block, which runs after it, can read it.
        for sequence in &sequences {
            if let Some(copy) = grammar.items[&sequence.item].result_copy {
                statements.push(self.production_assignment(copy)?);
            }
        }
        Ok(statements)
    }

    /// `return [value];` in a code block (18.17.6, 18.17.7), or `None` when
    /// no randsequence code block encloses it.
    pub(super) fn lower_randsequence_return(
        &mut self,
        value: Option<NodeId>,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        let Some(state) = self.randseq.last() else {
            return Ok(None);
        };
        let (result, jump) = match state.returns.last() {
            None => return Ok(None),
            Some(ReturnTarget::Label { label, result, .. }) => {
                (*result, vec![IrStmt::Goto(label.clone())])
            }
            Some(ReturnTarget::Exit { production, result }) => {
                let (production, result) = (*production, *result);
                let next = state.next.clone();
                let exit = self.goto_state(State::Exit(production))?;
                self.randseq().next_used = true;
                (result, vec![exit, IrStmt::Goto(next)])
            }
        };
        if let Some(ReturnTarget::Label { used, .. }) = self.randseq().returns.last_mut() {
            *used = true;
        }
        let mut statements = Vec::new();
        if let (Some(value), Some(result)) = (value, result) {
            statements.push(self.lower_assignment_operands(
                result,
                value,
                true,
                Operation::Assignment,
                false,
            )?);
        }
        statements.extend(jump);
        Ok(Some(vec![IrStmt::Block(statements)]))
    }

    /// A hidden zero-initialized two-state local declared at the start of
    /// the statement.
    fn hidden_local(&mut self, name: &str, width: u32, signed: bool) -> Result<(), String> {
        let init = int_expr(0, width, signed)?;
        self.randseq().locals.push(IrStmt::DeclLocal {
            name: name.to_owned(),
            width,
            signed,
            two_state: true,
            init: Some(Box::new(init)),
        });
        Ok(())
    }

    /// The return-state variable of a segmented production generated from
    /// more than one place; a single-site production returns to a constant
    /// state.
    fn return_state(&mut self, production: NodeId) -> Result<Option<String>, String> {
        if self.grammar().sites.get(&production).copied().unwrap_or(0) < 2 {
            return Ok(None);
        }
        if let Some(name) = self.randseq().returns_to.get(&production) {
            return Ok(Some(name.clone()));
        }
        let name = self.new_label("rsret");
        self.hidden_local(&name, 32, true)?;
        self.randseq().returns_to.insert(production, name.clone());
        Ok(Some(name))
    }

    /// Generate the segmented production `item` names from a dispatch arm:
    /// save a recursive production's frame, bind the formals, record where
    /// to continue and enter the production.
    fn call_segmented(&mut self, item: NodeId, continuation: State) -> Result<Vec<IrStmt>, String> {
        let grammar = self.grammar();
        let production = grammar.production_of(item);
        let caller = self.randseq_current_production();
        let return_state = self.return_state(production)?;
        let mut statements = Vec::new();
        if grammar.recursive.contains(&production) {
            statements.extend(self.push_frame(production, item)?);
        }
        statements.extend(self.bind_formals(item, caller)?);
        let label = self.state_label(continuation);
        match return_state {
            Some(name) => statements.push(assign(
                local_lhs(&name, 32, true),
                int_expr(i64::from(label), 32, true)?,
            )),
            None => {
                self.randseq().continuations.insert(production, label);
            }
        }
        statements.push(self.goto_state(State::Entry(production))?);
        Ok(statements)
    }

    /// The segmented production whose code is being lowered, if any.
    fn randseq_current_production(&self) -> Option<NodeId> {
        self.randseq
            .last()
            .and_then(|state| state.returns.last())
            .and_then(|target| match target {
                ReturnTarget::Exit { production, .. } => Some(*production),
                ReturnTarget::Label { .. } => None,
            })
    }

    /// The entry and element arms of segmented `production`.
    fn lower_segmented_production(&mut self, production: NodeId) -> Result<(), String> {
        let grammar = self.grammar();
        let decl = &grammar.decls[&production];
        self.randseq().returns.push(ReturnTarget::Exit {
            production,
            result: decl.result.map(|result| result.reference),
        });
        let lowered = (|| -> Result<Vec<IrStmt>, String> {
            let mut entry = Vec::new();
            if let Some(result) = decl.result {
                entry.push(self.production_assignment(result.reset)?);
            }
            entry.extend(self.production_rules(production, true)?);
            Ok(entry)
        })();
        self.randseq().returns.pop();
        let entry = lowered?;
        self.add_arm(State::Entry(production), entry);
        Ok(())
    }

    /// The exit arm of segmented `production`: restore a recursive
    /// production's frame and dispatch to the generation's continuation.
    fn segmented_exit(&mut self, production: NodeId) -> Result<(), String> {
        let pc = self.randseq().pc.clone().expect("dispatch loop");
        let mut exit = Vec::new();
        let target = match self.randseq().returns_to.get(&production).cloned() {
            Some(name) => {
                let saved = self.new_label("rsto");
                exit.push(int_local(&saved, Some(local_read(&name, 32, true))));
                local_read(&saved, 32, true)
            }
            None => {
                let label = self
                    .randseq()
                    .continuations
                    .get(&production)
                    .copied()
                    .ok_or_else(|| {
                        format!(
                            "randsequence production `{}` has no continuation",
                            self.production_name(production)
                        )
                    })?;
                int_expr(i64::from(label), 32, true)?
            }
        };
        if self.grammar().recursive.contains(&production) {
            exit.extend(self.pop_frame(production)?);
        }
        exit.push(assign(local_lhs(&pc, 32, true), target));
        self.add_arm(State::Exit(production), exit);
        Ok(())
    }

    /// One rule of a segmented production: its first arm's statements (for
    /// the entry arm's rule case); later arms are added as states.
    fn segmented_rule(
        &mut self,
        production: NodeId,
        rule_node: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let grammar = self.grammar();
        let rule = &grammar.rules[&rule_node];
        let mut arm = self.rule_resets(rule)?;
        let mut first = None;
        let mut open = None;
        if rule.rand_join.is_some() {
            arm.extend(self.lower_rand_join(rule_node)?);
        } else {
            for element in &rule.elements {
                let Some(after) = self.segmented_element(*element, &mut arm)? else {
                    continue;
                };
                let finished = std::mem::take(&mut arm);
                match open.replace(after) {
                    None => first = Some(finished),
                    Some(state) => self.add_arm(state, finished),
                }
                if let Some(copy) = grammar.items.get(element).and_then(|item| item.result_copy) {
                    arm.push(self.production_assignment(copy)?);
                }
            }
        }
        if let Some(code) = rule.weight_code {
            arm.extend(self.code_block(code)?);
        }
        arm.push(self.goto_state(State::Exit(production))?);
        Ok(match open {
            None => arm,
            Some(state) => {
                self.add_arm(state, arm);
                first.expect("first arm of a split rule")
            }
        })
    }

    /// Lower `element` into the open arm `arm`. Returns the state the rest
    /// of the rule continues in when the element generates a segmented
    /// production (`arm` then ends by setting `pc`).
    fn segmented_element(
        &mut self,
        element: NodeId,
        arm: &mut Vec<IrStmt>,
    ) -> Result<Option<State>, String> {
        let grammar = self.grammar();
        let kind = match self.cg.kind(element) {
            NodeKind::Production(kind) => kind.clone(),
            _ => {
                return Err(format!(
                    "randsequence element at {} is malformed",
                    self.production_location(element)
                ))
            }
        };
        let mut items = Vec::new();
        match &kind {
            ProductionKind::Item(_) => items.push(element),
            other => element_items(other, &mut items),
        }
        if !items.iter().any(|item| grammar.segmented_item(*item)) {
            arm.extend(self.inline_element(element)?);
            return Ok(None);
        }
        let after = State::After(element);
        match kind {
            ProductionKind::Item(_) => {
                arm.extend(self.call_segmented(element, after)?);
            }
            ProductionKind::If {
                condition,
                then_item,
                else_item,
            } => {
                let cond = self.cg.lower_boolean_expr(&self.path, condition)?;
                arm.push(self.goto_state(after)?);
                let then_ = self.branch_generation(then_item, after)?;
                let els = else_item
                    .map(|item| self.branch_generation(item, after))
                    .transpose()?;
                arm.push(if_then(cond, then_, els));
            }
            ProductionKind::Case {
                selector,
                items,
                default,
            } => {
                arm.push(self.goto_state(after)?);
                let mut bodies = Vec::with_capacity(items.len() + 1);
                for item in &items {
                    bodies.push(self.branch_generation(item.item, after)?);
                }
                if let Some(default) = default {
                    bodies.push(self.branch_generation(default, after)?);
                }
                arm.extend(self.production_case(selector, &items, default.is_some(), bodies)?);
            }
            ProductionKind::Repeat { count, item } => {
                let (check, value) = self.repeat_count(element, count)?;
                let (counter, width, signed) = self.repeat_counter(element, &value)?;
                let counter_lhs = IrLhs::WholeRef {
                    addr: format!("&{counter}"),
                    width,
                    signed,
                    two_state: false,
                    shortreal: false,
                };
                arm.extend(check);
                arm.push(assign(counter_lhs.clone(), value));
                let test = State::Loop(element);
                arm.push(self.goto_state(test)?);
                let counter_read = local_read(&counter, width, signed);
                let mut generate = vec![assign(
                    counter_lhs,
                    bin_expr(
                        IrBinOp::Sub,
                        counter_read.clone(),
                        int_expr(1, width, signed)?,
                    ),
                )];
                generate.extend(self.branch_generation(item, test)?);
                let loop_arm = vec![
                    self.goto_state(after)?,
                    if_then(
                        cmp_expr_ir(IrBinOp::Gt, counter_read, int_expr(0, width, signed)?),
                        generate,
                        None,
                    ),
                ];
                self.add_arm(test, loop_arm);
            }
            ProductionKind::CodeBlock { .. }
            | ProductionKind::Declaration(_)
            | ProductionKind::Rule(_) => {
                return Err(format!(
                    "randsequence element at {} is malformed",
                    self.production_location(element)
                ))
            }
        }
        Ok(Some(after))
    }

    /// The counter of a repeat element whose item is segmented. It belongs
    /// to its production's frame, so a recursive generation inside the loop
    /// cannot disturb it; it is created once per element.
    fn repeat_counter(
        &mut self,
        element: NodeId,
        value: &IrExpr,
    ) -> Result<(String, u32, bool), String> {
        if let Some(counter) = self.randseq().counters.get(&element) {
            return Ok(counter.clone());
        }
        let counter = self.new_label("rsk");
        let (width, signed) = (value.width, value.signed);
        self.randseq().locals.push(IrStmt::DeclLocal {
            name: counter.clone(),
            width,
            signed,
            two_state: false,
            init: None,
        });
        self.randseq()
            .counters
            .insert(element, (counter.clone(), width, signed));
        Ok((counter, width, signed))
    }

    /// One branch's generation in a segmented rule whose `pc` already names
    /// `after`: an inline expansion, or a call whose landing arm copies the
    /// result and continues at `after`.
    fn branch_generation(&mut self, item: NodeId, after: State) -> Result<Vec<IrStmt>, String> {
        if !self.grammar().segmented_item(item) {
            return self.expand_item(item);
        }
        let landing = State::After(item);
        let statements = self.call_segmented(item, landing)?;
        let mut arm = Vec::new();
        if let Some(copy) = self.grammar().items[&item].result_copy {
            arm.push(self.production_assignment(copy)?);
        }
        arm.push(self.goto_state(after)?);
        self.add_arm(landing, arm);
        Ok(statements)
    }

    /// The saved values of recursive `production`: its formals, its rules'
    /// implicit variables, its return state and its repeat counters.
    fn frame(&mut self, production: NodeId) -> Result<(), String> {
        if self.randseq().frames.contains_key(&production) {
            return Ok(());
        }
        let grammar = self.grammar();
        let decl = &grammar.decls[&production];
        let name = self.production_name(production);
        let location = self.production_location(production);
        let mut references = decl.formal_refs.clone();
        for rule in &decl.rules {
            let rule = &grammar.rules[rule];
            for reset in &rule.resets {
                if let NodeKind::Expr(ExprKind::Operation { operands, .. }) = self.cg.kind(*reset) {
                    references.push(operands[0]);
                }
            }
            for element in &rule.elements {
                if let NodeKind::Production(ProductionKind::Repeat { count, item }) =
                    self.cg.kind(*element)
                {
                    let (count, item) = (*count, *item);
                    if grammar.segmented_item(item) {
                        let value = self.cg.lower_expr(&self.path, count)?;
                        self.repeat_counter(*element, &value)?;
                    }
                }
            }
        }
        let mut entries = Vec::new();
        let mut strings = Vec::new();
        for reference in references {
            if self.cg.is_string_expr(&self.path, reference) {
                strings.push(reference);
                continue;
            }
            let unsupported = self.unsaveable(production, reference);
            if !self.frame_storable(reference) {
                return Err(unsupported);
            }
            let read = self
                .cg
                .lower_expr(&self.path, reference)
                .map_err(|error| format!("{unsupported}: {error}"))?;
            if read.width == 0 && !read.is_real() {
                return Err(unsupported);
            }
            let write = self.cg.lower_lhs(&self.path, reference)?;
            let real = read.is_real();
            let width = if real { 64 } else { read.width };
            entries.push(FrameEntry {
                read,
                write,
                real,
                width,
            });
        }
        if let Some(state) = self.randseq().returns_to.get(&production).cloned() {
            entries.push(FrameEntry {
                read: local_read(&state, 32, true),
                write: local_lhs(&state, 32, true),
                real: false,
                width: 32,
            });
        }
        let mut counters = self
            .randseq()
            .counters
            .iter()
            .map(|(element, counter)| (*element, counter.clone()))
            .collect::<Vec<_>>();
        counters.sort_by_key(|(element, _)| element.index());
        for (element, (counter, width, signed)) in counters {
            if self
                .cg
                .node(element)
                .parent
                .and_then(|rule| self.cg.node(rule).parent)
                != Some(production)
            {
                continue;
            }
            entries.push(FrameEntry {
                read: local_read(&counter, width, signed),
                write: IrLhs::WholeRef {
                    addr: format!("&{counter}"),
                    width,
                    signed,
                    two_state: false,
                    shortreal: false,
                },
                real: false,
                width,
            });
        }
        let width = entries
            .iter()
            .try_fold(0u32, |total, entry| total.checked_add(entry.width))
            .filter(|width| *width < crate::sim::emit_c::LLG_WIDTH_LIMIT)
            .ok_or_else(|| {
                format!(
                    "recursive randsequence production `{name}` at {location}: its saved \
                     values are wider than the packed-value limit"
                )
            })?;
        let container = self.cg.model.containers.len();
        self.cg.model.containers.push(IrContainer {
            c_name: format!("S_llg_container_{container}"),
            element: IrContainerElement::Packed {
                width: width.max(1),
                signed: false,
                two_state: false,
            },
            kind: IrContainerKind::Queue {
                maximum_elements: None,
            },
            initial_size: None,
            activation: true,
            class_field: None,
            receiver: None,
        });
        self.randseq()
            .locals
            .push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                container,
            ))));
        let strings = if strings.is_empty() {
            None
        } else {
            let container = self.cg.model.containers.len();
            self.cg.model.containers.push(IrContainer {
                c_name: format!("S_llg_container_{container}"),
                element: IrContainerElement::String,
                kind: IrContainerKind::Queue {
                    maximum_elements: None,
                },
                initial_size: None,
                activation: true,
                class_field: None,
                receiver: None,
            });
            self.randseq()
                .locals
                .push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                    container,
                ))));
            Some(StringFrame {
                container,
                references: strings,
            })
        };
        self.randseq().frames.insert(
            production,
            Frame {
                container,
                width: width.max(1),
                entries,
                strings,
            },
        );
        Ok(())
    }

    fn depth_local(&mut self) -> Result<String, String> {
        if let Some(depth) = self.randseq().depth.clone() {
            return Ok(depth);
        }
        let depth = self.new_label("rsd");
        self.hidden_local(&depth, 32, true)?;
        self.randseq().depth = Some(depth.clone());
        Ok(depth)
    }

    /// Count one more active recursive generation (failing past the limit)
    /// and save `production`'s current values.
    fn push_frame(&mut self, production: NodeId, item: NodeId) -> Result<Vec<IrStmt>, String> {
        self.frame(production)?;
        let depth = self.depth_local()?;
        let frame = &self.randseq.last().expect("randsequence state").frames[&production];
        let mut parts = Vec::with_capacity(frame.entries.len());
        for entry in &frame.entries {
            parts.push(if entry.real {
                IrExpr::new(
                    IrExprKind::SysFunc(Box::new(IrSysFunc::RealToBits(Box::new(
                        entry.read.clone(),
                    )))),
                    64,
                    false,
                    None,
                )
            } else {
                entry.read.clone()
            });
        }
        let (container, width) = (frame.container, frame.width);
        let strings = frame
            .strings
            .as_ref()
            .map(|strings| (strings.container, strings.references.clone()));
        let value = if parts.is_empty() {
            int_expr(0, 1, false)?
        } else {
            IrExpr::new(IrExprKind::Concat { parts }, width, false, None)
        };
        let message = format!(
            "randsequence recursion exceeds the depth limit of {RANDSEQUENCE_DEPTH_LIMIT} \
             active generations"
        );
        let fatal = IrStmt::Severity {
            level: crate::sim::ir::IrSeverityLevel::Fatal,
            fmt: c_string_literal(&message),
            args: Vec::new(),
            scope: self.path.clone(),
            location: self.production_location(item),
            fatal_finish_number: Some(0),
            runtime_failure: true,
            skip_finals: false,
        };
        let mut saves = vec![IrStmt::Container(Box::new(
            IrContainerStmt::QueuePushBack { container, value },
        ))];
        if let Some((strings, references)) = strings {
            for reference in references {
                saves.push(IrStmt::Container(Box::new(
                    IrContainerStmt::QueuePushBackString {
                        container: strings,
                        value: self.cg.lower_string(&self.path, reference)?,
                    },
                )));
            }
        }
        let mut statements = vec![
            assign(
                local_lhs(&depth, 32, true),
                bin_expr(
                    IrBinOp::Add,
                    local_read(&depth, 32, true),
                    int_expr(1, 32, true)?,
                ),
            ),
            if_then(
                cmp_expr_ir(
                    IrBinOp::Gt,
                    local_read(&depth, 32, true),
                    int_expr(i64::from(RANDSEQUENCE_DEPTH_LIMIT), 32, true)?,
                ),
                vec![fatal],
                None,
            ),
        ];
        statements.extend(saves);
        Ok(statements)
    }

    /// Restore `production`'s values saved by the matching `push_frame`.
    fn pop_frame(&mut self, production: NodeId) -> Result<Vec<IrStmt>, String> {
        self.frame(production)?;
        let depth = self.depth_local()?;
        let saved = self.new_label("rsf");
        let frame = &self.randseq.last().expect("randsequence state").frames[&production];
        let strings = frame
            .strings
            .as_ref()
            .map(|strings| (strings.container, strings.references.clone()));
        let mut statements = vec![IrStmt::DeclLocal {
            name: saved.clone(),
            width: frame.width,
            signed: false,
            two_state: false,
            init: Some(Box::new(IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::QueuePopBack(frame.container))),
                frame.width,
                false,
                None,
            ))),
        }];
        let mut offset = frame.width;
        for entry in &frame.entries {
            offset -= entry.width;
            let bits = IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(local_read(&saved, frame.width, false)),
                    left: i64::from(offset + entry.width - 1),
                    right: i64::from(offset),
                },
                entry.width,
                false,
                None,
            );
            let value = if entry.real {
                IrExpr::new(
                    IrExprKind::SysFunc(Box::new(IrSysFunc::BitsToReal(Box::new(bits)))),
                    0,
                    true,
                    None,
                )
            } else {
                bits
            };
            statements.push(assign(entry.write.clone(), value));
        }
        if let Some((strings, references)) = strings {
            for reference in references.into_iter().rev() {
                statements.push(self.lower_string_format_target(
                    reference,
                    IrStringExpr::QueuePop {
                        container: strings,
                        back: true,
                    },
                )?);
            }
        }
        statements.push(assign(
            local_lhs(&depth, 32, true),
            bin_expr(
                IrBinOp::Sub,
                local_read(&depth, 32, true),
                int_expr(1, 32, true)?,
            ),
        ));
        Ok(statements)
    }
}

/// A C string literal for a runtime diagnostic format (no conversions).
fn c_string_literal(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for character in text.chars() {
        match character {
            '"' | '\\' => {
                literal.push('\\');
                literal.push(character);
            }
            '%' => literal.push_str("%%"),
            _ => literal.push(character),
        }
    }
    literal.push('"');
    literal
}
