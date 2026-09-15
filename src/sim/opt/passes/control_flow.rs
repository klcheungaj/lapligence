//! Control flow.

use super::*;

// ── Pass: prune_branches ──────────────────────────────────────────────────────

/// The runtime truthiness of a constant condition (an ambiguous logical value
/// is false; a known one bit still makes a wider value true). Real constants
/// follow the C `llg_real_to_bool` exactly: `v != 0.0`, so NaN is truthy.
/// Returns `None` when the condition is not a constant.
pub(super) fn truthy_const(e: &IrExpr) -> Option<bool> {
    match &e.kind {
        IrExprKind::Const(c) => match c.real {
            Some(r) => Some(r != 0.0),
            None => as_packed_const(e).map(|v| v.bits.contains(&Bit::One)),
        },
        _ => None,
    }
}

pub(super) fn prune_stmt_list(stmts: &mut Vec<IrStmt>) {
    let old = std::mem::take(stmts);
    let mut out = Vec::with_capacity(old.len());
    for s in old {
        match s {
            IrStmt::If {
                cond,
                then_,
                els,
                check,
            } if check.is_none() => match truthy_const(&cond) {
                Some(true) => {
                    let mut taken = then_;
                    prune_stmt_list(&mut taken);
                    out.extend(taken);
                }
                Some(false) => {
                    if let Some(mut alt) = els {
                        prune_stmt_list(&mut alt);
                        out.extend(alt);
                    }
                }
                None => {
                    let mut then_ = then_;
                    prune_stmt_list(&mut then_);
                    let els = els.map(|mut e| {
                        prune_stmt_list(&mut e);
                        e
                    });
                    out.push(IrStmt::If {
                        cond,
                        then_,
                        els,
                        check,
                    });
                }
            },
            other => {
                let mut other = other;
                prune_nested_in_place(&mut other);
                out.push(other);
            }
        }
    }
    *stmts = out;
}

fn prune_nested_in_place(s: &mut IrStmt) {
    match s {
        IrStmt::Block(b) | IrStmt::ActivationScope { body: b, .. } => prune_stmt_list(b),
        IrStmt::ImmediateAssertion {
            if_true, if_false, ..
        } => {
            if let Some(if_true) = if_true {
                prune_stmt_list(if_true);
            }
            if let Some(if_false) = if_false {
                prune_stmt_list(if_false);
            }
        }
        IrStmt::DeferredImmediateAssertion { .. } => {}
        // A qualified conditional may contain an else-if ladder. Keep its
        // source-level shape intact: pruning a constant nested condition can
        // turn an else-if into an apparent default and suppress a required
        // no-match diagnostic.
        IrStmt::If { check, .. } if !check.is_none() => {}
        IrStmt::If { then_, els, .. } => {
            prune_stmt_list(then_);
            if let Some(els) = els {
                prune_stmt_list(els);
            }
        }
        IrStmt::While { cond, body } => match truthy_const(cond) {
            // A constant-false condition can never enter: an empty block.
            // (A constant-true condition keeps looping — its body carries
            // the waits.)
            Some(false) => {
                *s = IrStmt::Block(Vec::new());
            }
            _ => prune_stmt_list(body),
        },
        IrStmt::WaitCond { cond, body, .. } => match truthy_const(cond) {
            // wait (true) runs its body immediately; wait (false) STAYS —
            // zero-delay guard spin semantics.
            Some(true) => {
                let mut taken = std::mem::take(body);
                prune_stmt_list(&mut taken);
                *s = IrStmt::Block(taken);
            }
            _ => prune_stmt_list(body),
        },
        IrStmt::WaitEventTriggered { body, .. } => prune_stmt_list(body),
        IrStmt::WaitOrder {
            success, failure, ..
        } => {
            prune_stmt_list(success);
            prune_stmt_list(failure);
        }
        IrStmt::Repeat { body, .. } | IrStmt::Forever { body } => prune_stmt_list(body),
        IrStmt::For {
            init, incr, body, ..
        } => {
            prune_stmt_list(init);
            prune_stmt_list(incr);
            prune_stmt_list(body);
        }
        IrStmt::Case {
            sel,
            kind,
            items,
            check,
        } if check.is_none() => {
            if let Some(mut picked) = pick_case_branch(sel, *kind, items) {
                prune_stmt_list(&mut picked);
                *s = IrStmt::Block(picked);
                return;
            }
            for item in items {
                prune_stmt_list(&mut item.body);
            }
        }
        IrStmt::Case { items, .. } => {
            for item in items {
                prune_stmt_list(&mut item.body);
            }
        }
        _ => {}
    }
}

/// Names of every label REFERENCED by a `goto` in the tree (`Label`
/// statements are definitions, not references).
fn collect_goto_names(stmts: &[IrStmt], out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            IrStmt::Goto(l) => {
                out.insert(l.clone());
            }
            IrStmt::Block(b)
            | IrStmt::Forever { body: b }
            | IrStmt::ActivationScope { body: b, .. } => collect_goto_names(b, out),
            IrStmt::If { then_, els, .. } => {
                collect_goto_names(then_, out);
                if let Some(els) = els {
                    collect_goto_names(els, out);
                }
            }
            IrStmt::ImmediateAssertion {
                if_true, if_false, ..
            } => {
                if let Some(if_true) = if_true {
                    collect_goto_names(if_true, out);
                }
                if let Some(if_false) = if_false {
                    collect_goto_names(if_false, out);
                }
            }
            IrStmt::DeferredImmediateAssertion { .. } => {}
            IrStmt::While { body: b, .. } | IrStmt::Repeat { body: b, .. } => {
                collect_goto_names(b, out)
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                collect_goto_names(init, out);
                collect_goto_names(incr, out);
                collect_goto_names(body, out);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    collect_goto_names(&item.body, out);
                }
            }
            IrStmt::WaitCond { body: b, .. } => collect_goto_names(b, out),
            IrStmt::WaitEventTriggered { body: b, .. } => collect_goto_names(b, out),
            IrStmt::WaitOrder {
                success, failure, ..
            } => {
                collect_goto_names(success, out);
                collect_goto_names(failure, out);
            }
            _ => {}
        }
    }
}

