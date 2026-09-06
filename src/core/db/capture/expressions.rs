use crate::core::elab;
use crate::ffi::vpi::{self, VpiHandle};
use std::io::Read;

use super::super::capture;
use super::super::database::*;
use super::super::*;

pub(in crate::core::db) fn operation(handle: VpiHandle) -> Operation {
    Operation::from_raw(vpi::get(vpi::vpiOpType, handle))
}

pub(in crate::core::db) fn constant_type(handle: VpiHandle) -> ConstantType {
    ConstantType::from_raw(vpi::get(vpi::vpiConstType, handle))
}

impl CachedConstantSourceFile {
    fn span(&self, line: usize, column: usize, len: usize) -> Option<&str> {
        let Self::Available { text, line_starts } = self else {
            return None;
        };
        let line_start = *line_starts.get(line)? as usize;
        let mut line_end = line_starts
            .get(line + 1)
            .map_or(text.len(), |start| *start as usize);
        while line_end > line_start && matches!(text.as_bytes()[line_end - 1], b'\n' | b'\r') {
            line_end -= 1;
        }
        let start = line_start.checked_add(column)?;
        let end = start.checked_add(len)?;
        (start <= line_end && end <= line_end)
            .then(|| text.get(start..end))
            .flatten()
    }
}

impl Builder {
    /// Maximum bytes read from one source file while recovering unsigned
    /// constant spelling. A sentinel byte detects a racing/growing file.
    const MAX_CONSTANT_SOURCE_FILE_BYTES: usize = 8 * 1024 * 1024;
    /// Maximum combined source-text and line-offset bytes retained during one
    /// owned database build.
    const MAX_CONSTANT_SOURCE_CACHE_BYTES: usize = 64 * 1024 * 1024;
    /// Bounds negative-cache keys as well as open file descriptors/metadata
    /// work triggered by distinct UHDM paths.
    const MAX_CONSTANT_SOURCE_FILES: usize = 1024;
    const MAX_CONSTANT_SOURCE_PATH_BYTES: usize = 4096;
    /// Maximum retained lexical token or folded-expression span.
    const MAX_CONSTANT_SOURCE_SPAN_BYTES: usize = 4096;

    pub(in crate::core::db) fn admit_constant_source_files(&mut self, source_files: &[String]) {
        for path in source_files {
            if path.is_empty()
                || path.len() > Self::MAX_CONSTANT_SOURCE_PATH_BYTES
                || self.constant_source_allowed.contains(path)
            {
                continue;
            }
            if self.constant_source_allowed.len() == Self::MAX_CONSTANT_SOURCE_FILES {
                break;
            }
            let Some(total) = self.constant_source_bytes.checked_add(path.len()) else {
                break;
            };
            if total > Self::MAX_CONSTANT_SOURCE_CACHE_BYTES {
                break;
            }
            self.constant_source_allowed.insert(path.clone());
            self.constant_source_bytes = total;
        }
    }

    pub(in crate::core::db) fn constant_source(&mut self, handle: VpiHandle) -> ConstantSource {
        let file = vpi::obj_file(handle);
        let line = vpi::get(vpi::vpiLineNo, handle);
        let end_line = vpi::get(vpi::vpiEndLineNo, handle);
        let column = vpi::get(vpi::vpiColumnNo, handle);
        let end_column = vpi::get(vpi::vpiEndColumnNo, handle);
        if file.is_empty() || line <= 0 || column <= 0 {
            return ConstantSource::NotCaptured;
        }
        if !self.constant_source_allowed.contains(&file) {
            return ConstantSource::Unavailable;
        }
        if line != end_line || end_column <= column {
            return ConstantSource::Unavailable;
        }
        let span = match usize::try_from(end_column - column) {
            Ok(span) if span <= Self::MAX_CONSTANT_SOURCE_SPAN_BYTES => span,
            _ => return ConstantSource::Unavailable,
        };
        if !self.ensure_constant_source_file(&file) {
            return ConstantSource::Unavailable;
        }
        let Some(cached) = self.constant_source_files.get(&file) else {
            return ConstantSource::Unavailable;
        };
        match cached.span(line as usize - 1, column as usize - 1, span) {
            Some(source) => {
                let Some(total) = self.constant_source_bytes.checked_add(source.len()) else {
                    self.constant_source_cache_exhausted = true;
                    return ConstantSource::Unavailable;
                };
                if total > Self::MAX_CONSTANT_SOURCE_CACHE_BYTES {
                    self.constant_source_cache_exhausted = true;
                    return ConstantSource::Unavailable;
                }
                self.constant_source_bytes = total;
                ConstantSource::Exact(source.to_owned())
            }
            _ => ConstantSource::Unavailable,
        }
    }

