//! Statements.

use super::*;

#[derive(Debug)]
pub enum StmtKind {
    Begin,
    /// An immediate assertion with owned condition and action branches.
    /// Deferred/final metadata is retained so unsupported forms fail closed
    /// during simulator lowering instead of becoming ordinary assertions.
    ImmediateAssertion {
        kind: ImmediateAssertionKind,
        cond: NodeId,
        if_true: Option<NodeId>,
        if_false: Option<NodeId>,
        label: String,
        deferred: bool,
        is_final: bool,
    },
    /// A concurrent property assertion. The property graph is separate from
    /// ordinary statement/expression IR so sampled evaluation retains its
    /// clock, disable, attempt, and declaration identity.
    ConcurrentAssertion {
        kind: ConcurrentAssertionKind,
        property: NodeId,
        if_true: Option<NodeId>,
        if_false: Option<NodeId>,
        label: String,
    },
    IfElse {
        cond: NodeId,
        check: UniquePriorityCheck,
    },
    Assign {
        blocking: bool,
        op: Operation,
        /// Intra-assignment control (`a = #5 b;`, `a <= #5 b;`, and
        /// event/repeat forms) — see
        /// [`IntraControl`].  `None` when the assignment has none.
        delay: Option<IntraControl>,
    },
    Case {
        case_type: CaseKind,
        check: UniquePriorityCheck,
        items: Vec<CaseItem>,
    },
    For {
        /// Variables declared in the initializer (`for (int i = ...; ...)`).
        vars: Vec<NodeId>,
        init: Vec<NodeId>,
        cond: NodeId,
        incr: Vec<NodeId>,
        body: NodeId,
    },
    While {
        cond: NodeId,
        body: NodeId,
    },
    /// SystemVerilog post-test loop: execute `body`, then repeat while
    /// `cond` is true.
    DoWhile {
        cond: NodeId,
        body: NodeId,
    },
    Repeat {
        cond: NodeId,
        body: NodeId,
    },
    Forever {
        body: NodeId,
    },
    EventControl {
        specs: Vec<EventSpec>,
        /// `true` for `@*` / `always_comb` with no explicit sensitivity.
        implicit: bool,
        body: Option<NodeId>,
    },
    DelayControl {
        delay: NodeId,
    },
    /// `##count` waits for events of the resolved default clocking block.
    CycleDelayControl {
        count: NodeId,
    },
    EventTrigger {
        blocking: bool,
        target: Option<NodeId>,
        timing: Option<EventTriggerTiming>,
    },
    /// `wait (cond) stmt` — suspend until `cond` is true, then run the body.
    /// The (optional) body statement is captured as a child node.
    Wait {
        cond: NodeId,
    },
    /// `wait_order (...) action else failure` — suspend until canonical event
    /// objects arrive in order, retaining both action statements.
    WaitOrder {
        events: Vec<NodeId>,
        if_true: Option<NodeId>,
        if_false: Option<NodeId>,
    },
    /// `force lhs = rhs` — force a net/var until released or deassigned.
    Force {
        lhs: NodeId,
        rhs: NodeId,
    },
    /// `release lhs` — cancel a procedural force on `lhs`.
    Release {
        lhs: NodeId,
    },
    /// `deassign lhs` — cancel a procedural continuous assignment on `lhs`.
    Deassign {
        lhs: NodeId,
    },
    ProcContAssign {
        lhs: NodeId,
        rhs: NodeId,
    },
    /// A declaration statement at its executable lexical position. The
    /// declaration node owns its initializer through [`Db::var_initializer`].
    VariableDecl {
        declaration: NodeId,
    },
    Empty,
    Return {
        value: Option<NodeId>,
    },
    Fork {
        /// Resolved declaration identity for a named fork scope. Anonymous
        /// fork statements carry no target.
        target: Option<NodeId>,
        join_kind: JoinKind,
        branches: Vec<NodeId>,
    },
    /// `wait fork;` — suspend until every live fork group of the current
    /// process has completed.  Atomic: no children.
    WaitFork,
    /// `disable fork;` — kill every descendant of the current process.
    /// Atomic: no children.
    DisableFork,
    Disable {
        target: Option<NodeId>,
    },
    /// `break;` inside a loop (1800-2005 §12.7).  Atomic: no children.
    Break,
    /// `continue;` inside a loop (1800-2005 §12.7).  Atomic: no children.
    Continue,
    /// `foreach (array[index, ...]) body` loop. The array target is resolved
    /// against an already-captured declaration; iterator variables belong to
    /// this statement's lexical scope. `None` entries preserve omitted
    /// dimensions, including omitted trailing dimensions.
    Foreach {
        array: Option<NodeId>,
        vars: Vec<Option<NodeId>>,
        body: NodeId,
    },
    Unsupported {
        object_type: ObjectType,
    },
}

/// Ordered propagation-delay expressions on a continuous assignment or primitive.
#[derive(Clone, Copy, Debug)]
pub enum DriverDelay {
    /// One expression supplies every transition delay.
    Single(NodeId),
    /// Separate rise and fall expressions; turn-off is their minimum.
    RiseFall(NodeId, NodeId),
    /// Separate rise, fall and turn-off expressions, in that order.
    RiseFallTurnOff(NodeId, NodeId, NodeId),
}