/// Drop `Label` statements that no `goto` in the same tree targets anymore:
/// pruning can remove the branch carrying the only jump to a `_bk`/`_ct`/
/// `_xb`/`_id` label, leaving an unreferenced C label behind (`-Wall` warns
/// on those).  Safe because every goto targets a label inside the same
/// emitted function (= the same body tree).
pub(super) fn strip_unreferenced_labels(stmts: &mut Vec<IrStmt>) {
    let mut referenced = HashSet::new();
    collect_goto_names(stmts, &mut referenced);
    strip_labels_in(stmts, &referenced);
}

fn strip_labels_in(stmts: &mut Vec<IrStmt>, referenced: &HashSet<String>) {
    stmts.retain(|s| !matches!(s, IrStmt::Label(l) if !referenced.contains(l)));
    for s in stmts.iter_mut() {
        match s {
            IrStmt::Block(b)
            | IrStmt::Forever { body: b }
            | IrStmt::ActivationScope { body: b, .. } => strip_labels_in(b, referenced),
            IrStmt::If { then_, els, .. } => {
                strip_labels_in(then_, referenced);
                if let Some(els) = els {
                    strip_labels_in(els, referenced);
                }
            }
            IrStmt::ImmediateAssertion {
                if_true, if_false, ..
            } => {
                if let Some(if_true) = if_true {
                    strip_labels_in(if_true, referenced);
                }
                if let Some(if_false) = if_false {
                    strip_labels_in(if_false, referenced);
                }
            }
            IrStmt::DeferredImmediateAssertion { .. } => {}
            IrStmt::While { body: b, .. } | IrStmt::Repeat { body: b, .. } => {
                strip_labels_in(b, referenced)
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                strip_labels_in(init, referenced);
                strip_labels_in(incr, referenced);
                strip_labels_in(body, referenced);
            }
            IrStmt::Case { items, .. } => {
                for item in items.iter_mut() {
                    strip_labels_in(&mut item.body, referenced);
                }
            }
            IrStmt::WaitCond { body: b, .. } => strip_labels_in(b, referenced),
            IrStmt::WaitEventTriggered { body: b, .. } => strip_labels_in(b, referenced),
            IrStmt::WaitOrder {
                success, failure, ..
            } => {
                strip_labels_in(success, referenced);
                strip_labels_in(failure, referenced);
            }
            _ => {}
        }
    }
}

/// Pick the reachable case arm for a constant selector.  Strict
/// first-match-wins over the item order:
///
/// - prune to item k's body iff items[0..k] are all constant and provably
///   unmatched AND item k constant-matches;
/// - prune to a default arm iff EVERY item is constant and provably unmatched
///   (a non-constant item can never be proven unreachable, so it blocks);
/// - otherwise leave the case untouched.
///
/// Wildcard kinds compare with the elab casez/casex helpers: both selector and
/// item are constants here, so their verdict is exact.
fn pick_case_branch(
    sel: &IrExpr,
    kind: IrCaseKind,
    items: &[crate::sim::ir::IrCaseItem],
) -> Option<Vec<IrStmt>> {
    let sel_v = as_packed_const(sel)?;

    /// Provability of one item against the constant selector.
    enum Verdict {
        /// Default arm (no item expressions).
        Default,
        /// Every expression constant and none matches.
        Unmatched,
        /// Some expression constant-matches.
        Matched,
        /// Some expression is not constant: nothing is provable.
        Unknown,
    }

    let verdict = |item: &crate::sim::ir::IrCaseItem| -> Verdict {
        if item.exprs.is_empty() {
            return Verdict::Default;
        }
        for ex in &item.exprs {
            let Some(ev) = as_packed_const(ex) else {
                return Verdict::Unknown;
            };
            let matched = match kind {
                IrCaseKind::Exact => elab::case_eq(&sel_v, &ev),
                IrCaseKind::Casex => elab::casex_eq(&sel_v, &ev),
                IrCaseKind::Casez => elab::casez_eq(&sel_v, &ev),
                IrCaseKind::Real | IrCaseKind::Inside => return Verdict::Unknown,
            };
            if matched.to_u64() == Some(1) {
                return Verdict::Matched;
            }
        }
        Verdict::Unmatched
    };

    // First-match-wins scan.  A default arm or an unknown (non-constant)
    // item cannot be proven unmatched, so everything after it stays
    // unprovable and must keep the case.
    let mut prefix_proven = true;
    for item in items {
        match verdict(item) {
            Verdict::Unmatched => {}
            Verdict::Matched if prefix_proven => return Some(item.body.clone()),
            _ => prefix_proven = false,
        }
    }
    // Default pruning only when every non-default item is const-unmatched
    // (any Matched/Unknown/Default ordering hazard keeps the case intact).
    let mut default_body: Option<Vec<IrStmt>> = None;
    let mut all_provably_dead = true;
    for item in items {
        match verdict(item) {
            Verdict::Unmatched => {}
            Verdict::Default => {
                default_body.get_or_insert_with(|| item.body.clone());
            }
            _ => all_provably_dead = false,
        }
    }
    match default_body {
        _ if !all_provably_dead => None,
        Some(body) => Some(body),
        // Every item was constant-unmatched and there is no default: nothing
        // runs.
        None => Some(Vec::new()),
    }
}