    /// Return the bounded declaration prefix beginning at a UHDM object's
    /// source location. This is used only when UHDM omits declaration
    /// qualifiers such as `signed`; callers must treat absence as unknown.
    pub(in crate::core::db) fn declaration_source_prefix(
        &mut self,
        handle: VpiHandle,
    ) -> Option<String> {
        let file = vpi::obj_file(handle);
        let line = usize::try_from(vpi::get(vpi::vpiLineNo, handle)).ok()?;
        let column = usize::try_from(vpi::get(vpi::vpiColumnNo, handle)).ok()?;
        if line == 0
            || column == 0
            || !self.constant_source_allowed.contains(&file)
            || !self.ensure_constant_source_file(&file)
        {
            return None;
        }
        let CachedConstantSourceFile::Available { text, line_starts } =
            self.constant_source_files.get(&file)?
        else {
            return None;
        };
        let start = usize::try_from(*line_starts.get(line - 1)?).ok()? + column - 1;
        let end = text
            .len()
            .min(start.checked_add(Self::MAX_CONSTANT_SOURCE_SPAN_BYTES)?);
        let source = text.get(start..end)?;
        let prefix_end = source.find('{').map_or(source.len(), |offset| offset + 1);
        Some(source[..prefix_end].to_owned())
    }

    fn simple_size_cast_width(&mut self, handle: VpiHandle) -> Option<u32> {
        let ConstantSource::Exact(source) = self.constant_source(handle) else {
            return None;
        };
        let (size, expression) = source.split_once('\'')?;
        let size = size.trim().replace('_', "");
        if size.is_empty() || !size.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let expression = expression.trim();
        if !expression.starts_with('(') || !expression.ends_with(')') {
            return None;
        }
        size.parse::<u32>().ok().filter(|width| *width != 0)
    }

    fn size_cast_spelling(&mut self, handle: VpiHandle) -> (Option<String>, bool) {
        let source = match self.constant_source(handle) {
            ConstantSource::Exact(source) => Some(source),
            _ => {
                let decompiled = vpi::get_str(vpi::vpiDecompile, handle);
                if decompiled.is_empty() {
                    self.declaration_source_prefix(handle)
                } else {
                    Some(decompiled)
                }
            }
        };
        let Some(source) = source else {
            return (None, false);
        };
        let Some((prefix, _)) = source.trim_start().split_once('\'') else {
            return (None, true);
        };
        let token = prefix.trim().to_owned();
        let valid = !token.is_empty()
            && token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        (valid.then_some(token), true)
    }

    fn ensure_constant_source_file(&mut self, file: &str) -> bool {
        if let Some(cached) = self.constant_source_files.get(file) {
            return matches!(cached, CachedConstantSourceFile::Available { .. });
        }
        if self.constant_source_cache_exhausted
            || self.constant_source_files.len() >= Self::MAX_CONSTANT_SOURCE_FILES
            || file.len() > Self::MAX_CONSTANT_SOURCE_PATH_BYTES
            || self
                .constant_source_bytes
                .checked_add(file.len())
                .is_none_or(|total| total > Self::MAX_CONSTANT_SOURCE_CACHE_BYTES)
        {
            self.constant_source_cache_exhausted = true;
            return false;
        }
        self.constant_source_bytes += file.len();
        let loaded = self.load_constant_source_file(file);
        let available = matches!(loaded, CachedConstantSourceFile::Available { .. });
        self.constant_source_files.insert(file.to_owned(), loaded);
        available
    }

