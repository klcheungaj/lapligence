//! Array methods lowered as one loop in the calling frame (SIM-019).
//!
//! The C callback runtime evaluates a `with` expression over packed or real
//! items in a helper without access to the caller's frame. Methods it cannot
//! express (string, handle, record or nested-container elements, real or
//! string keys, expressions that read automatic state or call subroutines,
//! and string-keyed index results) instead run a generated loop in the
//! caller. The loop evaluates the `with` expression exactly once per element,
//! in index or key order (SV 7.12), with `item` naming the receiver element at
//! the loop's current key, so every element type that a select can read is
//! available. It records ordinal positions or one key per element in
//! temporary queues; runtime calls turn those into the result without calling
//! back into the model ([`IrContainerStmt::Gather`],
//! [`IrContainerStmt::UniquePositions`], [`IrContainerStmt::SortByKeys`]).

use super::*;
use crate::sim::ir::{IrObjectQuery, IrObjectStmt};

/// How a generated method loop names the current receiver element.
#[derive(Clone, Debug)]
pub(in crate::sim::codegen) enum InlineKey {
    /// Dynamic array or queue: the element index is the loop position.
    Position,
    /// Integral associative key local.
    Integral {
        name: String,
        width: u32,
        signed: bool,
    },
    /// String associative key local.
    String(String),
}

/// The iterator binding of one generated method loop.
#[derive(Clone, Debug)]
pub(in crate::sim::codegen) struct InlineIterator {
    /// Frontend iterator declaration (`item` or the named iterator).
    node: NodeId,
    container: usize,
    /// 32-bit signed ordinal of the current element in traversal order.
    position: String,
    key: InlineKey,
    /// Declared range of a fixed-array receiver copied into `container`.
    range: Option<(i32, i32)>,
}

/// One per-element value of a `with` expression or of the element itself.
enum InlineValue {
    Packed(IrExpr),
    Real(IrExpr),
    String(IrStringExpr),
}

/// Which family of method a generated loop serves, for the routing decision.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum InlineUse {
    /// `sum`/`product`/`and`/`or`/`xor` with a `with` clause.
    Reduction,
    /// A queue-valued locator, `min`/`max` or `unique` method.
    Result(IrContainerMethod),
    /// `sort`/`rsort`.
    Order,
}

pub(super) fn index_result(method: IrContainerMethod) -> bool {
    matches!(
        method,
        IrContainerMethod::FindIndex
            | IrContainerMethod::FindFirstIndex
            | IrContainerMethod::FindLastIndex
            | IrContainerMethod::UniqueIndex
    )
}

fn local_read(name: &str, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(IrExprKind::LocalRead(name.to_owned()), width, signed, None)
}

fn local_lhs(name: &str, width: u32, signed: bool, two_state: bool) -> IrLhs {
    IrLhs::WholeRef {
        addr: format!("&{name}"),
        width,
        signed,
        two_state,
        shortreal: false,
    }
}

fn int_const(value: i128, width: u32, signed: bool) -> IrExpr {
    pattern_key_expr(value, width, signed, true)
}

fn assign(lhs: IrLhs, rhs: IrExpr) -> IrStmt {
    IrStmt::Assign {
        lhs,
        rhs,
        nba: false,
    }
}

fn if_then(cond: IrExpr, then_: Vec<IrStmt>) -> IrStmt {
    IrStmt::If {
        cond,
        then_,
        els: None,
        check: IrUniquePriorityCheck::None,
    }
}

fn logic(op: IrBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::Bin {
            op,
            a: Box::new(a),
            b: Box::new(b),
        },
        1,
        false,
        None,
    )
}

fn not(value: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::LogNot,
            a: Box::new(value),
        },
        1,
        false,
        None,
    )
}

/// The declared index of the element at `position` of a fixed array copied
/// in left-to-right order.
fn fixed_index_of(position: IrExpr, left: i32, right: i32) -> IrExpr {
    IrExpr::resize_to(
        common_bin_expr(
            if left <= right {
                IrBinOp::Add
            } else {
                IrBinOp::Sub
            },
            int_const(i128::from(left), 32, true),
            position,
        ),
        32,
        true,
    )
}

fn real_zero() -> IrExpr {
    IrExpr::new(IrExprKind::Const(IrConst::real(0.0)), 0, false, None)
}

/// A typed local holding one key, and how to read and compare it.
struct KeyLocal {
    name: String,
    kind: KeyKind,
}

#[derive(Clone, Copy)]
enum KeyKind {
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Real,
    String,
}

impl KeyLocal {
    fn declare(&self) -> IrStmt {
        match self.kind {
            KeyKind::Packed {
                width,
                signed,
                two_state,
            } => IrStmt::DeclLocal {
                name: self.name.clone(),
                width,
                signed,
                init: None,
                two_state,
            },
            KeyKind::Real => IrStmt::DeclLocal {
                name: self.name.clone(),
                width: 0,
                signed: false,
                init: Some(Box::new(real_zero())),
                two_state: false,
            },
            KeyKind::String => IrStmt::DeclString {
                name: self.name.clone(),
                init: None,
            },
        }
    }

