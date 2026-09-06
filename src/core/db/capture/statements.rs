use crate::ffi::vpi::{self, VpiHandle};

use super::super::capture;
use super::super::database::*;
use super::super::*;

pub(in crate::core::db) fn always_kind(handle: VpiHandle) -> AlwaysKind {
    AlwaysKind::from_raw(vpi::get(vpi::vpiAlwaysType, handle))
}

pub(in crate::core::db) fn assignment_operation(handle: VpiHandle) -> Operation {
    Operation::from_raw(vpi::get(vpi::vpiOpType, handle))
}

pub(in crate::core::db) fn case_kind(handle: VpiHandle) -> CaseKind {
    CaseKind::from_raw(vpi::get(vpi::vpiCaseType, handle))
}

pub(in crate::core::db) fn join_kind(handle: VpiHandle) -> JoinKind {
    JoinKind::from_raw(vpi::get(vpi::vpiJoinType, handle))
}

pub(in crate::core::db) fn unsupported_type(raw: i32) -> ObjectType {
    ObjectType::from_statement_raw(raw)
}

impl Builder {
    pub(in crate::core::db) fn walk_process(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let id = self.register(
            parent,
            &props,
            NodeKind::Process {
                kind: ProcessKind::Always {
                    always_type: AlwaysKind::Unknown(0),
                },
            },
        );
        // `initial`/`final` report vpiAlwaysType=1 like `always`; the object
        // type is what distinguishes them.
        let kind = match vpi::obj_type(h) {
            vpi::vpiInitial => ProcessKind::Initial,
            vpi::vpiFinal => ProcessKind::Final,
            _ => ProcessKind::Always {
                always_type: capture::statements::always_kind(h),
            },
        };
        let mut kids: Vec<NodeId> = Vec::new();
        if let Some(stmt) = child(vpi::vpiStmt, h) {
            kids.push(self.walk_node(stmt.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        self.set_kind(id, NodeKind::Process { kind });
        Ok(id)
    }

    pub(in crate::core::db) fn walk_opt_stmt(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        match child(vpi::vpiStmt, h) {
            Some(s) => self.walk_node(s.raw(), parent),
            None => Ok(self.register(
                parent,
                &CommonProps::default(),
                NodeKind::Stmt(StmtKind::Empty),
            )),
        }
    }

    pub(in crate::core::db) fn walk_event_control(
        &mut self,
        h: VpiHandle,
        id: NodeId,
    ) -> Result<(), DbError> {
        let mut kids: Vec<NodeId> = Vec::new();
        let mut specs: Vec<EventSpec> = Vec::new();
        let mut implicit = false;
        match child(vpi::vpiCondition, h) {
            Some(cond) => {
                let mut stack = vec![cond];
                while let Some(node_handle) = stack.pop() {
                    let node = node_handle.raw();
                    let nt = vpi::obj_type(node);
                    if nt == vpi::vpiOperation {
                        let op = vpi::get(vpi::vpiOpType, node);
                        match op {
                            vpi::vpiEventOrOp => {
                                if let Some(operands) = node_handle.iterate(vpi::vpiOperand) {
                                    stack.extend(operands);
                                }
                            }
                            vpi::vpiPosedgeOp | vpi::vpiNegedgeOp => {
                                if let Some(sig) = iter(vpi::vpiOperand, node).into_iter().next() {
                                    let sid = self.walk_node(sig.raw(), Some(id))?;
                                    kids.push(sid);
                                    specs.push(EventSpec::Edge {
                                        sig: sid,
                                        posedge: op == vpi::vpiPosedgeOp,
                                    });
                                }
                            }
                            _ => {
                                // Unusual op in an event expression: capture
                                // its subtree, but not as a sensitivity spec.
                                kids.push(self.walk_node(node, Some(id))?);
                            }
                        }
                    } else if is_event_operand(nt) {
                        let sid = self.walk_node(node, Some(id))?;
                        kids.push(sid);
                        // A ref resolving to a captured named_event waits on
                        // the EVENT (LRM 1364-1995 §9.7.3), not on a signal
                        // value; both the ref-wrapped and the direct
                        // named_event shapes normalize to `Ref` nodes.
                        let event_target = match self.nodes[sid.0 as usize].kind {
                            NodeKind::Expr(ExprKind::Ref { target: Some(t) })
                                if matches!(
                                    self.nodes[t.0 as usize].kind,
                                    NodeKind::NamedEvent
                                ) =>
                            {
                                Some(t)
                            }
                            _ => None,
                        };
                        match event_target {
                            Some(t) => specs.push(EventSpec::Named(t)),
                            None => specs.push(EventSpec::AnyChange { sig: sid }),
                        }
                    } else {
                        // Unrecognised event operand: capture as Other.
                        kids.push(self.walk_node(node, Some(id))?);
                    }
                }
            }
            None => implicit = true,
        }
        let body = self.walk_opt_stmt(h, Some(id))?;
        kids.push(body);
        self.set_children(id, kids);
        self.set_stmt(
            id,
            StmtKind::EventControl {
                specs,
                implicit,
                body: Some(body),
            },
        );
        Ok(())
    }

    pub(in crate::core::db) fn recover_delay_source(&self, dc: VpiHandle) -> Option<String> {
        self.recover_delay_source_impl(dc)
    }

    fn recover_assignment_delay_source(
        &self,
        assignment: VpiHandle,
        dc: VpiHandle,
    ) -> Option<String> {
        let file = vpi::obj_file(dc);
        let line = vpi::obj_line(dc).max(1) as usize;
        let content = std::fs::read_to_string(file).ok()?;
        let line_start = content
            .match_indices('\n')
            .take(line.saturating_sub(1))
            .last()
            .map_or(0, |(index, _)| index + 1);
        let line_end = content[line_start..]
            .find('\n')
            .map_or(content.len(), |index| line_start + index);
        let dc_col = vpi::get(vpi::vpiColumnNo, dc).max(1) as usize - 1;
        let dc_offset = line_start.saturating_add(dc_col);
        if content.as_bytes().get(dc_offset) == Some(&b'#') {
            return Self::delay_source_after_hash(&content, dc_offset);
        }

        // Surelog points some identifier-valued controls (`#P`) at the
        // assignment instead of at '#'. In that shape, accept '#' only when
        // it is the first token after this assignment's operator.
        let assignment_col = vpi::get(vpi::vpiColumnNo, assignment).max(1) as usize - 1;
        let anchor = line_start.saturating_add(assignment_col).min(line_end);
        let statement_end = content[anchor..line_end]
            .find(';')
            .map_or(line_end, |index| anchor + index);
        let assignment_operator = anchor + content[anchor..statement_end].find('=')?;
        let mut control = assignment_operator + 1;
        while matches!(content.as_bytes().get(control), Some(b' ') | Some(b'\t')) {
            control += 1;
        }
        (content.as_bytes().get(control) == Some(&b'#'))
            .then(|| Self::delay_source_after_hash(&content, control))
            .flatten()
    }

    fn recover_delay_source_impl(&self, dc: VpiHandle) -> Option<String> {
        let file = vpi::obj_file(dc);
        if file.is_empty() {
            return None;
        }
        let line = vpi::obj_line(dc).max(1) as usize;
        let content = std::fs::read_to_string(&file).ok()?;
        let line_start = content
            .match_indices('\n')
            .take(line.saturating_sub(1))
            .last()
            .map_or(0, |(index, _)| index + 1);
        let line_end = content[line_start..]
            .find('\n')
            .map_or(content.len(), |index| line_start + index);
        let col = vpi::get(vpi::vpiColumnNo, dc).max(1) as usize - 1;
        let preferred = line_start.saturating_add(col);
        let bytes = content.as_bytes();
        let preceding_token_hash = || {
            let mut cursor = preferred.min(line_end);
            while cursor > line_start
                && matches!(bytes[cursor - 1], b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' | b'\'' | b'.')
            {
                cursor -= 1;
            }
            while cursor > line_start && matches!(bytes[cursor - 1], b' ' | b'\t') {
                cursor -= 1;
            }
            (cursor > line_start && bytes[cursor - 1] == b'#').then_some(cursor - 1)
        };
        let hash = if bytes.get(preferred) == Some(&b'#') {
            preferred
        } else if let Some(hash) = preceding_token_hash() {
            hash
        } else {
            line_start + content[line_start..line_end].find('#')?
        };
        Self::delay_source_after_hash(&content, hash)
    }

    fn delay_source_after_hash(content: &str, hash: usize) -> Option<String> {
        let bytes = content.as_bytes();
        let mut start = hash + 1;
        while matches!(
            bytes.get(start),
            Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n')
        ) {
            start += 1;
        }
        if bytes.get(start) == Some(&b'(') {
            let mut depth = 0usize;
            for index in start..bytes.len() {
                match bytes[index] {
                    b'(' => depth += 1,
                    b')' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            return Some(content[start + 1..index].trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
            return None;
        }
        let mut end = start;
        while matches!(bytes.get(end), Some(c) if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'\'' | b'.'))
        {
            end += 1;
        }
        (end > start).then(|| content[start..end].to_string())
    }

    pub(in crate::core::db) fn recover_disable_target_name(&self, h: VpiHandle) -> Option<String> {
        let file = vpi::obj_file(h);
        if file.is_empty() {
            return None;
        }
        let line = vpi::obj_line(h);
        let content = std::fs::read_to_string(&file).ok()?;
        let text = content.lines().nth(line.max(1) as usize - 1)?;
        let col = vpi::get(vpi::vpiColumnNo, h).max(1) as usize;
        // Byte-only scan: `i` steps one byte at a time, so slicing `text`
        // here could land inside a multi-byte UTF-8 codepoint (comments/
        // strings may carry non-ASCII) and panic during Db::build.
        let bytes = text.as_bytes();
        let mut best: Option<(usize, String)> = None;
        let mut i = 0usize;
        while i + 7 <= bytes.len() {
            if bytes[i..].starts_with(b"disable")
                && (i == 0 || !is_ident_byte(bytes[i - 1]))
                && (i + 7 == bytes.len() || !is_ident_byte(bytes[i + 7]))
            {
                let mut j = i + 7;
                while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
                    j += 1;
                }
                if j < bytes.len() && is_ident_start(bytes[j]) {
                    let start = j;
                    while j < bytes.len() && is_ident_byte(bytes[j]) {
                        j += 1;
                    }
                    // `col` counts characters while `start` is a byte offset;
                    // on lines with non-ASCII text the two disagree, so this
                    // distance is only a nearest-occurrence tie-break.
                    let dist = start.abs_diff(col.saturating_sub(1));
                    // Identifier bytes are ASCII by construction
                    // (`is_ident_start`/`is_ident_byte`); validate anyway so
                    // extraction stays panic-free by construction.
                    if let Ok(name) = std::str::from_utf8(&bytes[start..j]) {
                        if best.as_ref().map(|(d, _)| dist < *d).unwrap_or(true) {
                            best = Some((dist, name.to_string()));
                        }
                    }
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
        best.map(|(_, name)| name)
    }

    pub(in crate::core::db) fn resolve_disable_target(
        &self,
        name: &str,
        mut scope: Option<NodeId>,
    ) -> Option<NodeId> {
        while let Some(s) = scope {
            let node = &self.nodes[s.0 as usize];
            let candidate = match &node.kind {
                NodeKind::FuncTask { .. } => true,
                NodeKind::Stmt(StmtKind::Begin) | NodeKind::Stmt(StmtKind::Fork { .. }) => {
                    !node.name.is_empty()
                }
                _ => false,
            };
            if candidate && node.name == name {
                return Some(s);
            }
            scope = node.parent;
        }
        None
    }

    pub(in crate::core::db) fn assign_control(&self, h: VpiHandle) -> Option<IntraControl> {
        // Explicit event/repeat children would be unambiguous; the pinned Surelog
        // models those forms as a delay_control too, so this is only a
        // future-proof fast path.
        if child(vpi::vpiEventControl, h).is_some() || child(vpi::vpiRepeatControl, h).is_some() {
            return Some(IntraControl::EventOrRepeat);
        }
        let dc = child(vpi::vpiDelayControl, h)?;
        let source = self.recover_assignment_delay_source(h, dc.raw());
        match source {
            Some(source) if source.bytes().all(|byte| byte.is_ascii_digit()) => source
                .parse()
                .ok()
                .map(IntraControl::Ticks)
                .or(Some(IntraControl::UnresolvedDelay)),
            Some(source) => Some(IntraControl::Expression(source)),
            None => Some(IntraControl::EventOrRepeat),
        }
    }
}