    fn load_constant_source_file(&mut self, file: &str) -> CachedConstantSourceFile {
        let metadata = match std::fs::symlink_metadata(file) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            _ => return CachedConstantSourceFile::Unavailable,
        };
        let remaining = Self::MAX_CONSTANT_SOURCE_CACHE_BYTES - self.constant_source_bytes;
        if metadata.len() > Self::MAX_CONSTANT_SOURCE_FILE_BYTES as u64
            || metadata.len() > remaining as u64
        {
            return CachedConstantSourceFile::Unavailable;
        }
        let mut source = match std::fs::File::open(file) {
            Ok(source) => source,
            Err(_) => return CachedConstantSourceFile::Unavailable,
        };
        let opened_metadata = match source.metadata() {
            Ok(metadata) if metadata.is_file() => metadata,
            _ => return CachedConstantSourceFile::Unavailable,
        };
        if opened_metadata.len() > Self::MAX_CONSTANT_SOURCE_FILE_BYTES as u64
            || opened_metadata.len() > remaining as u64
        {
            return CachedConstantSourceFile::Unavailable;
        }
        let mut text = String::new();
        if source
            .by_ref()
            .take((Self::MAX_CONSTANT_SOURCE_FILE_BYTES.min(remaining)) as u64 + 1)
            .read_to_string(&mut text)
            .is_err()
            || text.len() > Self::MAX_CONSTANT_SOURCE_FILE_BYTES
        {
            return CachedConstantSourceFile::Unavailable;
        }
        let line_count = 1usize.saturating_add(text.bytes().filter(|byte| *byte == b'\n').count());
        let cost = match line_count
            .checked_mul(std::mem::size_of::<u32>())
            .and_then(|offsets| offsets.checked_add(text.len()))
        {
            Some(cost) => cost,
            None => return CachedConstantSourceFile::Unavailable,
        };
        let Some(total) = self.constant_source_bytes.checked_add(cost) else {
            return CachedConstantSourceFile::Unavailable;
        };
        if total > Self::MAX_CONSTANT_SOURCE_CACHE_BYTES {
            return CachedConstantSourceFile::Unavailable;
        }
        let mut line_starts = Vec::with_capacity(line_count);
        line_starts.push(0u32);
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .filter_map(|(index, _)| u32::try_from(index + 1).ok()),
        );
        self.constant_source_bytes = total;
        CachedConstantSourceFile::Available { text, line_starts }
    }

    pub(in crate::core::db) fn walk_node(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let t = vpi::obj_type(h);
        let id = self.register(parent, &props, NodeKind::Other);
        match t {
            // ── Statements ─────────────────────────────────────────────────
            vpi::vpiBegin | vpi::vpiNamedBegin => {
                // Classify BEFORE descending: a `disable <label>;` nested in
                // the block resolves its target against the enclosing scope
                // chain DURING the walk, so ancestors must already carry
                // their final kind.
                self.set_stmt(id, StmtKind::Begin);
                let mut kids: Vec<NodeId> = Vec::new();
                // Named events declared inside the block are captured first
                // so trigger statements below would resolve against them.
                // Harmless future-proofing only: Surelog v1.86 never emits
                // block-local event declarations as `vpiNamedEvent` children
                // here — they arrive as ordinary 1-bit `logic_var`s under the
                // block's `vpiVariables` (see [`NodeKind::NamedEvent`]).
                for ne in iter(vpi::vpiNamedEvent, h) {
                    kids.push(self.walk_named_event(ne.raw(), Some(id))?);
                }
                // Locals declared inside the block (function/task bodies,
                // named blocks) are captured as Var children so refs resolve
                // and `sim::codegen` can hoist their declarations.
                for v in iter(vpi::vpiVariables, h) {
                    kids.push(self.walk_var(v.raw(), Some(id))?);
                }
                for s in iter(vpi::vpiStmt, h) {
                    kids.push(self.walk_node(s.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                // Index named begin blocks so `disable <label>` targets
                // resolve through the (vpiType, vpiFullName) index (unnamed
                // blocks have no full name and are skipped by `index_node`).
                self.index_node(h, &props, id);
            }
            vpi::vpiIf | vpi::vpiIfElse => {
                let cond = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "if statement without condition".to_string())?;
                let cond_id = self.walk_node(cond.raw(), Some(id))?;
                let then = child(vpi::vpiStmt, h)
                    .ok_or_else(|| "if statement without then branch".to_string())?;
                let then_id = self.walk_node(then.raw(), Some(id))?;
                let mut kids = vec![cond_id, then_id];
                if let Some(els) = child(vpi::vpiElseStmt, h) {
                    kids.push(self.walk_node(els.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::IfElse { cond: cond_id });
            }
            vpi::vpiAssignment => {
                let blocking = vpi::get(vpi::vpiBlocking, h) != 0;
                let op = capture::statements::assignment_operation(h);
                let delay = self.assign_control(h);
                let lhs =
                    child(vpi::vpiLhs, h).ok_or_else(|| "assignment without LHS".to_string())?;
                let lhs_id = self.walk_node(lhs.raw(), Some(id))?;
                let mut kids = vec![lhs_id];
                if let Some(rhs) = child(vpi::vpiRhs, h) {
                    kids.push(self.walk_node(rhs.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                self.set_stmt(
                    id,
                    StmtKind::Assign {
                        blocking,
                        op,
                        delay,
                    },
                );
            }
            vpi::vpiCase => {
                let case_type = capture::statements::case_kind(h);
                let sel = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "case without selector".to_string())?;
                let sel_id = self.walk_node(sel.raw(), Some(id))?;
                let mut kids = vec![sel_id];
                let mut items = Vec::new();
                for item in iter(vpi::vpiCaseItem, h) {
                    let item = item.raw();
                    let mut exprs = Vec::new();
                    for e in iter(vpi::vpiExpr, item) {
                        let eid = self.walk_node(e.raw(), Some(id))?;
                        kids.push(eid);
                        exprs.push(eid);
                    }
                    let body = match child(vpi::vpiStmt, item) {
                        Some(s) => {
                            let bid = self.walk_node(s.raw(), Some(id))?;
                            kids.push(bid);
                            Some(bid)
                        }
                        None => None,
                    };
                    items.push(CaseItem { exprs, body });
                }
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::Case { case_type, items });
            }
            vpi::vpiFor => {
                let mut kids: Vec<NodeId> = Vec::new();
                // A for-loop with an inline declaration is a scope. Capture
                // any explicitly exposed locals first; v1.87 more commonly
                // exposes the declaration as the initializer's direct Var
                // LHS, which is recognized below before condition/body refs
                // are walked.
                let mut vars = Vec::new();
                for v in iter(vpi::vpiVariables, h) {
                    let vid = self.walk_var(v.raw(), Some(id))?;
                    kids.push(vid);
                    vars.push(vid);
                }
                let mut init = Vec::new();
                for s in iter(vpi::vpiForInitStmt, h) {
                    let sid = self.walk_node(s.raw(), Some(id))?;
                    kids.push(sid);
                    init.push(sid);
                }
                for statement in &init {
                    if let Some(lhs) = self.nodes[statement.index()].children.first().copied() {
                        if matches!(self.nodes[lhs.index()].kind, NodeKind::Var { .. }) {
                            vars.push(lhs);
                        }
                    }
                }
                let cond = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "for without condition".to_string())?;
                let cond_id = self.walk_node(cond.raw(), Some(id))?;
                kids.push(cond_id);
                let mut incr = Vec::new();
                for s in iter(vpi::vpiForIncStmt, h) {
                    let sid = self.walk_node(s.raw(), Some(id))?;
                    kids.push(sid);
                    incr.push(sid);
                }
                let body = self.walk_opt_stmt(h, Some(id))?;
                kids.push(body);
                self.set_children(id, kids);
                self.set_stmt(
                    id,
                    StmtKind::For {
                        vars,
                        init,
                        cond: cond_id,
                        incr,
                        body,
                    },
                );
            }
            vpi::vpiForeachStmt => {
                // UHDM exposes the iterated array as a single vpiVariables
                // relation and the lexical index declarations through the
                // vpiLoopVars group. The array already belongs to its module
                // scope, so retain its existing arena identity rather than
                // walking a duplicate declaration subtree.
                let array = child(vpi::vpiVariables, h).and_then(|variable| {
                    self.resolve_direct(variable.raw())
                        .or_else(|| self.resolve_ref(variable.raw()))
                });
                let mut kids = Vec::new();
                let mut vars = Vec::new();
                for variable in iter(vpi::vpiLoopVars, h) {
                    let variable = self.walk_var(variable.raw(), Some(id))?;
                    kids.push(variable);
                    vars.push(variable);
                }
                let body = self.walk_opt_stmt(h, Some(id))?;
                kids.push(body);
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::Foreach { array, vars, body });
            }
            vpi::vpiWhile | vpi::vpiDoWhile | vpi::vpiRepeat => {
                let cond = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "loop without condition".to_string())?;
                let cond_id = self.walk_node(cond.raw(), Some(id))?;
                let body = self.walk_opt_stmt(h, Some(id))?;
                self.set_children(id, vec![cond_id, body]);
                match t {
                    vpi::vpiWhile => self.set_stmt(
                        id,
                        StmtKind::While {
                            cond: cond_id,
                            body,
                        },
                    ),
                    vpi::vpiDoWhile => self.set_stmt(
                        id,
                        StmtKind::DoWhile {
                            cond: cond_id,
                            body,
                        },
                    ),
                    vpi::vpiRepeat => self.set_stmt(
                        id,
                        StmtKind::Repeat {
                            cond: cond_id,
                            body,
                        },
                    ),
                    _ => unreachable!(),
                }
            }
            vpi::vpiForever => {
                let body = self.walk_opt_stmt(h, Some(id))?;
                self.set_children(id, vec![body]);
                self.set_stmt(id, StmtKind::Forever { body });
            }
            vpi::vpiEventControl => self.walk_event_control(h, id)?,
            vpi::vpiDelayControl => {
                let source = self.recover_delay_source(h);
                let ticks = source.as_deref().and_then(|source| {
                    source
                        .bytes()
                        .all(|byte| byte.is_ascii_digit())
                        .then(|| source.parse().ok())
                        .flatten()
                });
                let expression = ticks.is_none().then_some(source).flatten();
                let body = self.walk_opt_stmt(h, Some(id))?;
                self.set_children(id, vec![body]);
                self.set_stmt(id, StmtKind::DelayControl { ticks, expression });
            }
            vpi::vpiEventStmt => {
                // `-> ev;` / `->> ev;` — the target named_event is resolved
                // by name against the enclosing scopes (see
                // `resolve_named_event`; Surelog exposes no relationship for
                // it).  The resolved event node is captured as the stmt's
                // child.
                let blocking = vpi::get(vpi::vpiBlocking, h) != 0;
                let name = props.name.clone();
                let target = self.resolve_named_event(&name, parent);
                let kids = match target {
                    Some(t) => vec![t],
                    None => Vec::new(),
                };
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::EventTrigger { blocking, target });
            }
            vpi::vpiWait => {
                let cond = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "wait without condition".to_string())?;
                let cond_id = self.walk_node(cond.raw(), Some(id))?;
                let mut kids = vec![cond_id];
                // `wait (cond) stmt`: the body is optional (Surelog emits no
                // vpiStmt for a bare `wait (cond);`).
                if let Some(body) = child(vpi::vpiStmt, h) {
                    kids.push(self.walk_node(body.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::Wait { cond: cond_id });
            }
            vpi::vpiForce => {
                let lhs = child(vpi::vpiLhs, h).ok_or_else(|| "force without LHS".to_string())?;
                let lhs_id = self.walk_node(lhs.raw(), Some(id))?;
                let rhs = child(vpi::vpiRhs, h).ok_or_else(|| "force without RHS".to_string())?;
                let rhs_id = self.walk_node(rhs.raw(), Some(id))?;
                self.set_children(id, vec![lhs_id, rhs_id]);
                self.set_stmt(
                    id,
                    StmtKind::Force {
                        lhs: lhs_id,
                        rhs: rhs_id,
                    },
                );
            }
            vpi::vpiRelease | vpi::vpiDeassign => {
                let lhs = child(vpi::vpiLhs, h)
                    .ok_or_else(|| "release/deassign without LHS".to_string())?;
                let lhs_id = self.walk_node(lhs.raw(), Some(id))?;
                self.set_children(id, vec![lhs_id]);
                if t == vpi::vpiRelease {
                    self.set_stmt(id, StmtKind::Release { lhs: lhs_id });
                } else {
                    self.set_stmt(id, StmtKind::Deassign { lhs: lhs_id });
                }
            }
            vpi::vpiAssignStmt => {
                // Procedural continuous assignment (`assign x = e;` inside
                // procedural code, UHDM `paASSIGN`; NOT a blocking
                // assignment — that is `vpiAssignment` above).  The UHDM
                // generated VPI layer exposes the operands as 1-to-1
                // `vpiLhs`/`vpiRhs` relations (verified against
                // assign_stmt.cpp's GetByVpiType).
                let lhs = child(vpi::vpiLhs, h)
                    .ok_or_else(|| "procedural continuous assignment without LHS".to_string())?;
                let lhs_id = self.walk_node(lhs.raw(), Some(id))?;
                let rhs = child(vpi::vpiRhs, h)
                    .ok_or_else(|| "procedural continuous assignment without RHS".to_string())?;
                let rhs_id = self.walk_node(rhs.raw(), Some(id))?;
                self.set_children(id, vec![lhs_id, rhs_id]);
                self.set_stmt(
                    id,
                    StmtKind::ProcContAssign {
                        lhs: lhs_id,
                        rhs: rhs_id,
                    },
                );
            }
            vpi::vpiNullStmt => {
                self.set_stmt(id, StmtKind::Empty);
            }
            vpi::vpiReturnStmt => {
                // `return [expr];` — the value lives under `vpiCondition`;
                // a bare `return;` has no children.
                let mut kids: Vec<NodeId> = Vec::new();
                let value = match child(vpi::vpiCondition, h) {
                    Some(v) => {
                        let vid = self.walk_node(v.raw(), Some(id))?;
                        kids.push(vid);
                        Some(vid)
                    }
                    None => None,
                };
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::Return { value });
            }
            vpi::vpiFork | vpi::vpiNamedFork => {
                // `fork … join` — the branches are the `vpiStmt` CHILDREN (a
                // list), each a `begin`/`named_begin` or a bare statement;
                // the join kind comes from `vpiJoinType` (vpiJoin=0,
                // vpiJoinNone=1, vpiJoinAny=2).  Named forks carry their
                // block name in the node's `vpiName` (captured by `common`).
                // Like the begin arms, classification happens BEFORE
                // descending so nested disables see the final kind.
                let join_kind = capture::statements::join_kind(h);
                self.set_stmt(
                    id,
                    StmtKind::Fork {
                        join_kind,
                        branches: Vec::new(),
                    },
                );
                let mut branches = Vec::new();
                for b in iter(vpi::vpiStmt, h) {
                    branches.push(self.walk_node(b.raw(), Some(id))?);
                }
                self.set_children(id, branches.clone());
                self.set_stmt(
                    id,
                    StmtKind::Fork {
                        join_kind,
                        branches,
                    },
                );
                // Index named forks (`fork : name … join`) like named begins
                // so a `disable` targeting one resolves to the fork node.
                self.index_node(h, &props, id);
            }
            vpi::vpiWaitFork => {
                // `wait fork;` — atomic statement, no children.
                self.set_stmt(id, StmtKind::WaitFork);
            }
            vpi::vpiDisableFork => {
                // `disable fork;` — atomic statement, no children.
                self.set_stmt(id, StmtKind::DisableFork);
            }
            vpi::vpiDisable => {
                // `disable <label>;` — the target object normally arrives
                // RESOLVED under `vpiExpr` (Surelog resolves tasks/functions
                // first, then the directly enclosing scope children by name),
                // so it is looked up in the same (vpiType, vpiFullName) index
                // used everywhere else; a ref wrapper falls back through
                // `resolve_ref`.  VERIFIED Surelog v1.86 quirk: when the
                // disable sits inside nested constructs (e.g. an `if` inside
                // a loop body inside the named block), the compile-time name
                // search does not climb out of the intermediate statements
                // and the UHDM keeps neither a `vpiExpr` nor a name — the
                // identifier is then recovered from the source line (same
                // strategy as `recover_delay_source`) and matched against the
                // enclosing scope chain.  The resolved node is kept in the
                // variant only — see the `Disable` docs for why it is not a
                // child.
                let mut target = child(vpi::vpiExpr, h).and_then(|t| {
                    self.resolve_direct(t.raw())
                        .or_else(|| self.resolve_ref(t.raw()))
                });
                if target.is_none() {
                    if let Some(name) = self.recover_disable_target_name(h) {
                        target = self.resolve_disable_target(&name, parent);
                    }
                }
                self.set_children(id, Vec::new());
                self.set_stmt(id, StmtKind::Disable { target });
            }
            vpi::vpiBreak | vpi::vpiContinue => {
                // `break;` / `continue;` — atomic statements, no children.
                if t == vpi::vpiBreak {
                    self.set_stmt(id, StmtKind::Break);
                } else {
                    self.set_stmt(id, StmtKind::Continue);
                }
            }

            // ── Expressions ────────────────────────────────────────────────
            vpi::vpiConstant => {
                let const_type = capture::expressions::constant_type(h);
                let unsigned = const_type == ConstantType::UnsignedInteger;
                let kind = ExprKind::Constant {
                    value: vpi::read_value(h),
                    size: vpi::get(vpi::vpiSize, h),
                    const_type,
                    source: if unsigned {
                        self.constant_source(h)
                    } else {
                        ConstantSource::NotCaptured
                    },
                    // Surelog v1.87's time-literal compilation path uniquely
                    // omits vpiDecompile while ordinary UInt constants set it.
                    time_literal_candidate: unsigned
                        && vpi::get_str(vpi::vpiDecompile, h).is_empty(),
                };
                self.set_expr(id, kind);
            }
            vpi::vpiEnumConst => {
                let value = elab::read_value(h).ok();
                self.set_kind(id, NodeKind::EnumConst { value });
            }
            vpi::vpiOperation => {
                let op = capture::expressions::operation(h);
                let reordered = vpi::get(vpi::vpiReordered, h) != 0;
                let mut operands = Vec::new();
                let mut kids: Vec<NodeId> = Vec::new();
                for o in iter(vpi::vpiOperand, h) {
                    let oid = self.walk_node(o.raw(), Some(id))?;
                    kids.push(oid);
                    operands.push(oid);
                }
                self.set_children(id, kids);
                if op.is(vpi::vpiCastOp) {
                    let typespec = child(vpi::vpiTypespec, h);
                    let mut ty = typespec
                        .as_ref()
                        .map(|typespec| self.typespec_info(typespec.raw()))
                        .unwrap_or_default();
                    // Surelog omits the range on size-cast and parameterized
                    // typedef cast typespecs. Prefer the elaborated operation
                    // size, then evaluate retained typedef ranges in the
                    // operation's enclosing parameter scope.
                    let (mut size_cast_expr, spelling_available) = self.size_cast_spelling(h);
                    if size_cast_expr.as_deref().is_some_and(|token| {
                        token.eq_ignore_ascii_case(&ty.kind)
                            || ty.type_name.as_deref() == Some(token)
                            || (!token
                                .replace('_', "")
                                .bytes()
                                .all(|byte| byte.is_ascii_digit())
                                && !self.nodes.iter().any(|node| {
                                    node.name == token
                                        && matches!(&node.kind, NodeKind::Param { .. })
                                }))
                    }) {
                        size_cast_expr = None;
                    }
                    let size_cast_width = size_cast_expr
                        .as_deref()
                        .and_then(|token| token.replace('_', "").parse::<u32>().ok())
                        .filter(|width| *width != 0)
                        .or_else(|| self.simple_size_cast_width(h));
                    if let Some(width) = size_cast_width {
                        ty.width = Some(width);
                    } else if ty.width.is_none() {
                        let width = vpi::get(vpi::vpiSize, h);
                        if width > 0 {
                            ty.width = u32::try_from(width).ok();
                        } else if let Some(typespec) = &typespec {
                            ty.width = self.contextual_typespec_width(typespec.raw(), h);
                        }
                    }
                    let operand = operands
                        .first()
                        .copied()
                        .ok_or_else(|| "cast without operand".to_string())?;
                    let cast_kind_known =
                        spelling_available || !matches!(ty.kind.as_str(), "int" | "integer");
                    let two_state = typespec
                        .as_ref()
                        .is_some_and(|typespec| self.typespec_two_state(typespec.raw(), 0));
                    self.set_expr(
                        id,
                        ExprKind::Cast {
                            operand,
                            ty,
                            size_cast: size_cast_expr.is_some(),
                            size_cast_expr,
                            cast_kind_known,
                            two_state,
                        },
                    );
                } else {
                    self.set_expr(
                        id,
                        ExprKind::Operation {
                            op,
                            reordered,
                            operands,
                        },
                    );
                }
            }
            vpi::vpiRefObj | vpi::vpiRefVar => {
                let target = self.resolve_ref(h);
                self.set_expr(id, ExprKind::Ref { target });
            }
            other if is_scalar_var_type(other) => {
                // Inline for-loop declarations reach the database through
                // the initializer assignment's direct variable LHS rather
                // than a vpiVariables relation on the for_stmt. Capture that
                // object in place so later refs can resolve to its indexed
                // arena identity.
                let ty = self.type_info_of(h);
                self.set_kind(id, NodeKind::Var { ty });
                if self.object_two_state(h) {
                    self.two_state_types.insert(id);
                }
                self.index_node(h, &props, id);
                if let Some(expression) = child(vpi::vpiExpr, h) {
                    let expression = self.walk_node(expression.raw(), Some(id))?;
                    self.set_children(id, vec![expression]);
                    self.vars_init.insert(id, expression);
                }
            }
            vpi::vpiNamedEvent => {
                // A named_event object used directly as an expression-shaped
                // node (e.g. an event-control condition or a posedge/negedge
                // operand without a ref wrapper): normalize to the same
                // `Ref` shape so consumers resolve it uniformly.
                let target = self.resolve_direct(h);
                self.set_expr(id, ExprKind::Ref { target });
            }
            vpi::vpiVarSelect => {
                // `var_select` doubles as the multi-level array select shape:
                // `a[i][j]` / `mem[addr][3:0]` carry one `vpiIndex` child per
                // level.  A bare `var_select` (no indices) is a plain ref.
                let idxs = iter(vpi::vpiIndex, h);
                if idxs.is_empty() {
                    let target = self.resolve_ref(h);
                    self.set_expr(id, ExprKind::Ref { target });
                } else {
                    let base = self.select_base(h, Some(id));
                    let mut kids = vec![base];
                    let mut indices = Vec::new();
                    for i in idxs {
                        let iid = self.walk_node(i.raw(), Some(id))?;
                        kids.push(iid);
                        indices.push(iid);
                    }
                    self.set_children(id, kids);
                    self.set_expr(id, ExprKind::ArraySelect { base, indices });
                }
            }
            vpi::vpiBitSelect => {
                let base = self.select_base(h, Some(id));
                let idx = child(vpi::vpiIndex, h)
                    .ok_or_else(|| "bit_select without index".to_string())?;
                let idx_id = self.walk_node(idx.raw(), Some(id))?;
                self.set_children(id, vec![base, idx_id]);
                self.set_expr(
                    id,
                    ExprKind::BitSelect {
                        base,
                        index: idx_id,
                    },
                );
            }
            vpi::vpiPartSelect => {
                let base = self.select_base(h, Some(id));
                let left = child(vpi::vpiLeftRange, h)
                    .ok_or_else(|| "part_select without left range".to_string())?;
                let left_id = self.walk_node(left.raw(), Some(id))?;
                let right = child(vpi::vpiRightRange, h)
                    .ok_or_else(|| "part_select without right range".to_string())?;
                let right_id = self.walk_node(right.raw(), Some(id))?;
                self.set_children(id, vec![base, left_id, right_id]);
                self.set_expr(
                    id,
                    ExprKind::PartSelect {
                        base,
                        left: left_id,
                        right: right_id,
                    },
                );
            }
            vpi::vpiIndexedPartSelect => {
                let base = self.select_base(h, Some(id));
                let base_expr = child(vpi::vpiBaseExpr, h)
                    .ok_or_else(|| "indexed_part_select without base".to_string())?;
                let base_expr_id = self.walk_node(base_expr.raw(), Some(id))?;
                let width_expr = child(vpi::vpiWidthExpr, h)
                    .ok_or_else(|| "indexed_part_select without width".to_string())?;
                let width_expr_id = self.walk_node(width_expr.raw(), Some(id))?;
                let neg = vpi::get(vpi::vpiIndexedPartSelectType, h) == vpi::vpiNegIndexed;
                self.set_children(id, vec![base, base_expr_id, width_expr_id]);
                self.set_expr(
                    id,
                    ExprKind::IndexedPartSelect {
                        base,
                        base_expr: base_expr_id,
                        width_expr: width_expr_id,
                        neg,
                    },
                );
            }
            vpi::vpiHierPath => {
                let mut parts = Vec::new();
                let mut refs = Vec::new();
                // A hier_path's `vpiActual` is 1-to-many: one ref_obj per
                // path element; each ref_obj's own `vpiActual` is the
                // concrete target.
                for a in iter(vpi::vpiActual, h) {
                    let a = a.raw();
                    let n = vpi::obj_name(a);
                    if !n.is_empty() {
                        parts.push(n);
                    }
                    refs.push(self.resolve_ref(a));
                }
                self.set_expr(id, ExprKind::HierPath { parts, refs });
            }

            // ── Calls ──────────────────────────────────────────────────────
            vpi::vpiSysFuncCall | vpi::vpiSysTaskCall => {
                let name = vpi::obj_name(h);
                let mut kids: Vec<NodeId> = Vec::new();
                for a in iter(vpi::vpiArgument, h) {
                    kids.push(self.walk_node(a.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                self.set_kind(id, NodeKind::SysCall { name });
            }
            vpi::vpiFuncCall | vpi::vpiTaskCall => {
                let name = vpi::obj_name(h);
                let is_task = t == vpi::vpiTaskCall;
                // The callee relationship is 1-to-1: `vpiFunction` for
                // `func_call`, `vpiTask` for `task_call`.  Best effort — the
                // def may not have been captured yet (call to a function
                // declared later in the same scope); codegen falls back to a
                // name lookup among the owning instance's functions.
                let callee = child(
                    if is_task {
                        vpi::vpiTask
                    } else {
                        vpi::vpiFunction
                    },
                    h,
                )
                .and_then(|c| self.resolve_direct(c.raw()));
                let mut kids: Vec<NodeId> = Vec::new();
                for a in iter(vpi::vpiArgument, h) {
                    kids.push(self.walk_node(a.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                self.set_kind(
                    id,
                    NodeKind::FuncCall {
                        name,
                        is_task,
                        callee,
                    },
                );
            }

            // ── Unknown constructs: capture with children, never fail ──────
            other => {
                let mut kids: Vec<NodeId> = Vec::new();
                for rel in OTHER_CHILD_RELS {
                    for c in iter(rel, h) {
                        kids.push(self.walk_node(c.raw(), Some(id))?);
                    }
                    if let Some(c) = child(rel, h) {
                        kids.push(self.walk_node(c.raw(), Some(id))?);
                    }
                }
                self.set_children(id, kids);
                if is_stmt_type(other) {
                    self.set_stmt(
                        id,
                        StmtKind::Unsupported {
                            vpi_type: capture::statements::unsupported_type(other),
                        },
                    );
                } else if is_expr_type(other) {
                    self.set_expr(id, ExprKind::Other);
                }
            }
        }
        Ok(id)
    }

    pub(in crate::core::db) fn select_base(
        &mut self,
        sel: VpiHandle,
        parent: Option<NodeId>,
    ) -> NodeId {
        if let Some(b) = self.resolve_ref(sel) {
            return b;
        }
        let full = vpi::obj_full_name(sel);
        if !full.is_empty() {
            if let Some(id) = self.array_by_fullname(&full) {
                return id;
            }
        }
        self.register(
            parent,
            &CommonProps::default(),
            NodeKind::Expr(ExprKind::Ref { target: None }),
        )
    }

    pub(in crate::core::db) fn array_by_fullname(&self, full: &str) -> Option<NodeId> {
        for t in [vpi::vpiArrayVar, vpi::vpiRegArray, vpi::vpiArrayNet] {
            if let Some(id) = self.index.get(&(t, full.to_string())) {
                return Some(*id);
            }
        }
        None
    }
}

#[cfg(test)]
mod source_cache_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

    fn test_path(tag: &str) -> std::path::PathBuf {
        let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "llg-constant-source-{tag}-{}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn source_cache_admits_bounded_regular_files() {
        let path = test_path("regular");
        std::fs::write(&path, "first\nsecond\n").expect("write source cache fixture");
        let mut builder = Builder::default();
        assert!(builder.ensure_constant_source_file(&path.to_string_lossy()));
        let CachedConstantSourceFile::Available { text, line_starts } = builder
            .constant_source_files
            .get(path.to_string_lossy().as_ref())
            .expect("cached source")
        else {
            panic!("regular file should be cached");
        };
        assert_eq!(text, "first\nsecond\n");
        assert_eq!(line_starts, &[0, 6, 13]);
        let cached = builder
            .constant_source_files
            .get(path.to_string_lossy().as_ref())
            .expect("cached source");
        assert_eq!(cached.span(1, 0, 6), Some("second"));
        assert_eq!(cached.span(0, 6, 1), None);
        assert_eq!(cached.span(0, 40, 1), None);
        assert_eq!(cached.span(0, 4, 2), None);
        std::fs::remove_file(path).expect("remove source cache fixture");
    }

    #[test]
    fn source_cache_negatively_caches_failures_and_bounds() {
        let missing = test_path("missing");
        let mut builder = Builder::default();
        assert!(!builder.ensure_constant_source_file(&missing.to_string_lossy()));
        std::fs::write(&missing, "now present").expect("create formerly missing fixture");
        assert!(!builder.ensure_constant_source_file(&missing.to_string_lossy()));
        std::fs::remove_file(missing).expect("remove formerly missing fixture");

        let oversized = test_path("oversized");
        let file = std::fs::File::create(&oversized).expect("create oversized fixture");
        file.set_len(Builder::MAX_CONSTANT_SOURCE_FILE_BYTES as u64 + 1)
            .expect("size oversized fixture");
        assert!(!builder.ensure_constant_source_file(&oversized.to_string_lossy()));
        std::fs::remove_file(oversized).expect("remove oversized fixture");

        assert!(
            !builder.ensure_constant_source_file(std::env::temp_dir().to_string_lossy().as_ref())
        );

        let mut exhausted = Builder {
            constant_source_bytes: Builder::MAX_CONSTANT_SOURCE_CACHE_BYTES,
            ..Builder::default()
        };
        assert!(!exhausted.ensure_constant_source_file("another-source.sv"));
        assert!(exhausted.constant_source_cache_exhausted);
    }
}