    fn read(&self) -> InlineValue {
        match self.kind {
            KeyKind::Packed { width, signed, .. } => {
                InlineValue::Packed(local_read(&self.name, width, signed))
            }
            KeyKind::Real => InlineValue::Real(local_read(&self.name, 0, false)),
            KeyKind::String => InlineValue::String(IrStringExpr::LocalRead(self.name.clone())),
        }
    }

    fn store(&self, value: InlineValue) -> IrStmt {
        match (self.kind, value) {
            (
                KeyKind::Packed {
                    width,
                    signed,
                    two_state,
                },
                InlineValue::Packed(value),
            ) => assign(
                local_lhs(&self.name, width, signed, two_state),
                IrExpr::resize_to(value, width, signed),
            ),
            (KeyKind::Real, InlineValue::Real(value)) => {
                assign(local_lhs(&self.name, 0, false, false), value)
            }
            (KeyKind::String, InlineValue::String(value)) => IrStmt::Object(Box::new(
                IrObjectStmt::StringAssignLocal(self.name.clone(), value),
            )),
            _ => unreachable!("key locals store values of their own kind"),
        }
    }

    fn element(&self) -> IrContainerElement {
        match self.kind {
            KeyKind::Packed {
                width,
                signed,
                two_state,
            } => IrContainerElement::Packed {
                width,
                signed,
                two_state,
            },
            KeyKind::Real => IrContainerElement::Real { shortreal: false },
            KeyKind::String => IrContainerElement::String,
        }
    }

    /// `self` strictly orders before `other` (`<`), or after it (`>`) with
    /// `greater`. Unknown packed comparisons are false, so an X key never
    /// replaces the current extreme.
    fn before(&self, other: &Self, greater: bool) -> IrExpr {
        let op = if greater { IrBinOp::Gt } else { IrBinOp::Lt };
        match (self.read(), other.read()) {
            (InlineValue::String(a), InlineValue::String(b)) => cmp_expr_ir(
                op,
                IrExpr::new(
                    IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringCompare(a, b, false))),
                    32,
                    true,
                    None,
                ),
                int_const(0, 32, true),
            ),
            (InlineValue::Packed(a), InlineValue::Packed(b))
            | (InlineValue::Real(a), InlineValue::Real(b)) => cmp_expr_ir(op, a, b),
            _ => unreachable!("compared key locals share one kind"),
        }
    }
}

impl InlineValue {
    fn kind(&self, two_state: bool) -> KeyKind {
        match self {
            InlineValue::Packed(value) => KeyKind::Packed {
                width: value.width,
                signed: value.signed,
                two_state,
            },
            InlineValue::Real(_) => KeyKind::Real,
            InlineValue::String(_) => KeyKind::String,
        }
    }

    fn push(self, queue: usize) -> IrStmt {
        IrStmt::Container(Box::new(match self {
            InlineValue::Packed(value) | InlineValue::Real(value) => {
                IrContainerStmt::QueuePushBack {
                    container: queue,
                    value,
                }
            }
            InlineValue::String(value) => IrContainerStmt::QueuePushBackString {
                container: queue,
                value,
            },
        }))
    }
}

