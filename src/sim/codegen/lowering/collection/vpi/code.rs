//! Expression and statement objects of the VPI catalog (IEEE 1800-2009
//! 37.54-37.74).
//!
//! Built from the owned database, only for models generated for VPI
//! applications (`CodegenOptions::vpi`): each object becomes one compact
//! [`IrVpiCode`] entry, and the children of an entry are contiguous so the
//! runtime walks them without allocation. Implicit conversions stay in the
//! table as hidden entries: the runtime skips them when it hands out
//! handles and applies them when it evaluates an expression, so values
//! follow the frontend's typed tree exactly.

use super::*;
use crate::core::db::{CaseItem, CaseKind, ConstraintKind, DistItem, UniquePriorityCheck};
use crate::sim::ir::IrVpiCode;

mod assertions;
use assertions::ClockSource;

/// What one code entry is built from.
enum Item<'a> {
    Expr(NodeId),
    Stmt(NodeId),
    /// The unnamed begin that holds the statements of a task or function
    /// body without a block of its own (37.12 detail 5).
    Body(NodeId, Vec<NodeId>),
    /// An expression used as a statement (`i++;`, a call, a for-loop
    /// increment).
    ExprStmt(NodeId),
    /// The initialization `v = init` of a variable declared by a for loop.
    DeclInit(NodeId),
    /// A declaration named where the source declares it (a loop variable).
    Decl(NodeId),
    /// The catalog object a call, disable or foreach names.
    Alias(usize, NodeId),
    /// The event expression of an event control: several events join in
    /// one `vpiEventOrOp`.
    Events(&'a [EventSpec], NodeId),
    Event(&'a EventSpec, NodeId),
    Branch(&'a CaseItem, NodeId),
    /// An intra-assignment timing control (37.59).
    Control(&'a IntraControl, NodeId),
    /// An omitted argument (37.38 detail 8).
    NullOp(NodeId),
    /// The constant `1` an increment or decrement adds or subtracts, at
    /// the position of the operation.
    One(NodeId),
    /// A `for` loop whose variables are declared by the enclosing implicit
    /// block: the loop and the declarations.
    ForWithLocals(NodeId, Vec<NodeId>),
    /// The entry of an assertion statement (37.46, 37.51).
    Assertion(NodeId),
    /// The `vpiProperty` of concurrent assertion statement `.1` whose
    /// property is `.0`.
    Property(NodeId, NodeId),
    /// A sequence or property expression.
    AssertionExpr(NodeId),
    /// The actual clocking event of an assertion.
    Clock(ClockSource<'a>),
    /// An event the frontend does not hold as written (a procedure's
    /// inferred clock).
    OwnedEvent(EventSpec, NodeId),
    /// A sequence or property declaration (37.47, 37.49).
    Declaration(NodeId),
    /// A formal of a declaration, with its object type.
    AssertionFormal(NodeId, &'static str),
    /// A relation to the root entry it names (an instance's declaration).
    Link(u32),
    /// The hidden entry whose children are the clocking event and default
    /// skews of clocking block `.0`, or the skews of its io declaration
    /// `.1` (37.44).
    Clocking(NodeId, Option<NodeId>),
    /// The clocking event of clocking block `.0`.
    ClockingEvent(NodeId),
    /// A clocking skew as a delay control, at `.1` when it is implicit.
    Skew(&'a ClockingSkew, NodeId),
    /// The hidden root of a constraint declaration (37.30) whose children
    /// are the constraint items of body `.0`.
    Constraint(NodeId),
    /// One constraint item or constraint expression (37.30, 37.34).
    ConstraintPart(NodeId),
    /// One item of the `dist` expression `.1` (37.30).
    Weighted(DistItem, NodeId),
}

type Children<'a> = Vec<(&'static str, Item<'a>)>;

/// Expression and statement tables under construction.
pub(super) struct CodeTables<'c, 'a> {
    cg: &'c Codegen<'a>,
    /// The catalog's typespecs, which cast target types join.
    typespecs: &'c mut TypespecTable,
    /// Catalog row of each full name.
    rows: HashMap<&'c str, usize>,
    /// Catalog row of each declaration, memoized.
    decl_rows: std::cell::RefCell<HashMap<NodeId, Option<usize>>>,
    /// Names of the system subroutines dispatched to VPI applications.
    user_calls: HashSet<&'c str>,
    pub(super) code: Vec<IrVpiCode>,
    pub(super) files: Vec<String>,
    file_index: HashMap<String, u32>,
    /// `(row, entry)`: the statement object of a named block.
    pub(super) named_blocks: Vec<(usize, u32)>,
    /// The catalog row of the named block the last built statement stands
    /// for, which `place` records with its entry.
    pending_block: Option<usize>,
    /// The root entry of each sequence and property declaration.
    declarations: HashMap<NodeId, u32>,
    /// The byte offset of each line of a source file, by file.
    line_starts: HashMap<String, Vec<usize>>,
    /// The entry of each statement node built (statement coverage links
    /// its entities to them, 40.5.3).
    statement_rows: HashMap<NodeId, u32>,
}

impl<'c, 'a> CodeTables<'c, 'a> {
    pub(super) fn new(
        cg: &'c Codegen<'a>,
        model: &'c IrModel,
        typespecs: &'c mut TypespecTable,
    ) -> Self {
        Self {
            cg,
            typespecs,
            rows: model
                .vpi_objects
                .iter()
                .enumerate()
                .map(|(index, object)| (object.full_name.as_str(), index))
                .collect(),
            decl_rows: Default::default(),
            user_calls: model
                .vpi_compile_calls
                .iter()
                .map(|call| call.name.as_str())
                .collect(),
            code: Vec::new(),
            files: Vec::new(),
            file_index: HashMap::new(),
            named_blocks: Vec::new(),
            pending_block: None,
            declarations: HashMap::new(),
            line_starts: HashMap::new(),
            statement_rows: HashMap::new(),
        }
    }

    /// The finished tables: entries, files, the statement entry of each
    /// named block's catalog row and the entry of each statement node.
    pub(super) fn finish(
        self,
    ) -> (
        Vec<IrVpiCode>,
        Vec<String>,
        Vec<(usize, u32)>,
        HashMap<NodeId, u32>,
    ) {
        (self.code, self.files, self.named_blocks, self.statement_rows)
    }

    /// The catalog row a full name keys, if any.
    pub(super) fn row(&self, key: &str) -> Option<usize> {
        self.rows.get(key).copied()
    }

    /// The catalog row of declaration or scope `id`.
    fn decl_row(&self, id: NodeId) -> Option<usize> {
        if let Some(row) = self.decl_rows.borrow().get(&id) {
            return *row;
        }
        let row = self.rows.get(self.cg.vpi_full_name(id).as_str()).copied();
        self.decl_rows.borrow_mut().insert(id, row);
        row
    }

    /// Add assertion statement `id` as a root and return its entry.
    pub(super) fn assertion_root(&mut self, id: NodeId, scope: Option<usize>) -> u32 {
        self.root(Item::Assertion(id), scope)
    }

    /// Add the clocking entry of block `block` (or of its io declaration
    /// `var`) as a root and return it.
    pub(super) fn clocking_root(
        &mut self,
        block: NodeId,
        var: Option<NodeId>,
        scope: Option<usize>,
    ) -> u32 {
        self.root(Item::Clocking(block, var), scope)
    }

    /// Add the constraint items of constraint body `body` under a hidden
    /// root and return the root's entry.
    pub(super) fn constraint_root(&mut self, body: NodeId, scope: Option<usize>) -> u32 {
        self.root(Item::Constraint(body), scope)
    }

    /// Add the expression `id` as a root and return its entry.
    pub(super) fn expression_root(&mut self, id: NodeId, scope: Option<usize>) -> u32 {
        self.root(Item::Expr(id), scope)
    }

    /// Add the `vpiStmt` of a process, task or function and return its
    /// entry: its statement, the unnamed begin holding the statements of a
    /// body without a block of its own, or nothing for an empty body
    /// (37.12 detail 5).
    pub(super) fn statement_root(&mut self, id: NodeId, scope: Option<usize>) -> Option<u32> {
        let item = self.statement_item(id)?;
        Some(self.root(item, scope))
    }

    fn root(&mut self, item: Item<'a>, scope: Option<usize>) -> u32 {
        let slot = self.code.len();
        self.code.push(IrVpiCode::new("0"));
        self.place(slot, "0", item, scope);
        u32::try_from(slot).unwrap_or(u32::MAX)
    }

    /// Build `item` into entry `slot`, reserving its children's entries
    /// contiguously before building them.
    fn place(&mut self, slot: usize, role: &'static str, item: Item<'a>, scope: Option<usize>) {
        let entry = u32::try_from(slot).unwrap_or(u32::MAX);
        match &item {
            Item::Stmt(id) => {
                self.statement_rows.entry(*id).or_insert(entry);
                // An unnamed block the entry folds into the for or foreach
                // statement it holds.
                if matches!(self.cg.kind(*id), NodeKind::Stmt(StmtKind::Begin))
                    && self.cg.node(*id).name.is_empty()
                {
                    let found = self
                        .cg
                        .vpi_for_declarations(*id)
                        .map(|(found, _)| found)
                        .or_else(|| self.cg.vpi_foreach_block(*id));
                    if let Some(found) = found {
                        self.statement_rows.entry(found).or_insert(entry);
                    }
                }
            }
            Item::ExprStmt(id) | Item::ForWithLocals(id, _) => {
                self.statement_rows.entry(*id).or_insert(entry);
            }
            _ => {}
        }
        let (mut code, children, inner) = self.build(item, scope);
        if let Some(row) = self.pending_block.take() {
            self.named_blocks
                .push((row, u32::try_from(slot).unwrap_or(u32::MAX)));
        }
        code.role = role;
        code.scope = scope;
        let first = self.code.len();
        // A link keeps the entry it names; it has no children.
        if code.flags & IrVpiCode::LINK == 0 {
            code.first = u32::try_from(first).unwrap_or(u32::MAX);
        }
        code.count = u32::try_from(children.len()).unwrap_or(u32::MAX);
        self.code[slot] = code;
        self.code
            .resize_with(first + children.len(), || IrVpiCode::new("0"));
        for (index, (role, child)) in children.into_iter().enumerate() {
            self.place(first + index, role, child, inner);
        }
    }

    fn located(&mut self, type_code: &'static str, id: NodeId) -> IrVpiCode {
        let node = self.cg.node(id);
        let mut code = IrVpiCode::new(type_code);
        // A written block starts at its `begin` or `fork` keyword (decision
        // A19-D13), even when its range is only its one statement's.
        code.line = self.cg.db.block_keyword_line(id).unwrap_or(node.line);
        if let Some(file) = node.file() {
            let next = u32::try_from(self.files.len()).unwrap_or(u32::MAX);
            let index = *self.file_index.entry(file.to_owned()).or_insert_with(|| {
                self.files.push(file.to_owned());
                next
            });
            code.file = Some(index);
        }
        code
    }

    /// Build one entry: its fields, its children and the scope they live
    /// in.
    fn build(
        &mut self,
        item: Item<'a>,
        scope: Option<usize>,
    ) -> (IrVpiCode, Children<'a>, Option<usize>) {
        match item {
            Item::Expr(id) => {
                let (code, children) = self.expression(id);
                (code, children, scope)
            }
            Item::Stmt(id) => self.statement(id, scope),
            Item::Body(at, statements) => {
                // A statement list has no position of its own: the begin
                // that stands for a task or function body starts where the
                // body opens, after the subroutine name (decision A19-D13),
                // any other at its first statement.
                let anchor = match statements.first() {
                    Some(first) if self.cg.node(at).line == 0 => *first,
                    _ => at,
                };
                let mut code = self.located("vpiBegin", anchor);
                if let Some(opening) = self.cg.db.subroutine_body_line(at) {
                    code.line = opening;
                }
                let children = statements
                    .into_iter()
                    .filter_map(|id| Some(("vpiStmt", self.statement_item(id)?)))
                    .collect();
                (code, children, scope)
            }
            Item::ExprStmt(id) => {
                let (code, children) = self.expression_statement(id);
                (code, children, scope)
            }
            Item::Decl(var) => {
                let name = self.cg.node(var).name.clone();
                let (code, children) = self.reference(var, Some(var), name);
                (code, children, scope)
            }
            Item::DeclInit(var) => {
                let mut code = self.located("vpiAssignment", var);
                code.subtype = "vpiAssignmentOp";
                code.flags |= IrVpiCode::BLOCKING;
                let mut children = vec![("vpiLhs", Item::Decl(var))];
                if let Some(init) = self.cg.db.var_initializer(var) {
                    children.push(("vpiRhs", Item::Expr(init)));
                }
                (code, children, scope)
            }
            Item::Assertion(id) => {
                let (code, children) = self.assertion(id);
                (code, children, scope)
            }
            Item::Property(property, statement) => {
                let (code, children) = self.property(property, statement);
                (code, children, scope)
            }
            Item::AssertionExpr(id) => {
                let (code, children) = self.assertion_expression(id);
                (code, children, scope)
            }
            Item::Clock(clock) => self.clock(clock, scope),
            Item::Declaration(id) => {
                let (code, children) = self.declaration(id);
                (code, children, scope)
            }
            Item::AssertionFormal(id, type_code) => (self.formal(id, type_code), Vec::new(), scope),
            Item::Clocking(block, var) => {
                let cg = self.cg;
                let code = self.located("vpiClockingBlock", var.unwrap_or(block));
                let Some(info) = cg.db.clocking_block(block) else {
                    return (code, Vec::new(), scope);
                };
                let mut children = Vec::new();
                let (input, output) = match var.and_then(|var| cg.db.clocking_var(var)) {
                    Some(decl) => super::clocking_var_skews(decl, info),
                    None => {
                        children.push(("vpiClockingEvent", Item::ClockingEvent(block)));
                        (Some(&info.default_input), Some(&info.default_output))
                    }
                };
                let at = var.unwrap_or(block);
                if let Some(skew) = input {
                    children.push(("vpiInputSkew", Item::Skew(skew, at)));
                }
                if let Some(skew) = output {
                    children.push(("vpiOutputSkew", Item::Skew(skew, at)));
                }
                (code, children, scope)
            }
            // The clocking event: an event control without a statement
            // (37.44, 37.60).
            Item::ClockingEvent(block) => {
                let cg = self.cg;
                let Some(info) = cg.db.clocking_block(block) else {
                    return (self.located("vpiEventControl", block), Vec::new(), scope);
                };
                let code = self.located("vpiEventControl", info.event);
                let children = (!info.event_specs.is_empty())
                    .then(|| ("vpiCondition", Item::Events(&info.event_specs, info.event)))
                    .into_iter()
                    .collect();
                (code, children, scope)
            }
            Item::Constraint(body) => {
                let code = self.located("vpiConstraint", body);
                let mut children = Vec::new();
                self.constraint_set(body, "vpiConstraintItem", &mut children);
                (code, children, scope)
            }
            Item::ConstraintPart(id) => {
                let (code, children) = self.constraint_item(id);
                (code, children, scope)
            }
            // 37.30: a value or value range with its weight; `:/` divides the
            // weight over the range.
            Item::Weighted(item, dist) => {
                let mut code = self.located("vpiDistItem", item.value);
                if code.line == 0 {
                    code.line = self.cg.node(dist).line;
                }
                code.subtype = if item.per_range {
                    "vpiDivDist"
                } else {
                    "vpiEqualDist"
                };
                let mut children = vec![("vpiValueRange", Item::Expr(item.value))];
                if let Some(weight) = item.weight {
                    children.push(("vpiWeight", Item::Expr(weight)));
                }
                (code, children, scope)
            }
            Item::Skew(skew, at) => {
                let code = self.located("vpiDelayControl", skew.delay.unwrap_or(at));
                let children = skew
                    .delay_expression
                    .map(|delay| ("vpiDelay", Item::Expr(delay)))
                    .into_iter()
                    .collect();
                (code, children, scope)
            }
            Item::Link(entry) => {
                let mut code = IrVpiCode::new("0");
                code.flags |= IrVpiCode::LINK;
                code.first = entry;
                (code, Vec::new(), scope)
            }
            Item::OwnedEvent(spec, at) => match spec {
                EventSpec::Qualified { event, condition } => {
                    let mut code = self.located("vpiOperation", at);
                    code.subtype = "vpiIffOp";
                    let children = vec![
                        ("vpiOperand", Item::OwnedEvent(*event, at)),
                        ("vpiOperand", Item::Expr(condition)),
                    ];
                    (code, children, scope)
                }
                EventSpec::Edge { sig, posedge } => {
                    let mut code = self.located("vpiOperation", sig);
                    code.subtype = if posedge {
                        "vpiPosedgeOp"
                    } else {
                        "vpiNegedgeOp"
                    };
                    code.width = 1;
                    (code, vec![("vpiOperand", Item::Expr(sig))], scope)
                }
                EventSpec::AnyChange { sig } | EventSpec::Named(sig) => {
                    self.build(Item::Expr(sig), scope)
                }
            },
            Item::Alias(row, at) => {
                let mut code = self.located("0", at);
                code.flags |= IrVpiCode::ALIAS;
                code.object = Some(row);
                (code, Vec::new(), scope)
            }
            Item::Events(specs, at) => {
                if let [spec] = specs {
                    return self.build(Item::Event(spec, at), scope);
                }
                let mut code = self.located("vpiOperation", at);
                code.subtype = "vpiEventOrOp";
                let children = specs
                    .iter()
                    .map(|spec| ("vpiOperand", Item::Event(spec, at)))
                    .collect();
                (code, children, scope)
            }
            Item::Event(spec, at) => match spec {
                EventSpec::AnyChange { sig } | EventSpec::Named(sig) => {
                    self.build(Item::Expr(*sig), scope)
                }
                EventSpec::Edge { sig, posedge } => {
                    let mut code = self.located("vpiOperation", *sig);
                    code.subtype = if *posedge {
                        "vpiPosedgeOp"
                    } else {
                        "vpiNegedgeOp"
                    };
                    code.width = 1;
                    (code, vec![("vpiOperand", Item::Expr(*sig))], scope)
                }
                EventSpec::Qualified { event, condition } => {
                    let mut code = self.located("vpiOperation", at);
                    code.subtype = "vpiIffOp";
                    (
                        code,
                        vec![
                            ("vpiOperand", Item::Event(event, at)),
                            ("vpiOperand", Item::Expr(*condition)),
                        ],
                        scope,
                    )
                }
            },
            Item::Branch(item, at) => {
                let code = self.located("vpiCaseItem", item.body.unwrap_or(at));
                let mut children: Children<'a> = item
                    .exprs
                    .iter()
                    .map(|expr| ("vpiExpr", Item::Expr(*expr)))
                    .collect();
                if let Some(body) = item.body.and_then(|body| self.statement_item(body)) {
                    children.push(("vpiStmt", body));
                }
                (code, children, scope)
            }
            Item::Control(control, at) => {
                let (code, children) = self.timing_control(control, at);
                (code, children, scope)
            }
            Item::NullOp(at) => {
                let mut code = self.located("vpiOperation", at);
                code.subtype = "vpiNullOp";
                (code, Vec::new(), scope)
            }
            Item::One(at) => {
                let mut code = self.located("vpiConstant", at);
                code.subtype = "vpiDecConst";
                code.flags |= IrVpiCode::SIGNED;
                code.width = 32;
                code.text = Some(format!("{:032b}", 1));
                (code, Vec::new(), scope)
            }
            Item::ForWithLocals(id, declarations) => {
                let (mut code, mut children, inner) = self.statement(id, scope);
                code.flags |= IrVpiCode::LOCALS;
                let inits = declarations
                    .into_iter()
                    .map(|var| ("vpiForInitStmt", Item::DeclInit(var)));
                children.splice(0..0, inits);
                (code, children, inner)
            }
        }
    }

    /// The width, signedness, realness and state domain of expression `id`.
    fn shape(&self, id: NodeId) -> (u32, bool, bool, bool) {
        let descriptor = self.cg.query_descriptor(id);
        let real = descriptor.is_some_and(|d| matches!(d.shape, TypeShape::Real { .. }));
        let signed = descriptor.is_some_and(|d| d.info.signed);
        let two_state = descriptor.is_some_and(|d| d.two_state);
        let width = if real {
            0
        } else {
            self.cg.fixed_value_width(id).unwrap_or(0)
        };
        (width, signed, real, two_state)
    }

    fn typed(&mut self, type_code: &'static str, id: NodeId) -> IrVpiCode {
        let (width, signed, real, _) = self.shape(id);
        let mut code = self.located(type_code, id);
        code.width = width;
        if signed {
            code.flags |= IrVpiCode::SIGNED;
        }
        if real {
            code.flags |= IrVpiCode::REAL;
        }
        code
    }

    /// The typespec of the value of expression `id`, or of the declared
    /// type of declaration `id` named by the typedef it names (37.23,
    /// 37.55 detail 5).
    fn value_typespec(&mut self, id: NodeId) -> Option<usize> {
        let cg = self.cg;
        let descriptor = cg.db.type_descriptor(id)?;
        cg.vpi_typespec(self.typespecs, descriptor, cg.db.declared_typedef(id), 0)
    }

    /// An expression llg does not decompose: its type, its size and no
    /// operands or value.
    /// The items of constraint set `id` as children reached by `role`: a
    /// braced set contributes its items. A `foreach` constraint and the
    /// later-edition `unique` and `disable soft` items have no 37.30 or
    /// 37.34 object (decision A19-D37).
    fn constraint_set(&self, id: NodeId, role: &'static str, children: &mut Children<'a>) {
        match self.cg.kind(id) {
            NodeKind::Constraint(ConstraintKind::List { items }) => {
                for item in items {
                    self.constraint_set(*item, role, children);
                }
            }
            NodeKind::Constraint(
                ConstraintKind::Foreach { .. }
                | ConstraintKind::Uniqueness { .. }
                | ConstraintKind::DisableSoft { .. },
            ) => {}
            _ => children.push((role, Item::ConstraintPart(id))),
        }
    }

    /// One constraint item (37.30) or constraint expression (37.34): an
    /// expression, a distribution, an implication, a constraint `if` or
    /// `if`-`else`, or a `solve ... before` ordering.
    fn constraint_item(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let (mut code, children) = self.constraint_object(id);
        // A constraint item without a position of its own starts at its
        // first expression.
        if code.line == 0 {
            if let Some(Item::Expr(first)) = children.first().map(|(_, item)| item) {
                let at = self.located("0", *first);
                code.line = at.line;
                code.file = at.file;
            }
        }
        (code, children)
    }

    fn constraint_object(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let kind = match cg.kind(id) {
            NodeKind::Constraint(kind) => kind,
            _ => return self.expression(id),
        };
        let mut children = Vec::new();
        match kind {
            ConstraintKind::Expr { expr, .. } => {
                let expr = *expr;
                if let NodeKind::Expr(ExprKind::Dist { left, items, .. }) = cg.kind(expr) {
                    let code = self.located("vpiDistribution", expr);
                    children.push(("vpiExpr", Item::Expr(*left)));
                    for item in items {
                        children.push(("vpiDistItem", Item::Weighted(*item, expr)));
                    }
                    return (code, children);
                }
                self.expression(expr)
            }
            ConstraintKind::Implication { predicate, body } => {
                let code = self.located("vpiImplication", id);
                children.push(("vpiCondition", Item::Expr(*predicate)));
                self.constraint_set(*body, "vpiConstraintExpr", &mut children);
                (code, children)
            }
            ConstraintKind::Conditional {
                predicate,
                then_body,
                else_body,
            } => {
                let code = self.located(
                    if else_body.is_some() {
                        "vpiConstrIfElse"
                    } else {
                        "vpiConstrIf"
                    },
                    id,
                );
                children.push(("vpiCondition", Item::Expr(*predicate)));
                self.constraint_set(*then_body, "vpiConstraintExpr", &mut children);
                if let Some(otherwise) = else_body {
                    self.constraint_set(*otherwise, "vpiElseConst", &mut children);
                }
                (code, children)
            }
            // 18.5.9: `solve solve_list before before_list`.
            ConstraintKind::SolveBefore { solve, before } => {
                let code = self.located("vpiConstraintOrdering", id);
                children.extend(solve.iter().map(|id| ("vpiSolveBefore", Item::Expr(*id))));
                children.extend(before.iter().map(|id| ("vpiSolveAfter", Item::Expr(*id))));
                (code, children)
            }
            _ => self.opaque("vpiOperation", id),
        }
    }

    fn opaque(&mut self, type_code: &'static str, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let mut code = self.typed(type_code, id);
        code.flags |= IrVpiCode::OPAQUE;
        (code, Vec::new())
    }

    fn expression(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let kind = match cg.kind(id) {
            NodeKind::Expr(kind) => kind,
            NodeKind::FuncCall { .. } | NodeKind::SysCall { .. } | NodeKind::MethodCall { .. } => {
                return self.call(id, false);
            }
            _ => return self.opaque("vpiOperation", id),
        };
        if cg.db.semantic_detail(id) == Some("EmptyArgument") {
            let mut code = self.located("vpiOperation", id);
            code.subtype = "vpiNullOp";
            return (code, Vec::new());
        }
        match kind {
            ExprKind::Constant {
                value,
                size,
                const_type,
                source,
                ..
            } => self.constant(id, value, *size, *const_type, source),
            ExprKind::Unbounded => {
                let mut code = self.located("vpiConstant", id);
                code.subtype = "vpiUnboundedConst";
                code.flags |= IrVpiCode::OPAQUE;
                code.text = Some("$".to_owned());
                (code, Vec::new())
            }
            ExprKind::Ref { target } => {
                let target = *target;
                self.reference(id, target, cg.node(id).name.clone())
            }
            ExprKind::HierPath { parts, refs } => {
                let target = refs.last().copied().flatten();
                self.reference(id, target, parts.join("."))
            }
            ExprKind::ScopeRef { target } => {
                let name = cg.node(*target).name.clone();
                self.reference(id, Some(*target), name)
            }
            ExprKind::Cast {
                operand, size_cast, ..
            } => {
                let mut code = self.typed("vpiOperation", id);
                code.subtype = "vpiCastOp";
                if self.shape(id).3 {
                    code.flags |= IrVpiCode::TWO_STATE;
                }
                if cg.db.is_implicit_conversion(id) {
                    code.flags |= IrVpiCode::HIDDEN;
                } else {
                    // 37.55 detail 3: the typespec of the cast's target type,
                    // named after the typedef the cast names.
                    code.typespec = cg.db.type_descriptor(id).and_then(|descriptor| {
                        cg.vpi_typespec(self.typespecs, descriptor, cg.db.declared_typedef(id), 0)
                    });
                    if *size_cast {
                        code.text = Some(code.width.to_string());
                    }
                }
                (code, vec![("vpiOperand", Item::Expr(*operand))])
            }
            // A value range `[low:high]` of a dist item or an inside
            // operator (11.4.13, 18.5.4) is a range (37.30 vpiValueRange).
            ExprKind::Operation {
                op: Operation::List,
                operands,
                ..
            } if operands.len() == 2 && cg.db.semantic_detail(id) == Some("ValueRange") => {
                let code = self.located("vpiRange", id);
                (
                    code,
                    vec![
                        ("vpiLeftRange", Item::Expr(operands[0])),
                        ("vpiRightRange", Item::Expr(operands[1])),
                    ],
                )
            }
            ExprKind::Operation {
                op,
                assignment,
                operands,
                ..
            } => {
                let Some(op_type) = vpi_op_type(*op).filter(|_| !*assignment) else {
                    return self.opaque("vpiOperation", id);
                };
                let mut code = self.typed("vpiOperation", id);
                code.subtype = op_type;
                let mut children: Children<'a> = Vec::with_capacity(operands.len());
                for (index, operand) in operands.iter().enumerate() {
                    // 37.55 detail 1: the operands of a replication are the
                    // multiplier and the expressions of the concatenation.
                    if index == 1 && *op == Operation::MultiConcat {
                        if let NodeKind::Expr(ExprKind::Operation {
                            op: Operation::Concat,
                            operands: inner,
                            ..
                        }) = cg.kind(*operand)
                        {
                            children.extend(inner.iter().map(|e| ("vpiOperand", Item::Expr(*e))));
                            continue;
                        }
                    }
                    children.push(("vpiOperand", Item::Expr(*operand)));
                }
                (code, children)
            }
            ExprKind::BitSelect { base, index } => {
                let mut code = self.select("vpiBitSelect", id, *base, &[*index]);
                code.0.width = code.0.width.max(1);
                code.1.push(("vpiIndex", Item::Expr(*index)));
                code
            }
            ExprKind::PartSelect { base, left, right } => {
                let mut code = self.select("vpiPartSelect", id, *base, &[*left, *right]);
                code.1.push(("vpiLeftRange", Item::Expr(*left)));
                code.1.push(("vpiRightRange", Item::Expr(*right)));
                code
            }
            ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            } => {
                let mut code = self.select(
                    "vpiIndexedPartSelect",
                    id,
                    *base,
                    &[*base_expr, *width_expr],
                );
                code.0.subtype = if *neg {
                    "vpiNegIndexed"
                } else {
                    "vpiPosIndexed"
                };
                code.1.push(("vpiBaseExpr", Item::Expr(*base_expr)));
                code.1.push(("vpiWidthExpr", Item::Expr(*width_expr)));
                code
            }
            ExprKind::ArraySelect { base, indices } => {
                // An element of a packed dimension is a select of the
                // packed value (37.54), like a bit-select of a vector.
                let packed = matches!(
                    cg.query_descriptor(*base).map(|descriptor| &descriptor.shape),
                    Some(TypeShape::PackedAtom { ranges }) if !ranges.is_empty()
                );
                if packed && indices.len() == 1 {
                    let mut code = self.select("vpiBitSelect", id, *base, indices);
                    code.1.push(("vpiIndex", Item::Expr(indices[0])));
                    return code;
                }
                let mut code = self.select("vpiVarSelect", id, *base, indices);
                // The element offset comes from the array's own dimensions.
                code.0.range = None;
                code.1
                    .extend(indices.iter().map(|index| ("vpiIndex", Item::Expr(*index))));
                code
            }
            _ => self.opaque("vpiOperation", id),
        }
    }

    fn constant(
        &mut self,
        id: NodeId,
        value: &crate::core::value::ValueData,
        size: i32,
        const_type: ConstantType,
        source: &ConstantSource,
    ) -> (IrVpiCode, Children<'a>) {
        let mut code = self.located("vpiConstant", id);
        code.subtype = vpi_const_type(const_type, source);
        match const_type {
            ConstantType::Null => {
                code.subtype = "vpiNullConst";
                return (code, Vec::new());
            }
            ConstantType::Time => {
                // A time literal is scaled by its scope's time unit; it is
                // decompiled from its spelling and has no value of its own.
                code.flags |= IrVpiCode::OPAQUE | IrVpiCode::REAL;
                if let ConstantSource::Exact(spelling) = source {
                    code.text = Some(spelling.clone());
                }
                return (code, Vec::new());
            }
            ConstantType::String => {
                if let Ok(text) = decoded_string_text(value, "string") {
                    code.flags |= IrVpiCode::STRING;
                    code.width = u32::try_from(text.len().max(1) * 8).unwrap_or(u32::MAX);
                    code.text = Some(text);
                    return (code, Vec::new());
                }
            }
            _ => {}
        }
        let Ok(constant) = read_const_from(value, size) else {
            code.flags |= IrVpiCode::OPAQUE;
            return (code, Vec::new());
        };
        if let Some(real) = constant.real {
            code.subtype = "vpiRealConst";
            code.flags |= IrVpiCode::REAL;
            code.text = Some(if real.is_finite() {
                format!("{real:?}")
            } else {
                "0.0".to_owned()
            });
            return (code, Vec::new());
        }
        if constant.signed {
            code.flags |= IrVpiCode::SIGNED;
        }
        if unbased_unsized(source) {
            code.flags |= IrVpiCode::FILL;
        }
        code.width = constant.width;
        code.text = Some(constant_digits(&constant));
        (code, Vec::new())
    }

    /// A reference: the catalog object it names, a value of an enum
    /// constant, or a named entry of its own for a declaration that is not
    /// a catalog object (an automatic variable, a class property).
    fn reference(
        &mut self,
        id: NodeId,
        target: Option<NodeId>,
        name: String,
    ) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        if let Some(target) = target {
            if let NodeKind::EnumConst {
                value: Some(Val::Bits(bits)),
            } = cg.kind(target)
            {
                let mut code = self.located("vpiConstant", id);
                code.subtype = "vpiBinaryConst";
                code.width = u32::try_from(bits.bits.len()).unwrap_or(u32::MAX);
                if bits.signed {
                    code.flags |= IrVpiCode::SIGNED;
                }
                code.text = Some(
                    bits.bits
                        .iter()
                        .map(|bit| match bit {
                            Bit::Zero => '0',
                            Bit::One => '1',
                            Bit::X => 'x',
                            Bit::Z => 'z',
                        })
                        .collect(),
                );
                return (code, Vec::new());
            }
            // A class member names no catalog row: its path starts at the
            // class, not at a scope (37.17 detail 25).
            let class_member = cg
                .node(target)
                .parent
                .is_some_and(|parent| matches!(cg.kind(parent), NodeKind::ClassDef));
            let catalog = !class_member
                && matches!(
                    cg.kind(target),
                    NodeKind::Net { .. }
                        | NodeKind::Var { .. }
                        | NodeKind::Array { .. }
                        | NodeKind::Param { .. }
                        | NodeKind::NamedEvent
                        | NodeKind::ModuleInst { .. }
                        | NodeKind::GenScope
                );
            if let Some(row) = catalog.then(|| self.decl_row(target)).flatten() {
                let mut code = self.typed("0", id);
                code.flags |= IrVpiCode::ALIAS;
                code.object = Some(row);
                code.text = Some(name);
                return (code, Vec::new());
            }
            let type_code = match cg.kind(target) {
                NodeKind::Var { ty } | NodeKind::Net { ty, .. } | NodeKind::FuncArg { ty, .. } => {
                    IrVpiVarKind::from_type_kind(&ty.kind, cg.vpi_packed_dimension_count(target))
                        .c_type()
                }
                NodeKind::Array { .. } => "vpiRegArray",
                NodeKind::NamedEvent => "vpiNamedEvent",
                _ => "vpiRefObj",
            };
            let mut code = self.typed(type_code, id);
            code.flags |= IrVpiCode::OPAQUE;
            code.text = Some(name);
            // A simple expression always has a typespec (37.55 detail 5).
            code.typespec = self.value_typespec(target);
            return (code, Vec::new());
        }
        let mut code = self.typed("vpiRefObj", id);
        code.flags |= IrVpiCode::OPAQUE;
        code.text = Some(name);
        (code, Vec::new())
    }

    /// A select of `base` by `indices`: the parent relation and the range of
    /// the dimension it indexes (37.19, 37.54, 37.55).
    fn select(
        &mut self,
        type_code: &'static str,
        id: NodeId,
        base: NodeId,
        indices: &[NodeId],
    ) -> (IrVpiCode, Children<'a>) {
        let mut code = self.typed(type_code, id);
        // 37.55 detail 5: a select's typespec is that of its value
        // (decision A19-D32).
        code.typespec = self.value_typespec(id);
        let parent_constant = match self.cg.kind(base) {
            NodeKind::Expr(
                ExprKind::Ref { .. } | ExprKind::HierPath { .. } | ExprKind::ScopeRef { .. },
            ) => true,
            NodeKind::Expr(
                ExprKind::BitSelect { .. }
                | ExprKind::PartSelect { .. }
                | ExprKind::IndexedPartSelect { .. }
                | ExprKind::ArraySelect { .. },
            ) => self.constant_select(base),
            NodeKind::Expr(_) => false,
            // A select whose base names the declaration directly.
            _ => true,
        };
        if parent_constant
            && indices
                .iter()
                .all(|index| self.constant_expression(*index, 0))
        {
            code.flags |= IrVpiCode::CONSTANT;
        }
        let (base_width, ..) = self.shape(base);
        code.range =
            self.cg
                .query_descriptor(base)
                .and_then(|descriptor| match &descriptor.shape {
                    TypeShape::PackedAtom { ranges } if !ranges.is_empty() => Some((
                        i64::try_from(ranges[0].left).ok()?,
                        i64::try_from(ranges[0].right).ok()?,
                    )),
                    _ => (base_width > 0).then(|| (i64::from(base_width) - 1, 0)),
                });
        let parent = match self.cg.kind(base) {
            NodeKind::Expr(_) => Item::Expr(base),
            _ => match self.decl_row(base) {
                Some(row) => Item::Alias(row, id),
                None => Item::Expr(base),
            },
        };
        (code, vec![("vpiParent", parent)])
    }

    fn constant_select(&self, id: NodeId) -> bool {
        let (base, indices): (NodeId, Vec<NodeId>) = match self.cg.kind(id) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, vec![*index]),
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                (*base, vec![*left, *right])
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                ..
            }) => (*base, vec![*base_expr, *width_expr]),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => (*base, indices.clone()),
            NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. }) => return true,
            NodeKind::Expr(_) => return false,
            _ => return true,
        };
        self.constant_select(base)
            && indices
                .iter()
                .all(|index| self.constant_expression(*index, 0))
    }

    /// An elaboration-time constant expression: literals, parameters and
    /// operations on them.
    fn constant_expression(&self, id: NodeId, depth: u32) -> bool {
        if depth > MAX_TYPESPEC_DEPTH {
            return false;
        }
        match self.cg.kind(id) {
            NodeKind::Expr(ExprKind::Constant { .. }) => true,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => matches!(
                self.cg.kind(*target),
                NodeKind::Param { .. } | NodeKind::Genvar { .. } | NodeKind::EnumConst { .. }
            ),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.constant_expression(*operand, depth + 1)
            }
            NodeKind::Expr(ExprKind::Operation {
                operands,
                assignment: false,
                ..
            }) => operands
                .iter()
                .all(|operand| self.constant_expression(*operand, depth + 1)),
            _ => false,
        }
    }

    /// A call: `vpiFuncCall`/`vpiTaskCall` with the subroutine it calls and
    /// its arguments, `vpiSysFuncCall`/`vpiSysTaskCall` with its arguments
    /// (37.38). A method call is not decomposed.
    fn call(&mut self, id: NodeId, statement: bool) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let arguments = || -> Children<'a> {
            cg.node(id)
                .children
                .iter()
                .filter(|child| {
                    matches!(
                        cg.kind(**child),
                        NodeKind::Expr(_)
                            | NodeKind::FuncCall { .. }
                            | NodeKind::SysCall { .. }
                            | NodeKind::MethodCall { .. }
                    )
                })
                .map(|child| {
                    let item = if cg.db.semantic_detail(*child) == Some("EmptyArgument") {
                        Item::NullOp(*child)
                    } else {
                        Item::Expr(*child)
                    };
                    ("vpiArgument", item)
                })
                .collect()
        };
        match cg.kind(id) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                let type_code = if *is_task {
                    "vpiTaskCall"
                } else {
                    "vpiFuncCall"
                };
                let mut code = if statement {
                    self.located(type_code, id)
                } else {
                    self.typed(type_code, id)
                };
                code.text = Some(name.clone());
                if !statement {
                    // The type of the value (decision A19-D32).
                    code.typespec = self.value_typespec(id);
                }
                let row = callee.and_then(|callee| self.decl_row(callee));
                code.object = row;
                let mut children = Vec::new();
                if let Some(row) = row {
                    children.push((
                        if *is_task { "vpiTask" } else { "vpiFunction" },
                        Item::Alias(row, id),
                    ));
                }
                children.extend(arguments());
                (code, children)
            }
            NodeKind::SysCall { name } => {
                let type_code = if statement {
                    "vpiSysTaskCall"
                } else {
                    "vpiSysFuncCall"
                };
                let mut code = if statement {
                    self.located(type_code, id)
                } else {
                    self.typed(type_code, id)
                };
                if self.user_calls.contains(name.as_str()) {
                    code.flags |= IrVpiCode::USER;
                }
                if !statement {
                    code.typespec = self.value_typespec(id);
                }
                code.text = Some(name.clone());
                (code, arguments())
            }
            NodeKind::MethodCall {
                name,
                receiver,
                callee,
            } => {
                let type_code = if statement {
                    "vpiMethodTaskCall"
                } else {
                    "vpiMethodFuncCall"
                };
                let mut code = if statement {
                    self.located(type_code, id)
                } else {
                    self.typed(type_code, id)
                };
                code.text = Some(name.clone());
                if !statement {
                    code.typespec = self.value_typespec(id);
                }
                // 37.38: the method's subroutine (NULL for a built-in
                // method, detail 11; a class method is no catalog object,
                // which USER marks), the object it applies to (detail 2),
                // the `with` expression of an array method (detail 1) and
                // the arguments (decision A19-D32).
                let mut children: Children<'a> = Vec::new();
                match callee.map(|callee| self.decl_row(callee)) {
                    Some(Some(row)) => children.push((
                        if statement { "vpiTask" } else { "vpiFunction" },
                        Item::Alias(row, id),
                    )),
                    Some(None) => code.flags |= IrVpiCode::USER,
                    None => {}
                }
                if let Some(receiver) = receiver {
                    children.push(("vpiPrefix", Item::Expr(*receiver)));
                }
                let mut operands: Vec<NodeId> = cg
                    .node(id)
                    .children
                    .iter()
                    .copied()
                    .filter(|child| Some(*child) != *receiver)
                    .collect();
                if cg.db.method_call_has_with_clause(id) {
                    // A randomize `with` block is a constraint, which is no
                    // object; an array method's `with` expression is its
                    // only operand.
                    operands.retain(|child| !matches!(cg.kind(*child), NodeKind::Constraint(_)));
                    if let [with] = operands.as_slice() {
                        children.push(("vpiWith", Item::Expr(*with)));
                        operands.clear();
                    }
                }
                children.extend(
                    operands
                        .into_iter()
                        .filter(|child| {
                            matches!(
                                cg.kind(*child),
                                NodeKind::Expr(_)
                                    | NodeKind::FuncCall { .. }
                                    | NodeKind::SysCall { .. }
                                    | NodeKind::MethodCall { .. }
                            )
                        })
                        .map(|child| {
                            let item = if cg.db.semantic_detail(child) == Some("EmptyArgument") {
                                Item::NullOp(child)
                            } else {
                                Item::Expr(child)
                            };
                            ("vpiArgument", item)
                        }),
                );
                (code, children)
            }
            _ => self.opaque("vpiOperation", id),
        }
    }

    /// The statement item of `id`: statement lists without a block of
    /// their own are folded away (a single statement is itself; several
    /// become an unnamed begin; none is no statement).
    fn statement_item(&self, id: NodeId) -> Option<Item<'a>> {
        let cg = self.cg;
        match (cg.kind(id), cg.db.semantic_detail(id)) {
            (NodeKind::Stmt(StmtKind::Begin), Some("List")) => {
                let mut statements = Vec::new();
                self.cg.vpi_flatten(id, &mut statements);
                match statements.as_slice() {
                    [] => None,
                    [single] => self.statement_item(*single),
                    _ => Some(Item::Body(id, statements)),
                }
            }
            (NodeKind::Stmt(StmtKind::Begin), Some("ExpressionStatement")) => cg
                .node(id)
                .children
                .first()
                .map(|expression| Item::ExprStmt(*expression)),
            (NodeKind::Stmt(StmtKind::VariableDecl { .. }), _) => None,
            (NodeKind::Stmt(StmtKind::Begin), Some("StatementBlock")) => None,
            (NodeKind::Stmt(_), _) => Some(Item::Stmt(id)),
            (
                NodeKind::FuncCall { .. } | NodeKind::SysCall { .. } | NodeKind::MethodCall { .. },
                _,
            ) => Some(Item::ExprStmt(id)),
            _ => None,
        }
    }

    /// A built statement that is the unnamed scope `row` (37.12 details
    /// 1-3): the entry stands for the row, and its children live in it.
    fn scoped(
        &mut self,
        built: (IrVpiCode, Children<'a>, Option<usize>),
        row: Option<usize>,
    ) -> (IrVpiCode, Children<'a>, Option<usize>) {
        let (mut code, children, inner) = built;
        let Some(row) = row else {
            return (code, children, inner);
        };
        code.flags |= IrVpiCode::ALIAS;
        code.object = Some(row);
        self.pending_block = Some(row);
        (code, children, Some(row))
    }

    fn statement(
        &mut self,
        id: NodeId,
        scope: Option<usize>,
    ) -> (IrVpiCode, Children<'a>, Option<usize>) {
        let cg = self.cg;
        let NodeKind::Stmt(kind) = cg.kind(id) else {
            let (code, children) = self.expression_statement(id);
            return (code, children, scope);
        };
        let body = |this: &Self, id: Option<NodeId>| -> Option<(&'static str, Item<'a>)> {
            Some(("vpiStmt", this.statement_item(id?)?))
        };
        let (code, children) = match kind {
            StmtKind::Begin | StmtKind::Fork { .. } => {
                let fork = matches!(kind, StmtKind::Fork { .. });
                let name = &cg.node(id).name;
                // An unnamed block that is a scope: its catalog row
                // (decision A19-D27).
                let scope_row = if name.is_empty() {
                    cg.vpi_block_scope(id)
                        .filter(|declared| cg.vpi_unnamed_scope_name(*declared).is_some())
                        .and_then(|declared| self.decl_row(declared))
                } else {
                    None
                };
                if !fork && name.is_empty() {
                    if let Some((found, declarations)) = self.cg.vpi_for_declarations(id) {
                        let built = self.build(Item::ForWithLocals(found, declarations), scope);
                        return self.scoped(built, scope_row);
                    }
                    if let Some(found) = self.cg.vpi_foreach_block(id) {
                        let built = self.statement(found, scope);
                        return self.scoped(built, scope_row);
                    }
                }
                let statements = match kind {
                    StmtKind::Fork { branches, .. } => branches.clone(),
                    _ => {
                        let mut statements = Vec::new();
                        self.cg.vpi_flatten(id, &mut statements);
                        statements
                    }
                };
                let children: Children<'a> = statements
                    .into_iter()
                    .filter_map(|statement| Some(("vpiStmt", self.statement_item(statement)?)))
                    .collect();
                if name.is_empty() {
                    let code = self.located(if fork { "vpiFork" } else { "vpiBegin" }, id);
                    return self.scoped((code, children, scope), scope_row);
                } else if let Some(row) = self.decl_row(id) {
                    // A named block is a catalog scope: the entry stands for
                    // it and its statements live in it.
                    let mut code = self.located("0", id);
                    code.flags |= IrVpiCode::ALIAS;
                    code.object = Some(row);
                    code.text = Some(name.clone());
                    self.pending_block = Some(row);
                    return (code, children, Some(row));
                } else {
                    let mut code = self.located(
                        if fork {
                            "vpiNamedFork"
                        } else {
                            "vpiNamedBegin"
                        },
                        id,
                    );
                    code.text = Some(name.clone());
                    (code, children)
                }
            }
            StmtKind::Assign {
                blocking,
                op,
                delay,
            } => {
                let mut code = self.located("vpiAssignment", id);
                code.subtype = if *op == Operation::Assignment {
                    "vpiAssignmentOp"
                } else {
                    vpi_op_type(*op).unwrap_or("vpiAssignmentOp")
                };
                if *blocking {
                    code.flags |= IrVpiCode::BLOCKING;
                }
                let operands = &cg.node(id).children;
                let mut children = Vec::new();
                if let Some(lhs) = operands.first() {
                    children.push(("vpiLhs", Item::Expr(*lhs)));
                }
                if let Some(rhs) = operands.get(1) {
                    children.push(("vpiRhs", Item::Expr(*rhs)));
                }
                if let Some(control) = delay {
                    if let Some(role) = control_type(control) {
                        children.push((role, Item::Control(control, id)));
                    }
                }
                (code, children)
            }
            StmtKind::IfElse {
                predicate,
                if_true,
                if_false,
                check,
            } => {
                let mut code = self.located(
                    if if_false.is_some() {
                        "vpiIfElse"
                    } else {
                        "vpiIf"
                    },
                    id,
                );
                code.qualifier = vpi_qualifier(*check);
                let mut children = Vec::new();
                match predicate.clauses.as_slice() {
                    [clause] if clause.pattern.is_none() => {
                        children.push(("vpiCondition", Item::Expr(clause.expression)));
                    }
                    _ => code.flags |= IrVpiCode::OPAQUE,
                }
                if code.flags & IrVpiCode::OPAQUE == 0 {
                    children.extend(body(self, Some(*if_true)));
                    if let Some(else_stmt) = if_false.and_then(|e| self.statement_item(e)) {
                        children.push(("vpiElseStmt", else_stmt));
                    }
                }
                (code, children)
            }
            StmtKind::Case {
                case_type,
                check,
                items,
            } => {
                let mut code = self.located("vpiCase", id);
                code.subtype = match case_type {
                    CaseKind::X => "vpiCaseX",
                    CaseKind::Z => "vpiCaseZ",
                    _ => "vpiCaseExact",
                };
                code.qualifier = match case_type {
                    CaseKind::Inside => match check {
                        UniquePriorityCheck::Unique | UniquePriorityCheck::Unique0 => {
                            "vpiUniqueQualifier | vpiInsideQualifier"
                        }
                        UniquePriorityCheck::Priority => {
                            "vpiPriorityQualifier | vpiInsideQualifier"
                        }
                        _ => "vpiInsideQualifier",
                    },
                    _ => vpi_qualifier(*check),
                };
                let mut children = Vec::new();
                if let Some(selector) = cg.node(id).children.first() {
                    children.push(("vpiCondition", Item::Expr(*selector)));
                }
                children.extend(
                    items
                        .iter()
                        .map(|item| ("vpiCaseItem", Item::Branch(item, id))),
                );
                (code, children)
            }
            StmtKind::For {
                vars,
                init,
                cond,
                incr,
                body: loop_body,
            } => {
                let mut code = self.located("vpiFor", id);
                let mut children: Children<'a> = Vec::new();
                if !vars.is_empty() {
                    code.flags |= IrVpiCode::LOCALS;
                    children.extend(
                        vars.iter()
                            .map(|var| ("vpiForInitStmt", Item::DeclInit(*var))),
                    );
                }
                children.extend(
                    init.iter()
                        .filter_map(|init| Some(("vpiForInitStmt", self.loop_step(*init)?))),
                );
                children.push(("vpiCondition", Item::Expr(*cond)));
                children.extend(
                    incr.iter()
                        .filter_map(|step| Some(("vpiForIncStmt", self.loop_step(*step)?))),
                );
                children.extend(body(self, Some(*loop_body)));
                (code, children)
            }
            StmtKind::While {
                cond,
                body: loop_body,
            }
            | StmtKind::DoWhile {
                cond,
                body: loop_body,
            }
            | StmtKind::Repeat {
                cond,
                body: loop_body,
            } => {
                let type_code = match kind {
                    StmtKind::While { .. } => "vpiWhile",
                    StmtKind::DoWhile { .. } => "vpiDoWhile",
                    _ => "vpiRepeat",
                };
                let code = self.located(type_code, id);
                let mut children = vec![("vpiCondition", Item::Expr(*cond))];
                children.extend(body(self, Some(*loop_body)));
                (code, children)
            }
            StmtKind::Forever { body: loop_body } => {
                let code = self.located("vpiForever", id);
                (code, body(self, Some(*loop_body)).into_iter().collect())
            }
            StmtKind::Foreach {
                array,
                vars,
                body: loop_body,
                ..
            } => {
                let code = self.located("vpiForeachStmt", id);
                let mut children = Vec::new();
                if let Some(row) = array.and_then(|array| self.decl_row(array)) {
                    children.push(("vpiVariables", Item::Alias(row, id)));
                }
                for var in vars {
                    children.push((
                        "vpiLoopVars",
                        match var {
                            Some(var) => Item::Decl(*var),
                            None => Item::NullOp(id),
                        },
                    ));
                }
                children.extend(body(self, Some(*loop_body)));
                (code, children)
            }
            StmtKind::EventControl {
                specs,
                body: control_body,
                ..
            } => {
                let code = self.located("vpiEventControl", id);
                let mut children = Vec::new();
                if !specs.is_empty() {
                    children.push(("vpiCondition", Item::Events(specs, id)));
                }
                children.extend(body(self, *control_body));
                (code, children)
            }
            StmtKind::DelayControl { delay } => {
                let code = self.located("vpiDelayControl", id);
                let mut children = vec![("vpiDelay", Item::Expr(*delay))];
                children.extend(body(self, cg.node(id).children.first().copied()));
                (code, children)
            }
            StmtKind::EventTrigger {
                blocking, target, ..
            } => {
                let mut code = self.located("vpiEventStmt", id);
                if *blocking {
                    code.flags |= IrVpiCode::BLOCKING;
                }
                let children = target
                    .map(|target| ("vpiNamedEvent", Item::Expr(target)))
                    .into_iter()
                    .collect();
                (code, children)
            }
            StmtKind::Wait { cond } => {
                let code = self.located("vpiWait", id);
                let mut children = vec![("vpiCondition", Item::Expr(*cond))];
                children.extend(body(self, cg.node(id).children.get(1).copied()));
                (code, children)
            }
            StmtKind::WaitOrder {
                events,
                if_true,
                if_false,
            } => {
                let code = self.located("vpiOrderedWait", id);
                let mut children: Children<'a> = events
                    .iter()
                    .map(|event| ("vpiCondition", Item::Expr(*event)))
                    .collect();
                children.extend(body(self, *if_true));
                if let Some(else_stmt) = if_false.and_then(|e| self.statement_item(e)) {
                    children.push(("vpiElseStmt", else_stmt));
                }
                (code, children)
            }
            StmtKind::Force { lhs, rhs } | StmtKind::ProcContAssign { lhs, rhs } => {
                let code = self.located(
                    if matches!(kind, StmtKind::Force { .. }) {
                        "vpiForce"
                    } else {
                        "vpiAssignStmt"
                    },
                    id,
                );
                (
                    code,
                    vec![("vpiLhs", Item::Expr(*lhs)), ("vpiRhs", Item::Expr(*rhs))],
                )
            }
            StmtKind::Release { lhs } | StmtKind::Deassign { lhs } => {
                let code = self.located(
                    if matches!(kind, StmtKind::Release { .. }) {
                        "vpiRelease"
                    } else {
                        "vpiDeassign"
                    },
                    id,
                );
                (code, vec![("vpiLhs", Item::Expr(*lhs))])
            }
            StmtKind::Disable { target } => {
                let code = self.located("vpiDisable", id);
                let children = target
                    .and_then(|target| self.decl_row(target))
                    .map(|row| ("vpiExpr", Item::Alias(row, id)))
                    .into_iter()
                    .collect();
                (code, children)
            }
            StmtKind::Return { value } => {
                let code = self.located("vpiReturnStmt", id);
                let children = value
                    .map(|value| ("vpiCondition", Item::Expr(value)))
                    .into_iter()
                    .collect();
                (code, children)
            }
            StmtKind::Empty => (self.located("vpiNullStmt", id), Vec::new()),
            StmtKind::Break => (self.located("vpiBreak", id), Vec::new()),
            StmtKind::Continue => (self.located("vpiContinue", id), Vec::new()),
            StmtKind::WaitFork => (self.located("vpiWaitFork", id), Vec::new()),
            StmtKind::DisableFork => (self.located("vpiDisableFork", id), Vec::new()),
            StmtKind::ImmediateAssertion {
                kind: assertion, ..
            } => {
                let type_code = match assertion {
                    crate::core::db::ImmediateAssertionKind::Assert => "vpiImmediateAssert",
                    crate::core::db::ImmediateAssertionKind::Assume => "vpiImmediateAssume",
                    crate::core::db::ImmediateAssertionKind::Cover => "vpiImmediateCover",
                };
                let mut code = self.located(type_code, id);
                code.flags |= IrVpiCode::OPAQUE;
                code.span = self.span(id);
                (code, Vec::new())
            }
            StmtKind::ConcurrentAssertion { .. } => {
                let mut code = self.located("vpiAssert", id);
                code.flags |= IrVpiCode::OPAQUE;
                code.span = self.span(id);
                (code, Vec::new())
            }
            StmtKind::PatternCase { .. } | StmtKind::RandCase { .. } => {
                let mut code = self.located("vpiCase", id);
                code.flags |= IrVpiCode::OPAQUE;
                (code, Vec::new())
            }
            StmtKind::CycleDelayControl { .. } => {
                let mut code = self.located("vpiDelayControl", id);
                code.flags |= IrVpiCode::OPAQUE;
                (code, Vec::new())
            }
            StmtKind::VariableDecl { .. }
            | StmtKind::RandSequence { .. }
            | StmtKind::ProceduralChecker { .. }
            | StmtKind::Unsupported { .. } => {
                let mut code = self.located("vpiNullStmt", id);
                code.flags |= IrVpiCode::OPAQUE;
                (code, Vec::new())
            }
        };
        (code, children, scope)
    }

    /// A for-loop initialization or step: an assignment statement or an
    /// expression used as one.
    fn loop_step(&self, id: NodeId) -> Option<Item<'a>> {
        match self.cg.kind(id) {
            NodeKind::Stmt(_) => self.statement_item(id),
            _ => Some(Item::ExprStmt(id)),
        }
    }

    /// An expression used as a statement: a call, an increment or
    /// decrement (a `vpiAssignment` adding or subtracting a constant `1`,
    /// as `k += 1` is under 37.59 detail 1; decision A19-D13), an
    /// assignment expression, else an opaque null statement.
    fn expression_statement(&mut self, id: NodeId) -> (IrVpiCode, Children<'a>) {
        let cg = self.cg;
        let mut inner = id;
        while let NodeKind::Expr(ExprKind::Cast { operand, .. }) = cg.kind(inner) {
            inner = *operand;
        }
        match cg.kind(inner) {
            NodeKind::FuncCall { .. } | NodeKind::SysCall { .. } | NodeKind::MethodCall { .. } => {
                self.call(inner, true)
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    op @ (Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement),
                operands,
                ..
            }) => {
                let mut code = self.located("vpiAssignment", inner);
                code.subtype = match op {
                    Operation::PostIncrement | Operation::PreIncrement => "vpiAddOp",
                    _ => "vpiSubOp",
                };
                code.flags |= IrVpiCode::BLOCKING;
                let children = operands
                    .first()
                    .map(|operand| {
                        vec![
                            ("vpiLhs", Item::Expr(*operand)),
                            ("vpiRhs", Item::One(inner)),
                        ]
                    })
                    .unwrap_or_default();
                (code, children)
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                assignment: true,
                operands,
                ..
            }) if operands.len() == 2 => {
                let mut code = self.located("vpiAssignment", inner);
                code.subtype = if *op == Operation::Assignment {
                    "vpiAssignmentOp"
                } else {
                    vpi_op_type(*op).unwrap_or("vpiAssignmentOp")
                };
                code.flags |= IrVpiCode::BLOCKING;
                (
                    code,
                    vec![
                        ("vpiLhs", Item::Expr(operands[0])),
                        ("vpiRhs", Item::Expr(operands[1])),
                    ],
                )
            }
            _ => {
                let mut code = self.located("vpiNullStmt", id);
                code.flags |= IrVpiCode::OPAQUE;
                (code, Vec::new())
            }
        }
    }

    /// An intra-assignment control: `vpiDelayControl` with its `vpiDelay`,
    /// `vpiEventControl` with its condition, `vpiRepeatControl` with its
    /// count and event control; none has a statement (37.60 and 37.63
    /// detail 1).
    fn timing_control(
        &mut self,
        control: &'a IntraControl,
        at: NodeId,
    ) -> (IrVpiCode, Children<'a>) {
        match control {
            IntraControl::Delay(delay) => {
                let code = self.located("vpiDelayControl", *delay);
                (code, vec![("vpiDelay", Item::Expr(*delay))])
            }
            IntraControl::Event { specs, .. } => {
                let code = self.located("vpiEventControl", at);
                let children = (!specs.is_empty())
                    .then(|| ("vpiCondition", Item::Events(specs, at)))
                    .into_iter()
                    .collect();
                (code, children)
            }
            IntraControl::Repeat { count, event, .. } => {
                let code = self.located("vpiRepeatControl", at);
                let mut children = vec![("vpiExpr", Item::Expr(*count))];
                if control_type(event) == Some("vpiEventControl") {
                    children.push(("vpiEventControl", Item::Control(event, at)));
                }
                (code, children)
            }
            IntraControl::Cycle { .. } | IntraControl::Unsupported { .. } => {
                let mut code = self.located("vpiDelayControl", at);
                code.flags |= IrVpiCode::OPAQUE;
                (code, Vec::new())
            }
        }
    }
}

