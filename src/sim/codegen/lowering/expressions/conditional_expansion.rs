//! Conditional operators whose result is assigned by statements (SV 11.4.11):
//! containers and native records.
//!
//! A known predicate assigns one alternative straight to the target; an
//! ambiguous one evaluates both and merges them. The two uses differ in
//! destination (the target may not be written before every alternative that
//! reads it has been evaluated), so an alternative is lowered once for each.
//! A conditional that holds further expanded conditionals in its alternatives
//! would double them per level, so beyond [`CONDITIONAL_EXPANSION_LEVELS`]
//! the whole tree of nested conditionals is lowered as one sequence, in which
//! nothing appears more often than a fixed number of times:
//!
//! ```text
//! { place;
//!   { p0: 1 -> { target = a0; goto done; }  X -> { place = here; goto cold; }
//!     p1: ...
//!     target = last; goto done; }
//! cold:
//!   { kept; goto the place;
//!     the first evaluated alternative is assigned to kept, every further one
//!     merged with it, the last of all into the target; }
//! done: }
//! ```
//!
//! A known predicate therefore costs its test: `kept` and the other
//! temporaries exist only on the cold path, and what steers the jumps is a
//! [`IrJumpState`]. The alternatives of several ambiguous predicates merge left
//! to right rather than from the innermost conditional outwards. That is exact
//! because a merge yields what both operands agree on and the default
//! elsewhere, and a default merges to itself: the grouping does not matter.
//!
//! An alternative that holds many conditionals in call arguments is lowered
//! once. Beside a variable, which is read in place, that needs no `kept`
//! (`single_once`); elsewhere a known predicate that selects it enters the cold
//! path, so this is done for deeper conditionals only
//! ([`CONDITIONAL_ARGUMENT_LEVELS`]).

use super::*;
use crate::sim::ir::{IrJumpState, IrJumpStateOp};
use std::collections::HashSet;

/// A conditional operator whose alternatives hold conditionals of the same kind
/// at most this deep in all (itself included) lowers each alternative once for
/// a known and once for an ambiguous predicate. Each level doubles the code of
/// the levels below it, so the code of such a conditional grows by at most two
/// to this power; every deeper one is lowered as one sequence.
pub(in crate::sim::codegen) const CONDITIONAL_EXPANSION_LEVELS: u32 = 3;

/// An alternative or predicate that holds more levels than
/// [`CONDITIONAL_EXPANSION_LEVELS`], in call arguments say, is lowered once
/// where that costs a known predicate nothing, and otherwise in conditionals
/// deeper than this in all: there a known predicate that selects it pays for
/// the temporaries of the cold path.
pub(in crate::sim::codegen) const CONDITIONAL_ARGUMENT_LEVELS: u32 = 6;

/// How the alternatives of one expanded conditional are assigned and merged.
pub(in crate::sim::codegen) trait ExpandedConditional {
    /// Storage holding an evaluated alternative.
    type Operand;

    /// The conditional operator that alternative `arm` is, when the target
    /// can take its alternatives as if they were those of the outer one.
    fn nested(
        &mut self,
        cg: &mut Codegen<'_>,
        path: &str,
        arm: NodeId,
    ) -> Result<Option<NodeId>, String>;

    /// Assign `arm` to `dst`, or to the target.
    fn assign(
        &mut self,
        cg: &mut Codegen<'_>,
        path: &str,
        dst: Option<&Self::Operand>,
        arm: NodeId,
    ) -> Result<IrStmt, String>;