impl Codegen<'_> {
    /// The generated-loop iterator that `node` names as an element: its
    /// declaration or a reference to it.
    pub(in crate::sim::codegen) fn inline_item(&self, node: NodeId) -> Option<&InlineIterator> {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            _ => node,
        };
        self.inline_iterators
            .iter()
            .rev()
            .find(|iterator| iterator.node == target)
    }

    /// The receiver element selected by a generated-loop iterator, in the
    /// shape of an element select: `(container, [iterator declaration])`.
    /// Index lowering maps the declaration to the loop key.
    pub(in crate::sim::codegen) fn inline_element_path(
        &self,
        node: NodeId,
    ) -> Option<(usize, NodeId, bool)> {
        self.inline_item(node).map(|iterator| {
            (
                iterator.container,
                iterator.node,
                matches!(iterator.key, InlineKey::String(_)),
            )
        })
    }

    fn inline_declaration(&self, node: NodeId) -> Option<&InlineIterator> {
        self.inline_iterators
            .iter()
            .rev()
            .find(|iterator| iterator.node == node)
    }

    fn inline_key_expr(iterator: &InlineIterator) -> Option<IrExpr> {
        match &iterator.key {
            InlineKey::Position => Some(local_read(&iterator.position, 32, true)),
            InlineKey::Integral {
                name,
                width,
                signed,
            } => Some(local_read(name, *width, *signed)),
            InlineKey::String(_) => None,
        }
    }

    /// The current integral key when `node` is a generated-loop iterator
    /// declaration standing for an element index.
    pub(in crate::sim::codegen) fn inline_key_read(&self, node: NodeId) -> Option<IrExpr> {
        self.inline_declaration(node)
            .and_then(Self::inline_key_expr)
    }

    /// The current string key when `node` is a generated-loop iterator
    /// declaration over a string-keyed array.
    pub(in crate::sim::codegen) fn inline_string_key_read(
        &self,
        node: NodeId,
    ) -> Option<IrStringExpr> {
        match &self.inline_declaration(node)?.key {
            InlineKey::String(name) => Some(IrStringExpr::LocalRead(name.clone())),
            _ => None,
        }
    }

    /// `item.index` of a generated-loop iterator (SV 7.12.4): the element
    /// index of a dynamic array or queue, or the associative key.
    pub(in crate::sim::codegen) fn inline_index_read(
        &self,
        path: &str,
        receiver: NodeId,
    ) -> Option<Result<IrExpr, String>> {
        let iterator = self.inline_item(receiver)?;
        if let Some((left, right)) = iterator.range {
            return Some(Ok(fixed_index_of(
                local_read(&iterator.position, 32, true),
                left,
                right,
            )));
        }
        Some(Self::inline_key_expr(iterator).ok_or_else(|| {
            format!("string array-method iterator index in `{path}` is not an integral value")
        }))
    }

    /// `item.index` of a generated-loop iterator over a string-keyed array.
    pub(in crate::sim::codegen) fn inline_string_index_read(
        &self,
        node: NodeId,
    ) -> Option<IrStringExpr> {
        let receiver = match self.kind(node) {
            NodeKind::SysCall { name } if name == "index" => *self.node(node).children.first()?,
            _ => return None,
        };
        match &self.inline_item(receiver)?.key {
            InlineKey::String(name) => Some(IrStringExpr::LocalRead(name.clone())),
            _ => None,
        }
    }

    /// Whether `node` reads automatic state of the enclosing activation,
    /// which a context-free callback helper cannot see.
    fn reads_activation_state(&self, node: NodeId) -> bool {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => Some(*target),
            _ => self.lexical_proc_local(node).map(|(target, _)| target),
        };
        if let Some(target) = target {
            if self
                .capture_source(target)
                .is_some_and(|source| source.info.static_signal.is_none())
                || self.event_string_local(target).is_some()
                || self.event_handle_local(target).is_some()
                || self.inline_declaration(target).is_some()
            {
                return true;
            }
        }
        self.node(node)
            .children
            .iter()
            .any(|child| self.reads_activation_state(*child))
    }

    /// Whether `node` calls a user subroutine, whose effects a read-only
    /// callback helper cannot perform.
    fn calls_subroutine(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::FuncCall { .. })
            || self
                .node(node)
                .children
                .iter()
                .any(|child| self.calls_subroutine(*child))
    }

    /// Whether the `with` expression yields a real or string value.
    fn non_integral_with(&self, node: NodeId) -> bool {
        self.db.type_descriptor(node).is_some_and(|descriptor| {
            matches!(descriptor.shape, TypeShape::Real { .. } | TypeShape::String)
        })
    }

    /// Whether a method on `container` must be lowered as a generated loop
    /// rather than through the packed/real callback runtime.
    pub(super) fn inline_method_needed(
        &self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
        container: usize,
        usage: InlineUse,
    ) -> Result<bool, String> {
        let target = &self.model.containers[container];
        let with_node = self.container_method_with_node(path, call, receiver)?;
        if !target.element.is_packed() && !target.element.is_real() {
            return Ok(true);
        }
        if let InlineUse::Result(method) = usage {
            if index_result(method)
                && matches!(
                    target.kind,
                    IrContainerKind::Associative {
                        key: IrAssocKey::String
                    }
                )
            {
                return Ok(true);
            }
        }
        let Some(with_node) = with_node else {
            return Ok(false);
        };
        if self.reads_activation_state(with_node)
            || self.calls_subroutine(with_node)
            || self.non_integral_with(with_node)
        {
            return Ok(true);
        }
        // Real items reach the callback runtime only for locators.
        Ok(target.element.is_real()
            && !matches!(
                usage,
                InlineUse::Result(
                    IrContainerMethod::Find
                        | IrContainerMethod::FindIndex
                        | IrContainerMethod::FindFirst
                        | IrContainerMethod::FindFirstIndex
                        | IrContainerMethod::FindLast
                        | IrContainerMethod::FindLastIndex
                )
            ))
    }

    fn inline_name(&mut self, tag: &str) -> String {
        let sequence = self.proc_seq;
        self.proc_seq += 1;
        format!("_llg_m{tag}{sequence}")
    }

    /// An empty activation queue declared at the start of the method.
    fn inline_queue(&mut self, element: IrContainerElement) -> (usize, IrStmt) {
        let index = self.model.containers.len();
        self.model.containers.push(IrContainer {
            c_name: format!("S_llg_container_{index}"),
            element,
            kind: IrContainerKind::Queue {
                maximum_elements: None,
            },
            initial_size: None,
            activation: true,
            class_field: None,
            receiver: None,
        });
        (
            index,
            IrStmt::Container(Box::new(IrContainerStmt::Declare(index))),
        )
    }

    /// The receiver element at the current key, for methods without a
    /// `with` clause.
    fn inline_element_value(
        &self,
        path: &str,
        name: &str,
        iterator: &InlineIterator,
    ) -> Result<InlineValue, String> {
        let container = iterator.container;
        let element = &self.model.containers[container].element;
        let string_key = match &iterator.key {
            InlineKey::String(key) => Some(IrStringExpr::LocalRead(key.clone())),
            _ => None,
        };
        let index = Self::inline_key_expr(iterator);
        let read = |operation: IrContainerExpr, width: u32, signed: bool| {
            IrExpr::new(
                IrExprKind::Container(Box::new(operation)),
                width,
                signed,
                None,
            )
        };
        Ok(match (element, string_key, index) {
            (IrContainerElement::Packed { width, signed, .. }, Some(key), _) => {
                InlineValue::Packed(read(IrContainerExpr::GetString { container, key }, *width, *signed))
            }
            (IrContainerElement::Packed { width, signed, .. }, None, Some(index)) => {
                InlineValue::Packed(read(
                    IrContainerExpr::Get {
                        container,
                        index: Box::new(index),
                    },
                    *width,
                    *signed,
                ))
            }
            (IrContainerElement::Real { .. }, Some(key), _) => InlineValue::Real(read(
                IrContainerExpr::GetStringReal { container, key },
                0,
                false,
            )),
            (IrContainerElement::Real { .. }, None, Some(index)) => InlineValue::Real(read(
                IrContainerExpr::GetReal {
                    container,
                    index: Box::new(index),
                },
                0,
                false,
            )),
            (IrContainerElement::String, Some(key), _) => {
                InlineValue::String(IrStringExpr::AssociativeGet {
                    container,
                    key: Box::new(key),
                })
            }
            (IrContainerElement::String, None, Some(index)) => {
                InlineValue::String(IrStringExpr::ContainerGet {
                    container,
                    index: Box::new(index),
                })
            }
            _ => {
                return Err(format!(
                    "array method `{name}` in `{path}` needs a `with` clause: its elements have no relational order"
                ))
            }
        })
    }

    /// The per-element value of `with_node` (or of the element itself).
    fn inline_value(
        &mut self,
        path: &str,
        name: &str,
        with_node: Option<NodeId>,
        iterator: &InlineIterator,
    ) -> Result<InlineValue, String> {
        let Some(node) = with_node else {
            return self.inline_element_value(path, name, iterator);
        };
        if self.is_string_expr(path, node) {
            return Ok(InlineValue::String(self.lower_string(path, node)?));
        }
        let value = self.lower_expr(path, node)?;
        Ok(if value.is_real() {
            InlineValue::Real(value)
        } else {
            InlineValue::Packed(value)
        })
    }

    /// One loop over `container` in index/key order (or reverse) running
    /// `body` per element with `iterator` bound. `stop` names a one-bit local
    /// that ends the loop once set.
    fn inline_loop(
        &mut self,
        path: &str,
        container: usize,
        iterator: NodeId,
        reverse: bool,
        stop: Option<&str>,
        body: impl FnOnce(&mut Self, &InlineIterator) -> Result<Vec<IrStmt>, String>,
    ) -> Result<Vec<IrStmt>, String> {
        let position = self.inline_name("pos");
        let mut statements = vec![IrStmt::DeclLocal {
            name: position.clone(),
            width: 32,
            signed: true,
            init: None,
            two_state: true,
        }];
        let key = match self.model.containers[container].kind.clone() {
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => InlineKey::Position,
            IrContainerKind::Associative {
                key:
                    IrAssocKey::Integral {
                        width,
                        signed,
                        two_state,
                    },
            } => {
                let name = self.inline_name("key");
                statements.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    init: None,
                    two_state,
                });
                InlineKey::Integral {
                    name,
                    width,
                    signed,
                }
            }
            IrContainerKind::Associative {
                key: IrAssocKey::String,
            } => {
                let name = self.inline_name("key");
                statements.push(IrStmt::DeclString {
                    name: name.clone(),
                    init: None,
                });
                InlineKey::String(name)
            }
            IrContainerKind::Associative {
                key: IrAssocKey::Wildcard,
            } => {
                return Err(format!(
                    "array method over a wildcard-index associative array in `{path}` has no key traversal (SV 7.8.1)"
                ))
            }
        };
        let binding = InlineIterator {
            node: iterator,
            container,
            position: position.clone(),
            key: key.clone(),
            range: self.inline_fixed_ranges.get(&container).copied(),
        };
        self.inline_iterators.push(binding.clone());
        let lowered = body(self, &binding);
        self.inline_iterators.pop();
        let body = lowered?;

        let read = || local_read(&position, 32, true);
        let lhs = || local_lhs(&position, 32, true, true);
        let size = IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::Size(container))),
            32,
            true,
            None,
        );
        let last = common_bin_expr(IrBinOp::Sub, size.clone(), int_const(1, 32, true));
        let step = assign(
            lhs(),
            IrExpr::resize_to(
                common_bin_expr(
                    if reverse { IrBinOp::Sub } else { IrBinOp::Add },
                    read(),
                    int_const(1, 32, true),
                ),
                32,
                true,
            ),
        );
        let start = assign(
            lhs(),
            if reverse {
                IrExpr::resize_to(last, 32, true)
            } else {
                int_const(0, 32, true)
            },
        );
        let (init, mut cond, incr) = match &key {
            InlineKey::Position => (
                vec![start],
                if reverse {
                    cmp_expr_ir(IrBinOp::Ge, read(), int_const(0, 32, true))
                } else {
                    cmp_expr_ir(IrBinOp::Lt, read(), size)
                },
                vec![step],
            ),
            InlineKey::Integral { .. } | InlineKey::String(_) => {
                // Associative arrays are walked in key order with
                // first/next (or last/prev); the position counts entries.
                let found = self.inline_name("found");
                statements.push(IrStmt::DeclLocal {
                    name: found.clone(),
                    width: 32,
                    signed: true,
                    init: None,
                    two_state: true,
                });
                let traverse = |direction: IrAssocTraversal| {
                    let operation = match &key {
                        InlineKey::Integral {
                            name,
                            width,
                            signed,
                        } => IrContainerExpr::AssocTraverse {
                            container,
                            direction,
                            key_address: format!("&{name}"),
                            key_signal: None,
                            key_width: *width,
                            key_signed: *signed,
                            key_two_state: matches!(
                                self.model.containers[container].kind,
                                IrContainerKind::Associative {
                                    key: IrAssocKey::Integral {
                                        two_state: true,
                                        ..
                                    }
                                }
                            ),
                        },
                        InlineKey::String(name) => IrContainerExpr::AssocTraverseStringLocal {
                            container,
                            direction,
                            key_name: name.clone(),
                        },
                        InlineKey::Position => unreachable!("associative keys only"),
                    };
                    IrExpr::new(IrExprKind::Container(Box::new(operation)), 32, true, None)
                };
                let (first, next) = if reverse {
                    (IrAssocTraversal::Last, IrAssocTraversal::Prev)
                } else {
                    (IrAssocTraversal::First, IrAssocTraversal::Next)
                };
                let found_lhs = || local_lhs(&found, 32, true, true);
                (
                    vec![start, assign(found_lhs(), traverse(first))],
                    cmp_expr_ir(
                        IrBinOp::Neq,
                        local_read(&found, 32, true),
                        int_const(0, 32, true),
                    ),
                    vec![assign(found_lhs(), traverse(next)), step],
                )
            }
        };
        if let Some(stop) = stop {
            cond = logic(IrBinOp::LogAnd, not(local_read(stop, 1, false)), cond);
        }
        statements.push(IrStmt::For {
            init,
            cond,
            incr,
            body,
        });
        Ok(statements)
    }

    fn method_iterator(&self, call: NodeId) -> NodeId {
        // Without a `with` clause no iterator is declared; the loop then only
        // reads the element through its key, so the call node stands in.
        self.db.method_call_iterator(call).unwrap_or(call)
    }

    /// `dst = receiver.method() [with (...)]` for a queue-valued locator,
    /// `min`/`max` or `unique` method (SV 7.12.1, 7.12.4).
    pub(super) fn lower_inline_method_result(
        &mut self,
        path: &str,
        dst: usize,
        call: NodeId,
        receiver: NodeId,
        container: usize,
        method: IrContainerMethod,
    ) -> Result<IrStmt, String> {
        let name = match self.kind(call) {
            NodeKind::MethodCall { name, .. } => name.clone(),
            _ => return Err(format!("array method in `{path}` is not a method call")),
        };
        let name = name.as_str();
        let with_node = self.container_method_with_node(path, call, receiver)?;
        let iterator = self.method_iterator(call);
        let (positions, declare) = self.inline_queue(IrContainerElement::Packed {
            width: 32,
            signed: true,
            two_state: true,
        });
        let mut statements = vec![declare];
        let keys = index_result(method);
        let push_position = |iterator: &InlineIterator| {
            IrStmt::Container(Box::new(IrContainerStmt::QueuePushBack {
                container: positions,
                value: local_read(&iterator.position, 32, true),
            }))
        };
        let two_state = with_node.is_some_and(|node| self.db.is_two_state_type(node));
        match method {
            IrContainerMethod::Find
            | IrContainerMethod::FindIndex
            | IrContainerMethod::FindFirst
            | IrContainerMethod::FindFirstIndex
            | IrContainerMethod::FindLast
            | IrContainerMethod::FindLastIndex => {
                let Some(with_node) = with_node else {
                    return Err(format!(
                        "array locator method `{name}` in `{path}` requires a with clause"
                    ));
                };
                let single = !matches!(
                    method,
                    IrContainerMethod::Find | IrContainerMethod::FindIndex
                );
                let reverse = matches!(
                    method,
                    IrContainerMethod::FindLast | IrContainerMethod::FindLastIndex
                );
                let stop = single.then(|| self.inline_name("stop"));
                if let Some(stop) = &stop {
                    statements.push(IrStmt::DeclLocal {
                        name: stop.clone(),
                        width: 1,
                        signed: false,
                        init: Some(Box::new(int_const(0, 1, false))),
                        two_state: true,
                    });
                }
                let stop_name = stop.clone();
                statements.extend(self.inline_loop(
                    path,
                    container,
                    iterator,
                    reverse,
                    stop.as_deref(),
                    |this, iterator| {
                        let truth = match this.inline_value(path, name, Some(with_node), iterator)? {
                            InlineValue::Packed(value) => value,
                            InlineValue::Real(value) => {
                                cmp_expr_ir(IrBinOp::Neq, value, real_zero())
                            }
                            InlineValue::String(_) => {
                                return Err(format!(
                                    "array locator `{name}` in `{path}` needs an integral `with` expression"
                                ))
                            }
                        };
                        let mut selected = vec![push_position(iterator)];
                        if let Some(stop) = &stop_name {
                            selected.push(assign(
                                local_lhs(stop, 1, false, true),
                                int_const(1, 1, false),
                            ));
                        }
                        Ok(vec![if_then(truth, selected)])
                    },
                )?);
            }
            IrContainerMethod::Min | IrContainerMethod::Max => {
                let greater = method == IrContainerMethod::Max;
                let have = self.inline_name("have");
                let best_position = self.inline_name("best");
                statements.push(IrStmt::DeclLocal {
                    name: have.clone(),
                    width: 1,
                    signed: false,
                    init: Some(Box::new(int_const(0, 1, false))),
                    two_state: true,
                });
                statements.push(IrStmt::DeclLocal {
                    name: best_position.clone(),
                    width: 32,
                    signed: true,
                    init: Some(Box::new(int_const(0, 32, true))),
                    two_state: true,
                });
                let key_name = self.inline_name("k");
                let best_name = self.inline_name("kbest");
                let mut declarations = Vec::new();
                let loop_statements =
                    self.inline_loop(path, container, iterator, false, None, |this, iterator| {
                        let value = this.inline_value(path, name, with_node, iterator)?;
                        let kind = value.kind(two_state);
                        let key = KeyLocal {
                            name: key_name,
                            kind,
                        };
                        let best = KeyLocal {
                            name: best_name,
                            kind,
                        };
                        declarations.push(key.declare());
                        declarations.push(best.declare());
                        let mut better = logic(
                            IrBinOp::LogOr,
                            not(local_read(&have, 1, false)),
                            key.before(&best, greater),
                        );
                        if let KeyKind::Real = kind {
                            // NaN is unordered: a known key replaces a NaN
                            // extreme, and a NaN key never wins.
                            let (InlineValue::Real(current), InlineValue::Real(extreme)) =
                                (key.read(), best.read())
                            else {
                                unreachable!("real key locals read reals");
                            };
                            better = logic(
                                IrBinOp::LogOr,
                                better,
                                logic(
                                    IrBinOp::LogAnd,
                                    cmp_expr_ir(IrBinOp::Neq, extreme.clone(), extreme),
                                    cmp_expr_ir(IrBinOp::Eq, current.clone(), current),
                                ),
                            );
                        }
                        let update = vec![
                            best.store(key.read()),
                            assign(
                                local_lhs(&best_position, 32, true, true),
                                local_read(&iterator.position, 32, true),
                            ),
                            assign(local_lhs(&have, 1, false, true), int_const(1, 1, false)),
                        ];
                        Ok(vec![key.store(value), if_then(better, update)])
                    })?;
                // Key locals are declared before the loop that assigns them.
                statements.extend(declarations);
                statements.extend(loop_statements);
                statements.push(if_then(
                    local_read(&have, 1, false),
                    vec![IrStmt::Container(Box::new(
                        IrContainerStmt::QueuePushBack {
                            container: positions,
                            value: local_read(&best_position, 32, true),
                        },
                    ))],
                ));
            }
            IrContainerMethod::Unique | IrContainerMethod::UniqueIndex => {
                let mut keys_queue = None;
                let mut declarations = Vec::new();
                let loop_statements =
                    self.inline_loop(path, container, iterator, false, None, |this, iterator| {
                        let value = this.inline_value(path, name, with_node, iterator)?;
                        let element = KeyLocal {
                            name: String::new(),
                            kind: value.kind(two_state),
                        }
                        .element();
                        let (queue, declare) = this.inline_queue(element);
                        declarations.push(declare);
                        keys_queue = Some(queue);
                        Ok(vec![value.push(queue)])
                    })?;
                statements.extend(declarations);
                statements.extend(loop_statements);
                statements.push(IrStmt::Container(Box::new(
                    IrContainerStmt::UniquePositions {
                        positions,
                        keys: keys_queue.ok_or("unique keys were not lowered")?,
                    },
                )));
            }
            IrContainerMethod::Sort
            | IrContainerMethod::RSort
            | IrContainerMethod::Reverse
            | IrContainerMethod::Shuffle => {
                return Err(format!(
                    "array method `{name}` in `{path}` reorders its receiver and returns no queue"
                ))
            }
        }
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Gather {
            dst,
            src: container,
            positions,
            keys,
        })));
        Ok(IrStmt::Block(statements))
    }

    /// `receiver.sort()`/`rsort()` [with (...)] by keys computed once per
    /// element in index order (SV 7.12.2).
    pub(super) fn lower_inline_order(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
        container: usize,
        descending: bool,
        name: &str,
    ) -> Result<IrStmt, String> {
        if !matches!(
            self.model.containers[container].kind,
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "array method `{name}` in `{path}` requires a dynamic array or queue"
            ));
        }
        let with_node = self.container_method_with_node(path, call, receiver)?;
        let iterator = self.method_iterator(call);
        let two_state = with_node.is_some_and(|node| self.db.is_two_state_type(node));
        let mut keys_queue = None;
        let mut declarations = Vec::new();
        let loop_statements =
            self.inline_loop(path, container, iterator, false, None, |this, iterator| {
                let value = this.inline_value(path, name, with_node, iterator)?;
                let element = KeyLocal {
                    name: String::new(),
                    kind: value.kind(two_state),
                }
                .element();
                let (queue, declare) = this.inline_queue(element);
                declarations.push(declare);
                keys_queue = Some(queue);
                Ok(vec![value.push(queue)])
            })?;
        let mut statements = declarations;
        statements.extend(loop_statements);
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::SortByKeys {
            container,
            keys: keys_queue.ok_or("ordering keys were not lowered")?,
            descending,
        })));
        Ok(IrStmt::Block(statements))
    }

    /// `receiver.sum()` (and product/and/or/xor) `with (...)` as an
    /// expression: the accumulator has the `with` expression's
    /// self-determined type and starts at the operation's identity, like the
    /// packed callback path (SV 7.12.3).
    pub(super) fn lower_inline_reduction(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
        container: usize,
        operation: IrContainerReduction,
        name: &str,
    ) -> Result<IrExpr, String> {
        let Some(with_node) = self.container_method_with_node(path, call, receiver)? else {
            return Err(format!(
                "container reduction `{name}` in `{path}` has no with clause"
            ));
        };
        let iterator = self.method_iterator(call);
        let two_state = self.db.is_two_state_type(with_node);
        let accumulator = self.inline_name("acc");
        let mut shape = None;
        let loop_statements =
            self.inline_loop(path, container, iterator, false, None, |this, iterator| {
                let InlineValue::Packed(value) =
                    this.inline_value(path, name, Some(with_node), iterator)?
                else {
                    return Err(format!(
                        "array reduction `{name}` in `{path}` needs an integral `with` expression"
                    ));
                };
                let (width, signed) = (value.width, value.signed);
                shape = Some((width, signed));
                let op = match operation {
                    IrContainerReduction::Sum => IrBinOp::Add,
                    IrContainerReduction::Product => IrBinOp::Mul,
                    IrContainerReduction::BitAnd => IrBinOp::BitAnd,
                    IrContainerReduction::BitOr => IrBinOp::BitOr,
                    IrContainerReduction::BitXor => IrBinOp::BitXor,
                };
                let combined = IrExpr::new(
                    IrExprKind::Bin {
                        op,
                        a: Box::new(local_read(&accumulator, width, signed)),
                        b: Box::new(value),
                    },
                    width,
                    signed,
                    None,
                );
                Ok(vec![assign(
                    local_lhs(&accumulator, width, signed, two_state),
                    combined,
                )])
            })?;
        let (width, signed) = shape.ok_or("reduction value was not lowered")?;
        let identity = match operation {
            IrContainerReduction::Product => int_const(1, width, signed),
            IrContainerReduction::BitAnd => IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::BitNeg,
                    a: Box::new(int_const(0, width, signed)),
                },
                width,
                signed,
                None,
            ),
            _ => int_const(0, width, signed),
        };
        let mut statements = vec![IrStmt::DeclLocal {
            name: accumulator.clone(),
            width,
            signed,
            init: Some(Box::new(identity)),
            two_state,
        }];
        statements.extend(loop_statements);
        Ok(IrExpr::new(
            IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                statements,
                value: local_read(&accumulator, width, signed),
            })),
            width,
            signed,
            None,
        ))
    }
}