/// The relation (and type) of an intra-assignment control.
fn control_type(control: &IntraControl) -> Option<&'static str> {
    match control {
        IntraControl::Delay(_) => Some("vpiDelayControl"),
        IntraControl::Event { .. } => Some("vpiEventControl"),
        IntraControl::Repeat { .. } => Some("vpiRepeatControl"),
        IntraControl::Cycle { .. } | IntraControl::Unsupported { .. } => None,
    }
}

/// `vpiQualifier` of an `if` or `case` (Annex M). IEEE 1800-2009 has no
/// code for `unique0`, which reports as `unique`.
fn vpi_qualifier(check: UniquePriorityCheck) -> &'static str {
    match check {
        UniquePriorityCheck::Unique | UniquePriorityCheck::Unique0 => "vpiUniqueQualifier",
        UniquePriorityCheck::Priority => "vpiPriorityQualifier",
        _ => "vpiNoQualifier",
    }
}

/// Four-state digits of an integral constant, most significant first.
/// Whether a literal is written unbased and unsized (`'0`, `'1`, `'x`,
/// `'z`; 5.7.1), so its decompiled text keeps that spelling (decision
/// A19-D11).
fn unbased_unsized(source: &ConstantSource) -> bool {
    let ConstantSource::Exact(spelling) = source else {
        return false;
    };
    let mut chars = spelling.trim().chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some('\''), Some('0' | '1' | 'x' | 'X' | 'z' | 'Z'), None)
    )
}