    /// Evaluate `arm` into storage that later statements of `out` may read.
    fn evaluate(
        &mut self,
        cg: &mut Codegen<'_>,
        path: &str,
        arm: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<Self::Operand, String>;

    /// Whether [`Self::evaluate`] names storage of `arm` itself, which the
    /// merge reads only after the later alternative has been evaluated.
    fn reads_in_place(&mut self, cg: &mut Codegen<'_>, arm: NodeId) -> Result<bool, String>;

    /// Declare storage for the merged alternatives of `node`.
    fn declare_kept(
        &mut self,
        cg: &mut Codegen<'_>,
        node: NodeId,
        out: &mut Vec<IrStmt>,
    ) -> Result<Self::Operand, String>;

    /// Merge two evaluated alternatives into `dst`, or into the target.
    fn merge(
        &mut self,
        cg: &mut Codegen<'_>,
        path: &str,
        dst: Option<&Self::Operand>,
        left: &Self::Operand,
        right: &Self::Operand,
    ) -> Result<Vec<IrStmt>, String>;

    /// Evaluate both alternatives and merge them into the target.
    fn merge_alternatives(
        &mut self,
        cg: &mut Codegen<'_>,
        path: &str,
        if_true: NodeId,
        if_false: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let mut statements = Vec::new();
        let left = self.evaluate(cg, path, if_true, &mut statements)?;
        let right = self.evaluate(cg, path, if_false, &mut statements)?;
        statements.extend(self.merge(cg, path, None, &left, &right)?);
        Ok(statements)
    }

    /// The target, if nothing but the statement being lowered can see it.
    fn private_target(&self, cg: &Codegen<'_>) -> Option<Self::Operand>;

    /// Copy `src` to `dst`, or to the target.
    fn copy(
        &mut self,
        cg: &mut Codegen<'_>,
        path: &str,
        dst: Option<&Self::Operand>,
        src: &Self::Operand,
    ) -> Result<IrStmt, String>;
}

/// An alternative of a sequence.
enum Alternative {
    /// Lowered for each use.
    Leaf(NodeId),
    /// Holds expanded conditionals of its own (in call arguments, say) too
    /// deep to repeat, so it is lowered for the cold path alone and a known
    /// predicate that selects it goes there, to the place given.
    Once(NodeId, u32),
    Nested(Box<Conditional>),
}

/// A conditional operator of a sequence. Places are numbered in source order.
struct Conditional {
    node: NodeId,
    /// The predicate holds expanded conditionals: it is lowered for the
    /// ambiguous path alone, which a known predicate enters at this place.
    predicate: Option<u32>,
    /// Where the path of an ambiguous predicate starts: before the first
    /// alternative.
    ambiguous: u32,
    if_true: Alternative,
    /// The last place of `if_true`.
    true_end: u32,
    /// `if_true` is read in place and what `if_false` evaluates may write
    /// it: it is read after that.
    true_read_last: bool,
    if_false: Alternative,
}

impl Conditional {
    /// The first and the last place of the first alternative.
    fn places_of_true(&self) -> (u32, u32) {
        (self.ambiguous + 1, self.true_end)
    }
}

/// The predicate nodes and the two alternatives of a conditional operator.
fn conditional_parts(kind: &NodeKind) -> Option<(Vec<NodeId>, NodeId, NodeId)> {
    match kind {
        NodeKind::Expr(ExprKind::Conditional {
            predicate,
            if_true,
            if_false,
        }) => Some((
            predicate
                .clauses
                .iter()
                .flat_map(|clause| std::iter::once(clause.expression).chain(clause.pattern))
                .collect(),
            *if_true,
            *if_false,
        )),
        NodeKind::Expr(ExprKind::Operation {
            op: Operation::Conditional,
            operands,
            ..
        }) => match operands[..] {
            [condition, if_true, if_false] => Some((vec![condition], if_true, if_false)),
            _ => None,
        },
        _ => None,
    }
}

impl Codegen<'_> {
    /// Whether `node` is a conditional operator.
    pub(in crate::sim::codegen) fn is_conditional(&self, node: NodeId) -> bool {
        conditional_parts(self.kind(node)).is_some()
    }

    /// Whether `node` is a conditional operator of a type that only
    /// statements assign.
    fn is_expanded_conditional(&self, node: NodeId) -> bool {
        self.is_conditional(node)
            && (self.native_value_type(node).is_some()
                || self
                    .query_descriptor(node)
                    .is_some_and(|descriptor| match &descriptor.shape {
                        TypeShape::Container { .. } => true,
                        TypeShape::FixedArray { .. } => {
                            Self::fixed_descriptor_width_bits(descriptor).is_none()
                        }
                        _ => false,
                    }))
    }

    /// How often an alternative nested in `node` would be repeated: the
    /// number of expanded conditionals on the deepest path through
    /// alternatives.
    fn expansion_levels(&self, node: NodeId) -> u32 {
        // Every conditional around `node` asks; the types that decide are the
        // database's.
        if let Some(levels) = self.expansion_levels.borrow().get(&node) {
            return *levels;
        }
        let levels = self.find_expansion_levels(node);
        self.expansion_levels.borrow_mut().insert(node, levels);
        levels
    }

    fn find_expansion_levels(&self, node: NodeId) -> u32 {
        if self.is_expanded_conditional(node) {
            if let Some((predicate, if_true, if_false)) = conditional_parts(self.kind(node)) {
                let alternatives = self
                    .expansion_levels(if_true)
                    .max(self.expansion_levels(if_false));
                return predicate
                    .into_iter()
                    .map(|node| self.expansion_levels(node))
                    .fold(alternatives + 1, u32::max);
            }
        }
        self.node(node)
            .children
            .iter()
            .map(|child| self.expansion_levels(*child))
            .fold(0, u32::max)
    }

    /// The value of the predicate of conditional operator `node`.
    fn lower_conditional_selector(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Conditional { predicate, .. }) => {
                self.lower_conditional_predicate(path, predicate)
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Conditional,
                operands,
                ..
            }) if operands.len() == 3 => self.lower_boolean_expr(path, operands[0]),
            _ => Err(format!("conditional operator in `{path}` is malformed")),
        }
    }

    /// Assign conditional operator `node` through `sink`.
    pub(in crate::sim::codegen) fn expand_conditional<S: ExpandedConditional>(
        &mut self,
        path: &str,
        sink: &mut S,
        node: NodeId,
    ) -> Result<IrStmt, String> {
        let (_, if_true, if_false) = conditional_parts(self.kind(node))
            .ok_or_else(|| format!("conditional operator in `{path}` is malformed"))?;
        let nested = self
            .expansion_levels(if_true)
            .max(self.expansion_levels(if_false));
        let direct = sink.nested(self, path, if_true)?.is_some()
            || sink.nested(self, path, if_false)?.is_some();
        let deep = self.lowered_once(if_true) || self.lowered_once(if_false);
        let once = self.expansion_levels(node) > CONDITIONAL_ARGUMENT_LEVELS;
        if nested < CONDITIONAL_EXPANSION_LEVELS || !direct && !deep {
            return self.repeat_alternatives(path, sink, node, if_true, if_false);
        }
        if !direct {
            if let Some(statement) = self.single_once(path, sink, node, if_true, if_false)? {
                return Ok(statement);
            }
            if !once {
                return self.repeat_alternatives(path, sink, node, if_true, if_false);
            }
        }
        self.sequence_alternatives(path, sink, node, once)
    }

    /// Whether `node` holds more conditionals than may be repeated.
    fn lowered_once(&self, node: NodeId) -> bool {
        self.expansion_levels(node) > CONDITIONAL_EXPANSION_LEVELS
    }

    /// The short form: each alternative once for a known and once for an
    /// ambiguous predicate.
    fn repeat_alternatives<S: ExpandedConditional>(
        &mut self,
        path: &str,
        sink: &mut S,
        node: NodeId,
        if_true: NodeId,
        if_false: NodeId,
    ) -> Result<IrStmt, String> {
        let selector = self.lower_conditional_selector(path, node)?;
        let take_true = sink.assign(self, path, None, if_true)?;
        let take_false = sink.assign(self, path, None, if_false)?;
        let merge = sink.merge_alternatives(self, path, if_true, if_false)?;
        Ok(predicate_switch(
            selector,
            vec![take_true],
            vec![take_false],
            merge,
        ))
    }

    /// The short form of a conditional with one alternative that holds many
    /// conditionals and another that is not kept while that one is evaluated:
    /// the target itself receives it, or the temporary that a merge needs.
    fn single_once<S: ExpandedConditional>(
        &mut self,
        path: &str,
        sink: &mut S,
        node: NodeId,
        if_true: NodeId,
        if_false: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let first = self.lowered_once(if_true);
        let (once, other) = if first {
            (if_true, if_false)
        } else {
            (if_false, if_true)
        };
        if self.lowered_once(other) || !first && !sink.reads_in_place(self, other)? {
            return Ok(None);
        }
        let ambiguous = self.expansion_name("ambiguous");
        let done = self.expansion_name("done");
        let selector = self
            .lower_conditional_selector(path, node)
            .map_err(|error| self.locate_error(node, error))?;
        let taken = vec![
            sink.assign(self, path, None, other)?,
            IrStmt::Goto(done.clone()),
        ];
        let (if_known_true, if_known_false) = if first {
            (Vec::new(), taken)
        } else {
            (taken, Vec::new())
        };
        let mut statements = Vec::new();
        let target = sink.private_target(self);
        let value = match &target {
            Some(target) => {
                statements.push(sink.assign(self, path, None, once)?);
                target
            }
            None => &sink.evaluate(self, path, once, &mut statements)?,
        };
        let single = self.expansion_name("single");
        statements.push(jump_state(
            &ambiguous,
            IrJumpStateOp::GotoIf {
                first: 0,
                last: 0,
                label: single.clone(),
            },
        ));
        let mut merge = Vec::new();
        let other = sink.evaluate(self, path, other, &mut merge)?;
        let (left, right) = if first {
            (value, &other)
        } else {
            (&other, value)
        };
        merge.extend(sink.merge(self, path, None, left, right)?);
        if target.is_none() {
            merge.push(IrStmt::Goto(done.clone()));
        }
        statements.push(IrStmt::Block(merge));
        statements.push(IrStmt::Label(single));
        if target.is_none() {
            let copy = sink.copy(self, path, None, value)?;
            statements.push(IrStmt::Block(vec![copy]));
        }
        Ok(Some(IrStmt::Block(vec![
            jump_state(&ambiguous, IrJumpStateOp::Declare),
            predicate_switch(
                selector,
                if_known_true,
                if_known_false,
                vec![jump_state(&ambiguous, IrJumpStateOp::Set(1))],
            ),
            IrStmt::Block(statements),
            IrStmt::Label(done),
        ])))
    }

    fn sequence_alternatives<S: ExpandedConditional>(
        &mut self,
        path: &str,
        sink: &mut S,
        node: NodeId,
        once: bool,
    ) -> Result<IrStmt, String> {
        let mut sequence = Sequence {
            path,
            sink,
            once,
            place: self.expansion_name("place"),
            places: 0,
            entered: HashSet::new(),
            single: false,
            states: Vec::new(),
            cold: self.expansion_name("cold"),
            done: self.expansion_name("done"),
        };
        let root = sequence.conditional(self, node)?;
        let mut known = Vec::new();
        sequence.known(self, &root, &mut known)?;
        let cold = sequence.cold_path(self, &root)?;
        let mut statements: Vec<_> = sequence
            .states
            .iter()
            .map(|state| jump_state(state, IrJumpStateOp::Declare))
            .collect();
        statements.extend(blocks_between_labels(known));
        statements.extend([
            IrStmt::Label(sequence.cold),
            IrStmt::Block(cold),
            IrStmt::Label(sequence.done),
        ]);
        Ok(IrStmt::Block(statements))
    }

    /// A fresh name for a jump state or label of one expansion.
    fn expansion_name(&mut self, tag: &str) -> String {
        let sequence = self.native_copy_sequence;
        self.native_copy_sequence += 1;
        format!("_llg_cond_{tag}_{sequence}")
    }
}

