//! Writes to `const` variables through built-in methods.
//!
//! The frontend rejects assignments, `ref` bindings and output actuals that
//! name a `const` variable, but not a built-in method that modifies the
//! variable it is called on. SV 6.20.6 makes a `const` variable one that
//! "cannot be written", so `q.push_back(x)`, `aa.delete(k)`, `a.sort()` or
//! `s.putc(0, c)` on a `const` declaration is an illegal write as well. A
//! user-defined method of a `const` class handle stays legal: the handle is
//! constant, the object it names is not.

use super::*;

/// Built-in array, queue and associative-array methods that change the
/// receiver's contents (SV 7.5.3, 7.8.7, 7.10.2, 7.12.2).
const MODIFYING_CONTAINER_METHODS: &[&str] = &[
    "delete",
    "insert",
    "push_back",
    "push_front",
    "pop_back",
    "pop_front",
    "sort",
    "rsort",
    "reverse",
    "shuffle",
];

/// Built-in string methods that change the receiver (SV 6.16).
const MODIFYING_STRING_METHODS: &[&str] =
    &["putc", "itoa", "hextoa", "octtoa", "bintoa", "realtoa"];

impl Codegen<'_> {
    /// Reject a modifying built-in method call whose receiver is a `const`
    /// variable or class property, with the call's source location.
    pub(in super::super) fn validate_const_method_writes(&self) -> Result<(), String> {
        for node in self.db.node_ids() {
            let NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } = self.kind(node)
            else {
                continue;
            };
            let Some(declaration) = self.receiver_declaration(*receiver) else {
                continue;
            };
            if !self.db.is_const_variable(declaration) {
                continue;
            }
            let modifies = match self.kind(declaration) {
                NodeKind::Array { .. } => MODIFYING_CONTAINER_METHODS.contains(&name.as_str()),
                NodeKind::Var { ty } if ty.kind == "string" => {
                    MODIFYING_STRING_METHODS.contains(&name.as_str())
                }
                _ => false,
            };
            if modifies {
                return Err(format!(
                    "{}: `{name}` modifies `{}`, which is declared const; a const variable cannot be written after its initialization (SV 6.20.6)",
                    self.source_location(node),
                    self.node(declaration).name
                ));
            }
        }
        Ok(())
    }

    /// The declaration a method receiver names as a whole: a plain or
    /// hierarchical reference (`q`, `obj.q`). A selected element names
    /// different storage and is not followed.
    fn receiver_declaration(&self, receiver: NodeId) -> Option<NodeId> {
        match self.kind(receiver) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
            _ => None,
        }
    }
}