#[derive(Debug)]
pub enum IntraControl {
    /// Delay expression evaluated in the assignment's owning scope.
    Delay(NodeId),
    /// One event control (`@(posedge a or ev)`) retained as owned event specs.
    Event {
        control: NodeId,
        specs: Vec<EventSpec>,
        implicit: bool,
    },
    /// A repeat event control (`repeat (n) @(...)`).  The nested control is
    /// retained so lowering never has to recover syntax or source text.
    Repeat {
        control: NodeId,
        count: NodeId,
        event: Box<IntraControl>,
    },
    /// A cycle delay in an assignment timing control. The count is resolved
    /// against the owning scope's default clocking event during lowering.
    Cycle { control: NodeId, count: NodeId },
    /// A timing form known to the frontend but not yet executable by the
    /// simulator.  Keeping its identity gives consumers a source-located
    /// diagnostic instead of silently treating it as an untimed assignment.
    Unsupported { control: NodeId },
}

impl IntraControl {
    pub(crate) fn referenced_nodes(&self, nodes: &mut Vec<NodeId>) {
        match self {
            Self::Delay(delay) => nodes.push(*delay),
            Self::Event { control, specs, .. } => {
                nodes.push(*control);
                for spec in specs {
                    spec.referenced_nodes(nodes);
                }
            }
            Self::Repeat {
                control,
                count,
                event,
            } => {
                nodes.extend([*control, *count]);
                event.referenced_nodes(nodes);
            }
            Self::Cycle { control, count } => nodes.extend([*control, *count]),
            Self::Unsupported { control } => nodes.push(*control),
        }
    }
}

/// Timing attached to a nonblocking named-event trigger (`->> timing ev`).
///
/// The timing control remains an owned semantic value until simulator
/// lowering.  Unsupported timing nodes are retained so a consumer can issue
/// a source-located rejection instead of silently treating them as an
/// immediate trigger.
#[derive(Debug)]
pub enum EventTriggerTiming {
    Delay {
        control: NodeId,
        expression: NodeId,
    },
    Event {
        control: NodeId,
        specs: Vec<EventSpec>,
        implicit: bool,
    },
    Repeat {
        control: NodeId,
        count: NodeId,
        event: Box<EventTriggerTiming>,
    },
    Unsupported {
        control: NodeId,
    },
}

impl EventTriggerTiming {
    pub(crate) fn referenced_nodes(&self, nodes: &mut Vec<NodeId>) {
        match self {
            Self::Delay {
                control,
                expression,
            } => nodes.extend([*control, *expression]),
            Self::Event { control, specs, .. } => {
                nodes.push(*control);
                for spec in specs {
                    spec.referenced_nodes(nodes);
                }
            }
            Self::Repeat {
                control,
                count,
                event,
            } => {
                nodes.extend([*control, *count]);
                event.referenced_nodes(nodes);
            }
            Self::Unsupported { control } => nodes.push(*control),
        }
    }
}

/// One sensitivity entry of an event control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventSpec {
    /// An event whose qualifier is sampled when its source triggers.
    Qualified {
        event: Box<EventSpec>,
        condition: NodeId,
    },
    Edge {
        sig: NodeId,
        posedge: bool,
    },
    AnyChange {
        sig: NodeId,
    },
    /// A named event (`@(ev)`) — the value is the owned event expression. It
    /// remains an expression node so array selects and hierarchical paths keep
    /// their declaration identity and indices until simulator lowering.
    Named(NodeId),
}

/// Edge selector retained for clocking block input/output skews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockingEdge {
    None,
    Posedge,
    Negedge,
    BothEdges,
}

/// Owned timing metadata for one clocking block skew. `delay` identifies the
/// captured timing control while `delay_expression` identifies its scalar
/// delay expression when the control is a regular `#` delay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockingSkew {
    pub edge: ClockingEdge,
    pub delay: Option<NodeId>,
    pub delay_expression: Option<NodeId>,
}

/// Owned declaration and event metadata for one clocking block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockingBlockInfo {
    pub event: NodeId,
    pub event_specs: Vec<EventSpec>,
    pub event_implicit: bool,
    pub is_default: bool,
    pub is_global: bool,
    pub default_input: ClockingSkew,
    pub default_output: ClockingSkew,
}

/// Owned source and skew metadata for one clocking block variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockingVarInfo {
    pub block: NodeId,
    pub source: NodeId,
    pub direction: Direction,
    pub input: ClockingSkew,
    pub output: ClockingSkew,
}

impl EventSpec {
    pub(crate) fn referenced_nodes(&self, nodes: &mut Vec<NodeId>) {
        match self {
            Self::Qualified { event, condition } => {
                event.referenced_nodes(nodes);
                nodes.push(*condition);
            }
            Self::Edge { sig, .. } | Self::AnyChange { sig } | Self::Named(sig) => nodes.push(*sig),
        }
    }
}