/// A conditional tree being lowered as one sequence.
struct Sequence<'e, S> {
    path: &'e str,
    sink: &'e mut S,
    /// The jump state naming the place the cold path starts from.
    place: String,
    /// Number of places.
    places: u32,
    /// Alternatives and predicates that hold many conditionals are lowered
    /// once.
    once: bool,
    /// Places the known path enters the cold one at.
    entered: HashSet<u32>,
    /// The alternative that finishes the target may be the only one: a place
    /// precedes a predicate or an alternative a known predicate selected, or
    /// an alternative is read after later ones.
    single: bool,
    /// Further jump states of the cold path.
    states: Vec<String>,
    /// Label of the path of ambiguous predicates.
    cold: String,
    /// Label after the whole sequence.
    done: String,
}

/// The path of ambiguous predicates under construction.
struct ColdPath<O> {
    /// The jumps to the entered places.
    entries: Vec<IrStmt>,
    body: Vec<IrStmt>,
    /// Labels some jump names; the others are dropped.
    targets: HashSet<String>,
    /// States that some jump depends on.
    tested: HashSet<String>,
    kept: O,
    /// The state telling whether `kept` holds an alternative.
    filled: String,
    /// The code of an alternative precedes.
    started: bool,
    /// The state telling, after a first alternative without conditionals,
    /// whether its predicate was true. The first conditional that asks makes
    /// it.
    selected: Option<String>,
    /// Label where `kept` is complete.
    end: String,
    /// The last alternative assigned the target itself.
    direct: bool,
}