fn constant_digits(constant: &crate::sim::ir::IrConst) -> String {
    let bit = |plane: &[u64], index: u32| {
        plane
            .get((index / 64) as usize)
            .is_some_and(|word| word >> (index % 64) & 1 != 0)
    };
    (0..constant.width)
        .rev()
        .map(|index| {
            if bit(&constant.z, index) {
                'z'
            } else if bit(&constant.x, index) {
                'x'
            } else if bit(&constant.bits, index) {
                '1'
            } else {
                '0'
            }
        })
        .collect()
}

/// The Annex K/M `vpiOpType` of an operation, or `None` for one IEEE
/// 1800-2009 gives no code (logical equivalence, assertion operators).
pub(super) fn vpi_op_type(op: Operation) -> Option<&'static str> {
    Some(match op {
        Operation::UnaryMinus => "vpiMinusOp",
        Operation::UnaryPlus => "vpiPlusOp",
        Operation::LogicalNot => "vpiNotOp",
        Operation::BitwiseNot => "vpiBitNegOp",
        Operation::ReductionAnd => "vpiUnaryAndOp",
        Operation::ReductionNand => "vpiUnaryNandOp",
        Operation::ReductionOr => "vpiUnaryOrOp",
        Operation::ReductionNor => "vpiUnaryNorOp",
        Operation::ReductionXor => "vpiUnaryXorOp",
        Operation::ReductionXnor => "vpiUnaryXNorOp",
        Operation::Subtract => "vpiSubOp",
        Operation::Divide => "vpiDivOp",
        Operation::Modulo => "vpiModOp",
        Operation::Equal => "vpiEqOp",
        Operation::NotEqual => "vpiNeqOp",
        Operation::CaseEqual => "vpiCaseEqOp",
        Operation::CaseNotEqual => "vpiCaseNeqOp",
        Operation::Greater => "vpiGtOp",
        Operation::GreaterEqual => "vpiGeOp",
        Operation::Less => "vpiLtOp",
        Operation::LessEqual => "vpiLeOp",
        Operation::ShiftLeft => "vpiLShiftOp",
        Operation::ShiftRight => "vpiRShiftOp",
        Operation::Add => "vpiAddOp",
        Operation::Multiply => "vpiMultOp",
        Operation::LogicalAnd => "vpiLogAndOp",
        Operation::LogicalOr => "vpiLogOrOp",
        Operation::BitwiseAnd => "vpiBitAndOp",
        Operation::BitwiseOr => "vpiBitOrOp",
        Operation::BitwiseXor => "vpiBitXorOp",
        Operation::BitwiseXnor => "vpiBitXNorOp",
        Operation::Conditional => "vpiConditionOp",
        Operation::Concat => "vpiConcatOp",
        Operation::MultiConcat => "vpiMultiConcatOp",
        Operation::EventOr => "vpiEventOrOp",
        Operation::Null => "vpiNullOp",
        Operation::List => "vpiListOp",
        Operation::MinTypMax => "vpiMinTypMaxOp",
        Operation::Posedge => "vpiPosedgeOp",
        Operation::Negedge => "vpiNegedgeOp",
        Operation::ArithmeticShiftLeft => "vpiArithLShiftOp",
        Operation::ArithmeticShiftRight => "vpiArithRShiftOp",
        Operation::Power => "vpiPowerOp",
        Operation::Imply => "vpiImplyOp",
        Operation::PostIncrement => "vpiPostIncOp",
        Operation::PreIncrement => "vpiPreIncOp",
        Operation::PostDecrement => "vpiPostDecOp",
        Operation::PreDecrement => "vpiPreDecOp",
        Operation::Cast => "vpiCastOp",
        Operation::Iff => "vpiIffOp",
        Operation::WildEqual => "vpiWildEqOp",
        Operation::WildNotEqual => "vpiWildNeqOp",
        Operation::StreamLeftToRight => "vpiStreamLROp",
        Operation::StreamRightToLeft => "vpiStreamRLOp",
        Operation::AssignmentPattern => "vpiAssignmentPatternOp",
        Operation::MultiAssignmentPattern => "vpiMultiAssignmentPatternOp",
        Operation::Inside => "vpiInsideOp",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::unbased_unsized;
    use crate::core::db::ConstantSource;

    #[test]
    fn only_unbased_unsized_spellings_keep_their_fill_digit() {
        let exact = |text: &str| ConstantSource::Exact(text.to_owned());
        for text in ["'0", "'1", "'x", "'X", "'z", "'Z"] {
            assert!(unbased_unsized(&exact(text)), "{text}");
        }
        for text in ["'b1", "1'b1", "'hx", "0", "'", "\"'x\""] {
            assert!(!unbased_unsized(&exact(text)), "{text}");
        }
        assert!(!unbased_unsized(&ConstantSource::Unavailable));
    }
}