/// A packed or real one-dimensional fixed array copied into an activation
/// queue so the dynamic-receiver methods apply to it (SIM-019).
pub(super) struct FixedMethodCopy {
    pub(super) queue: usize,
    pub(super) statements: Vec<IrStmt>,
    array: usize,
    range: (i32, i32),
    real: bool,
    width: u32,
    signed: bool,
}

impl Codegen<'_> {
    /// Copy a stored packed or real fixed array receiver, element by element
    /// in left-to-right order, into a fresh activation queue. Native fixed
    /// arrays (string, handle, record elements) are already containers and
    /// return `None`, as do receivers that are not a whole stored array.
    pub(super) fn fixed_method_copy(
        &mut self,
        receiver: NodeId,
    ) -> Result<Option<FixedMethodCopy>, String> {
        if !matches!(
            self.kind(receiver),
            NodeKind::Array { .. }
                | NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
        ) || self.container_of(receiver).is_some()
        {
            return Ok(None);
        }
        let Some(array) = self
            .array_of(receiver)
            .filter(|array| array.dims.len() == 1)
        else {
            return Ok(None);
        };
        let (array_ir, range, real, shortreal, width, signed) = (
            array.ir,
            array.dims[0],
            array.real,
            array.shortreal,
            if array.real { 0 } else { array.elem_width },
            array.signed,
        );
        let array = self.reference_array(array_ir);
        let element = if real {
            IrContainerElement::Real { shortreal }
        } else {
            IrContainerElement::Packed {
                width,
                signed,
                two_state: self.model.arrays[array].two_state,
            }
        };
        let (queue, declare) = self.inline_queue(element);
        self.inline_fixed_ranges.insert(queue, range);
        let count = i128::from(range.0.abs_diff(range.1)) + 1;
        let position = self.inline_name("fpos");
        let read = || local_read(&position, 32, true);
        let value = IrExpr::new(
            IrExprKind::ArrayRead {
                arr: array,
                indices: vec![fixed_index_of(read(), range.0, range.1)],
                elem_sel: IrElemSel::Whole,
            },
            width,
            signed,
            None,
        );
        let statements = vec![
            declare,
            IrStmt::DeclLocal {
                name: position.clone(),
                width: 32,
                signed: true,
                init: None,
                two_state: true,
            },
            IrStmt::For {
                init: vec![assign(
                    local_lhs(&position, 32, true, true),
                    int_const(0, 32, true),
                )],
                cond: cmp_expr_ir(IrBinOp::Lt, read(), int_const(count, 32, true)),
                incr: vec![assign(
                    local_lhs(&position, 32, true, true),
                    IrExpr::resize_to(
                        common_bin_expr(IrBinOp::Add, read(), int_const(1, 32, true)),
                        32,
                        true,
                    ),
                )],
                body: vec![IrStmt::Container(Box::new(
                    IrContainerStmt::QueuePushBack {
                        container: queue,
                        value,
                    },
                ))],
            },
        ];
        Ok(Some(FixedMethodCopy {
            queue,
            statements,
            array,
            range,
            real,
            width,
            signed,
        }))
    }

    /// Statements that store the copied queue back into the fixed array, in
    /// the same order it was read (after an in-place reordering method).
    pub(super) fn fixed_method_store(&mut self, copy: &FixedMethodCopy) -> Vec<IrStmt> {
        let position = self.inline_name("fpos");
        let read = || local_read(&position, 32, true);
        let index = Box::new(read());
        let value = IrExpr::new(
            IrExprKind::Container(Box::new(if copy.real {
                IrContainerExpr::GetReal {
                    container: copy.queue,
                    index,
                }
            } else {
                IrContainerExpr::Get {
                    container: copy.queue,
                    index,
                }
            })),
            copy.width,
            copy.signed,
            None,
        );
        let count = i128::from(copy.range.0.abs_diff(copy.range.1)) + 1;
        vec![
            IrStmt::DeclLocal {
                name: position.clone(),
                width: 32,
                signed: true,
                init: None,
                two_state: true,
            },
            IrStmt::For {
                init: vec![assign(
                    local_lhs(&position, 32, true, true),
                    int_const(0, 32, true),
                )],
                cond: cmp_expr_ir(IrBinOp::Lt, read(), int_const(count, 32, true)),
                incr: vec![assign(
                    local_lhs(&position, 32, true, true),
                    IrExpr::resize_to(
                        common_bin_expr(IrBinOp::Add, read(), int_const(1, 32, true)),
                        32,
                        true,
                    ),
                )],
                body: vec![assign(
                    IrLhs::ArrayElem {
                        arr: copy.array,
                        indices: vec![fixed_index_of(read(), copy.range.0, copy.range.1)],
                        elem_sel: IrElemSel::Whole,
                    },
                    value,
                )],
            },
        ]
    }

    /// Rewrite each copied-queue position in an index-result queue as the
    /// declared fixed-array index.
    pub(super) fn fixed_method_indices(
        &mut self,
        copy: &FixedMethodCopy,
        dst: usize,
    ) -> Result<Vec<IrStmt>, String> {
        let IrContainerElement::Packed {
            width,
            signed,
            two_state,
        } = self.model.containers[dst].element
        else {
            return Err("fixed-array index method requires an integral result queue".to_owned());
        };
        let position = self.inline_name("fidx");
        let read = || local_read(&position, 32, true);
        let size = IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::Size(dst))),
            32,
            true,
            None,
        );
        let current = IrExpr::convert_to(
            IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Get {
                    container: dst,
                    index: Box::new(read()),
                })),
                width,
                signed,
                None,
            ),
            32,
            true,
        );
        let index = fixed_index_of(current, copy.range.0, copy.range.1);
        let value = ir_to_storage(
            IrExpr::convert_to(index, width, signed),
            width,
            signed,
            two_state,
        )?;
        Ok(vec![
            IrStmt::DeclLocal {
                name: position.clone(),
                width: 32,
                signed: true,
                init: None,
                two_state: true,
            },
            IrStmt::For {
                init: vec![assign(
                    local_lhs(&position, 32, true, true),
                    int_const(0, 32, true),
                )],
                cond: cmp_expr_ir(IrBinOp::Lt, read(), size),
                incr: vec![assign(
                    local_lhs(&position, 32, true, true),
                    IrExpr::resize_to(
                        common_bin_expr(IrBinOp::Add, read(), int_const(1, 32, true)),
                        32,
                        true,
                    ),
                )],
                body: vec![IrStmt::Container(Box::new(IrContainerStmt::Set {
                    container: dst,
                    index: read(),
                    value,
                }))],
            },
        ])
    }
}