impl<O> ColdPath<O> {
    fn goto(&mut self, label: &str) -> IrStmt {
        self.targets.insert(label.to_owned());
        IrStmt::Goto(label.to_owned())
    }

    fn goto_if(&mut self, state: &str, first: u32, last: u32, label: &str) -> IrStmt {
        self.targets.insert(label.to_owned());
        self.tested.insert(state.to_owned());
        jump_state(
            state,
            IrJumpStateOp::GotoIf {
                first,
                last,
                label: label.to_owned(),
            },
        )
    }

    fn label(&mut self, label: String) {
        self.body.push(IrStmt::Label(label));
    }
}

impl<S: ExpandedConditional> Sequence<'_, S> {
    fn new_place(&mut self) -> u32 {
        let place = self.places;
        self.places += 1;
        place
    }

    fn new_state(&mut self, cg: &mut Codegen<'_>, tag: &str) -> String {
        let state = cg.expansion_name(tag);
        self.states.push(state.clone());
        state
    }

    /// The tree of nested conditionals below `node`, with its places.
    fn conditional(&mut self, cg: &mut Codegen<'_>, node: NodeId) -> Result<Conditional, String> {
        let (predicate, if_true, if_false) = conditional_parts(cg.kind(node))
            .ok_or_else(|| format!("conditional operator in `{}` is malformed", self.path))?;
        // The predicate of the root, which no place precedes, is lowered once
        // whatever it holds.
        let predicate = (self.places > 0
            && predicate.iter().any(|node| self.lowered_once(cg, *node)))
        .then(|| self.new_place());
        let ambiguous = self.new_place();
        let pure_false = cg.side_effect_free(if_false);
        let if_true = self.alternative(cg, if_true)?;
        let true_end = self.places - 1;
        let true_read_last = match if_true {
            Alternative::Leaf(arm) if !pure_false => self.sink.reads_in_place(cg, arm)?,
            _ => false,
        };
        self.single |= true_read_last;
        let if_false = self.alternative(cg, if_false)?;
        Ok(Conditional {
            node,
            predicate,
            ambiguous,
            if_true,
            true_end,
            true_read_last,
            if_false,
        })
    }

    fn lowered_once(&self, cg: &Codegen<'_>, node: NodeId) -> bool {
        self.once && cg.lowered_once(node)
    }

    fn alternative(&mut self, cg: &mut Codegen<'_>, arm: NodeId) -> Result<Alternative, String> {
        if let Some(node) = self.sink.nested(cg, self.path, arm)? {
            return Ok(Alternative::Nested(Box::new(self.conditional(cg, node)?)));
        }
        Ok(if self.lowered_once(cg, arm) {
            Alternative::Once(arm, self.new_place())
        } else {
            Alternative::Leaf(arm)
        })
    }

    fn selector(&self, cg: &mut Codegen<'_>, node: NodeId) -> Result<IrExpr, String> {
        cg.lower_conditional_selector(self.path, node)
            .map_err(|error| cg.locate_error(node, error))
    }

    /// Continue on the cold path at `place`.
    fn enter(&mut self, place: u32) -> Vec<IrStmt> {
        self.entered.insert(place);
        let mut statements = Vec::new();
        // The state starts at the place of the root.
        if place != 0 {
            statements.push(jump_state(&self.place, IrJumpStateOp::Set(place)));
        }
        statements.push(IrStmt::Goto(self.cold.clone()));
        statements
    }

    /// `conditional` while every predicate so far was known.
    fn known(
        &mut self,
        cg: &mut Codegen<'_>,
        conditional: &Conditional,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        if let Some(place) = conditional.predicate {
            self.single = true;
            out.extend(self.enter(place));
            return Ok(());
        }
        let selector = self.selector(cg, conditional.node)?;
        let ambiguous = match (&conditional.if_true, &conditional.if_false) {
            (Alternative::Leaf(if_true), Alternative::Leaf(if_false)) => {
                let mut merge = self
                    .sink
                    .merge_alternatives(cg, self.path, *if_true, *if_false)?;
                merge.push(IrStmt::Goto(self.done.clone()));
                merge
            }
            _ => self.enter(conditional.ambiguous),
        };
        if matches!(conditional.if_true, Alternative::Nested(_)) {
            // Beside the test, not inside it: a jump out of nested tests would
            // release the selector of each.
            let if_false = cg.expansion_name("false");
            let skip = vec![IrStmt::Goto(if_false.clone())];
            out.push(predicate_switch(selector, Vec::new(), skip, ambiguous));
            self.known_alternative(cg, &conditional.if_true, out)?;
            out.push(IrStmt::Label(if_false));
        } else {
            let mut taken = Vec::new();
            self.known_alternative(cg, &conditional.if_true, &mut taken)?;
            out.push(predicate_switch(selector, taken, Vec::new(), ambiguous));
        }
        self.known_alternative(cg, &conditional.if_false, out)
    }

    fn known_alternative(
        &mut self,
        cg: &mut Codegen<'_>,
        alternative: &Alternative,
        out: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        match alternative {
            Alternative::Leaf(arm) => {
                out.push(self.sink.assign(cg, self.path, None, *arm)?);
                out.push(IrStmt::Goto(self.done.clone()));
            }
            Alternative::Once(_, place) => {
                self.single = true;
                out.extend(self.enter(*place));
            }
            Alternative::Nested(conditional) => self.known(cg, conditional, out)?,
        }
        Ok(())
    }

    /// The statements that run once a predicate was ambiguous (or a part
    /// lowered for this path alone was reached).
    fn cold_path(
        &mut self,
        cg: &mut Codegen<'_>,
        root: &Conditional,
    ) -> Result<Vec<IrStmt>, String> {
        let mut statements = Vec::new();
        let kept = self.sink.declare_kept(cg, root.node, &mut statements)?;
        let mut path = ColdPath {
            entries: Vec::new(),
            body: Vec::new(),
            targets: HashSet::new(),
            tested: HashSet::new(),
            kept,
            filled: self.new_state(cg, "filled"),
            started: false,
            selected: None,
            end: cg.expansion_name("end"),
            direct: false,
        };
        let end = path.end.clone();
        let exit = Exit {
            label: &end,
            end: LeafEnd::Last,
        };
        self.cold_conditional(cg, root, true, exit, &mut path)?;
        let ColdPath {
            entries,
            body,
            targets,
            tested,
            kept,
            direct,
            ..
        } = path;
        if tested.contains(&self.place) {
            self.states.push(self.place.clone());
        }
        statements.push(IrStmt::Block(entries));
        statements.extend(blocks_between_labels(body.into_iter().filter(
            |statement| !matches!(statement, IrStmt::Label(label) if !targets.contains(label)),
        )));
        let reached = targets.contains(&end);
        if reached {
            statements.push(IrStmt::Label(end));
        }
        if reached || !direct {
            statements.push(self.sink.copy(cg, self.path, None, &kept)?);
        }
        Ok(statements)
    }

    /// Start at `label` when the known path entered at `place`.
    fn entry(&self, place: u32, label: &str, path: &mut ColdPath<S::Operand>) {
        if self.entered.contains(&place) {
            let jump = path.goto_if(&self.place, place, place, label);
            path.entries.push(jump);
        }
    }

    /// `conditional` on the cold path, which goes on at `exit`.
    fn cold_conditional(
        &mut self,
        cg: &mut Codegen<'_>,
        conditional: &Conditional,
        root: bool,
        exit: Exit<'_>,
        path: &mut ColdPath<S::Operand>,
    ) -> Result<(), String> {
        // The predicate of the root is never true here. The conditionals nested
        // in a first alternative use the common state, so this one keeps its
        // predicate in a state of its own.
        let selected = match conditional.if_true {
            _ if root => None,
            Alternative::Nested(_) => Some(self.new_state(cg, "selected")),
            Alternative::Leaf(_) | Alternative::Once(..) => {
                if path.selected.is_none() {
                    path.selected = Some(self.new_state(cg, "selected"));
                }
                path.selected.clone()
            }
        };
        let if_false = cg.expansion_name("false");
        // The root is entered after its predicate, which is not repeated.
        if let Some(selected) = &selected {
            if let Some(place) = conditional.predicate {
                let label = cg.expansion_name("test");
                self.entry(place, &label, path);
                path.label(label);
            }
            let selector = self.selector(cg, conditional.node)?;
            let skip = path.goto(&if_false);
            path.body.push(predicate_switch(
                selector,
                vec![jump_state(selected, IrJumpStateOp::Set(1))],
                vec![skip],
                vec![jump_state(selected, IrJumpStateOp::Set(0))],
            ));
            let label = cg.expansion_name("either");
            self.entry(conditional.ambiguous, &label, path);
            path.label(label);
        }
        let reached = conditional
            .true_read_last
            .then(|| self.new_state(cg, "read"));
        let exit_false = if let Some(reached) = &reached {
            path.body.push(jump_state(reached, IrJumpStateOp::Set(1)));
            cg.expansion_name("late")
        } else {
            let after_true = cg.expansion_name("after");
            let end = match (&conditional.if_true, exit.end, &selected) {
                (Alternative::Nested(_), ..) => LeafEnd::LastWithin(conditional),
                (_, LeafEnd::Last, Some(selected)) => LeafEnd::LastIf(selected),
                _ => LeafEnd::Kept,
            };
            let after_true = Exit {
                label: &after_true,
                end,
            };
            self.cold_alternative(cg, &conditional.if_true, after_true, path)?;
            path.label(after_true.label.to_owned());
            exit.label.to_owned()
        };
        if let Some(selected) = &selected {
            let leave = path.goto_if(selected, 1, 1, &exit_false);
            path.body.push(leave);
        }
        // Entered within the alternative, after a true predicate.
        let (first, last) = conditional.places_of_true();
        if first <= last {
            let leave = path.goto_if(&self.place, first, last, &exit_false);
            path.body.push(leave);
        }
        path.label(if_false);
        let false_exit = Exit {
            label: &exit_false,
            end: match reached {
                Some(_) => LeafEnd::Kept,
                None => exit.end,
            },
        };
        self.cold_alternative(cg, &conditional.if_false, false_exit, path)?;
        if let (Some(reached), Alternative::Leaf(arm)) = (reached, &conditional.if_true) {
            path.label(exit_false);
            let skipped = cg.expansion_name("skipped");
            let skip = path.goto_if(&reached, 0, 0, &skipped);
            path.body.push(skip);
            self.cold_leaf(cg, *arm, None, exit.end, path)?;
            path.label(skipped);
            path.direct = false;
        }
        Ok(())
    }

    fn cold_alternative(
        &mut self,
        cg: &mut Codegen<'_>,
        alternative: &Alternative,
        exit: Exit<'_>,
        path: &mut ColdPath<S::Operand>,
    ) -> Result<(), String> {
        let (arm, once) = match alternative {
            Alternative::Nested(conditional) => {
                return self.cold_conditional(cg, conditional, false, exit, path)
            }
            Alternative::Leaf(arm) => (*arm, None),
            Alternative::Once(arm, place) => {
                let label = cg.expansion_name("arm");
                self.entry(*place, &label, path);
                path.label(label);
                (*arm, Some(*place))
            }
        };
        self.cold_leaf(cg, arm, once, exit.end, path)
    }

    /// Evaluate `arm`, at place `once` when it is lowered once, and add it to
    /// `kept` or finish the target with it.
    fn cold_leaf(
        &mut self,
        cg: &mut Codegen<'_>,
        arm: NodeId,
        once: Option<u32>,
        end: LeafEnd<'_>,
        path: &mut ColdPath<S::Operand>,
    ) -> Result<(), String> {
        let last = matches!(end, LeafEnd::Last);
        let (full, added) = (cg.expansion_name("full"), cg.expansion_name("added"));
        let finish = |path: &mut ColdPath<S::Operand>, done: &str| {
            if last {
                IrStmt::Goto(done.to_owned())
            } else {
                path.goto(&added)
            }
        };
        let mut statements = Vec::new();
        let mut value = None;
        if let Some(place) = once {
            let operand = self.sink.evaluate(cg, self.path, arm, &mut statements)?;
            // Entered at it, after known predicates alone.
            if !last {
                let shared = cg.expansion_name("shared");
                statements.push(path.goto_if(&self.place, 0, place - 1, &shared));
                statements.push(path.goto_if(&self.place, place + 1, u32::MAX, &shared));
                let copy = self.sink.copy(cg, self.path, None, &operand)?;
                statements.push(IrStmt::Block(vec![copy, IrStmt::Goto(self.done.clone())]));
                statements.push(IrStmt::Label(shared));
            }
            value = Some(operand);
        }
        // Nothing precedes the first alternative.
        if !std::mem::replace(&mut path.started, true) && !last {
            statements.push(match &value {
                Some(operand) => self.sink.copy(cg, self.path, Some(&path.kept), operand)?,
                None => self.sink.assign(cg, self.path, Some(&path.kept), arm)?,
            });
            statements.push(jump_state(&path.filled, IrJumpStateOp::Set(1)));
            path.body.push(IrStmt::Block(statements));
            path.direct = false;
            return Ok(());
        }
        if self.single || !last {
            let mut empty = vec![path.goto_if(&path.filled.clone(), 1, 1, &full)];
            let kept = (!last).then_some(&path.kept);
            empty.push(match &value {
                Some(operand) => self.sink.copy(cg, self.path, kept, operand)?,
                None => self.sink.assign(cg, self.path, kept, arm)?,
            });
            if !last {
                empty.push(jump_state(&path.filled, IrJumpStateOp::Set(1)));
            }
            empty.push(finish(path, &self.done));
            statements.push(IrStmt::Block(empty));
            statements.push(IrStmt::Label(full));
        }
        let mut merged = Vec::new();
        let value = match value {
            Some(value) => value,
            None => self.sink.evaluate(cg, self.path, arm, &mut merged)?,
        };
        let last_if = match end {
            LeafEnd::LastIf(selected) => Some((selected, (1, 1))),
            LeafEnd::LastWithin(within) => Some((self.place.as_str(), within.places_of_true())),
            LeafEnd::Kept | LeafEnd::Last => None,
        };
        let direct = last_if.map(|(state, (first, last))| {
            let label = cg.expansion_name("last");
            merged.push(path.goto_if(state, first, last, &label));
            label
        });
        let kept = &path.kept;
        let dst = (!last).then_some(kept);
        let mut merge = self.sink.merge(cg, self.path, dst, kept, &value)?;
        merge.push(finish(path, &self.done));
        merged.push(IrStmt::Block(merge));
        if let Some(label) = direct {
            let mut merge = self.sink.merge(cg, self.path, None, &path.kept, &value)?;
            merge.push(IrStmt::Goto(self.done.clone()));
            merged.push(IrStmt::Label(label));
            merged.push(IrStmt::Block(merge));
        }
        statements.push(IrStmt::Block(merged));
        if !last {
            statements.push(IrStmt::Label(added));
        }
        path.body.push(IrStmt::Block(statements));
        path.direct = last;
        Ok(())
    }
}

/// Where the cold path goes on after an alternative.
#[derive(Clone, Copy)]
struct Exit<'c> {
    label: &'c str,
    end: LeafEnd<'c>,
}

/// What follows an alternative of the cold path.
#[derive(Clone, Copy)]
enum LeafEnd<'c> {
    /// Further code; the alternative joins `kept`.
    Kept,
    /// Nothing when the state is 1: it joins `kept` into the target then.
    LastIf(&'c str),
    /// Nothing when the known path entered in the first alternative of the
    /// conditional, so that every predicate around it was known.
    LastWithin(&'c Conditional),
    /// Nothing.
    Last,
}

/// No label shares a block with what a jump to it passes: a statement may
/// declare C variables.
fn blocks_between_labels(statements: impl IntoIterator<Item = IrStmt>) -> Vec<IrStmt> {
    let mut blocks = Vec::new();
    let mut run = Vec::new();
    for statement in statements {
        if matches!(statement, IrStmt::Label(_)) {
            if !run.is_empty() {
                blocks.push(IrStmt::Block(std::mem::take(&mut run)));
            }
            blocks.push(statement);
        } else {
            run.push(statement);
        }
    }
    blocks.push(IrStmt::Block(run));
    blocks
}

fn jump_state(name: &str, operation: IrJumpStateOp) -> IrStmt {
    IrStmt::JumpState(Box::new(IrJumpState {
        name: name.to_owned(),
        operation,
    }))
}

/// The truth of predicate value `selector` in one bit: 0, 1 or X.
fn truth(selector: IrExpr) -> IrExpr {
    let not = |a: IrExpr| {
        IrExpr::new(
            IrExprKind::Un {
                op: IrUnOp::LogNot,
                a: Box::new(a),
            },
            1,
            false,
            None,
        )
    };
    if selector.width == 1 && !selector.signed && selector.fill.is_none() {
        selector
    } else {
        not(not(selector))
    }
}

/// `selector` true, false or ambiguous, each with its statements.
fn predicate_switch(
    selector: IrExpr,
    if_true: Vec<IrStmt>,
    if_false: Vec<IrStmt>,
    ambiguous: Vec<IrStmt>,
) -> IrStmt {
    IrStmt::Case {
        sel: truth(selector),
        kind: IrCaseKind::Exact,
        items: vec![
            IrCaseItem::new(vec![const_bits_expr(1, true)], if_true),
            IrCaseItem::new(vec![const_bits_expr(1, false)], if_false),
            IrCaseItem::new(Vec::new(), ambiguous),
        ],
        check: IrUniquePriorityCheck::None,
    }
}

#[cfg(test)]
mod tests;
