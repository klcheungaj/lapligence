//! What a generated model can annotate: its instance tree with cell types
//! and ports, and per instance the declared specify paths and timing checks
//! with the model rows they became.
//!
//! Lowering builds the targets from the owned database and the model's
//! [`IrModulePathSet`]s and [`IrTimingCheckSet`]s; the resolver only reads
//! them. Rows refer to model indices: `set`/`destination`/`path` of
//! `IrModel::module_paths`, `set`/`check` of `IrModel::timing_checks`. A
//! declaration whose rows were dropped by lowering (an undriven path
//! destination, a duplicate or never-reporting check) keeps its entry with
//! fewer rows, so an annotation still matches it.
//!
//! [`IrModulePathSet`]: crate::sim::ir::IrModulePathSet
//! [`IrTimingCheckSet`]: crate::sim::ir::IrTimingCheckSet

use super::expr::{write_identifier, CondExpr};
use super::plan::SdfLimitSlot;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfPortDirection {
    Input,
    Output,
    Inout,
    /// `ref` and other port kinds, which no SDF construct annotates.
    Other,
}

/// A port of a module or primitive instance. `range` is the declared
/// packed range `[msb:lsb]`, `None` for a scalar port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfTargetPort {
    pub name: String,
    pub direction: SdfPortDirection,
    pub range: Option<(i64, i64)>,
}

/// A specify path or timing check terminal: a signal name and the declared
/// indices it selects (`None` for the whole signal).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SdfTerminal {
    pub name: String,
    pub range: Option<(i64, i64)>,
}

impl fmt::Display for SdfTerminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_identifier(f, &self.name)?;
        match self.range {
            Some((msb, lsb)) if msb == lsb => write!(f, "[{msb}]"),
            Some((msb, lsb)) => write!(f, "[{msb}:{lsb}]"),
            None => Ok(()),
        }
    }
}

/// The source edge of a path (30.4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfTargetEdge {
    None,
    Posedge,
    Negedge,
    /// `edge`: either.
    Any,
}

/// The condition of a path or check event. `If(None)` is a condition whose
/// text is outside the SDF condition subset: it matches no SDF COND.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdfTargetCondition {
    None,
    If(Option<CondExpr>),
    IfNone,
}

/// One model path row of a declaration: input terminal `input` to output
/// terminal `output` of the declaration became path `path` of destination
/// `destination` of path set `set`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SdfPathRow {
    pub input: u32,
    pub output: u32,
    pub set: u32,
    pub destination: u32,
    pub path: u32,
}

/// A declared specify path (`(a, b *> y) = ...;` is one declaration).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfTargetPath {
    /// `file:line:col` of the declaration.
    pub origin: String,
    pub inputs: Vec<SdfTerminal>,
    pub outputs: Vec<SdfTerminal>,
    pub edge: SdfTargetEdge,
    pub condition: SdfTargetCondition,
    pub rows: Vec<SdfPathRow>,
}

/// The declared kinds of system timing checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfTimingCheckKind {
    Setup,
    Hold,
    SetupHold,
    Recovery,
    Removal,
    RecRem,
    Skew,
    TimeSkew,
    FullSkew,
    Period,
    Width,
    NoChange,
}

impl SdfTimingCheckKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Setup => "$setup",
            Self::Hold => "$hold",
            Self::SetupHold => "$setuphold",
            Self::Recovery => "$recovery",
            Self::Removal => "$removal",
            Self::RecRem => "$recrem",
            Self::Skew => "$skew",
            Self::TimeSkew => "$timeskew",
            Self::FullSkew => "$fullskew",
            Self::Period => "$period",
            Self::Width => "$width",
            Self::NoChange => "$nochange",
        }
    }
}

/// A reference or data event: its terminal, edge mask (the bits of
/// `TimingEdges`, 0 for any transition) and `&&&` condition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfCheckEvent {
    pub terminal: SdfTerminal,
    pub edges: u8,
    pub condition: SdfTargetCondition,
}

/// One model row of a declared check: check `check` of timing check set
/// `set`, whose `limit` holds the declared limit `slot` (its `limit2` holds
/// the second limit when `slot` is [`SdfLimitSlot::First`] of a check with
/// two limits that lowered to one row).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SdfCheckRow {
    pub set: u32,
    pub check: u32,
    pub slot: SdfLimitSlot,
}

/// A declared timing check. `rows` is empty when lowering kept no row (a
/// duplicate, or a `$setuphold`/`$recrem` whose window cannot report).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfTargetCheck {
    pub origin: String,
    pub kind: SdfTimingCheckKind,
    pub reference: SdfCheckEvent,
    /// `None` for `$width` and `$period`.
    pub data: Option<SdfCheckEvent>,
    /// `$setuphold`/`$recrem` timestamp and timecheck conditions.
    pub stamp_condition: SdfTargetCondition,
    pub check_condition: SdfTargetCondition,
    pub rows: Vec<SdfCheckRow>,
    /// The check is a `$setuphold` or `$recrem` whose limits enter the
    /// delayed signal delay solve of its module (negative timing checks
    /// enabled, IEEE 1800-2009 31.9.1), whether or not the solve gave
    /// nonzero delays: a new limit must re-solve the delays of every
    /// delayed signal of the module, not only change the limit.
    pub delayed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfScopeKind {
    /// A module instance (its cell type is the module name).
    Module,
    /// A gate or user-defined primitive instance (the primitive name).
    Primitive,
    /// A generate block: a level of the hierarchy without a cell type.
    Generate,
}

/// One level of the instance tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfScope {
    /// The name as declared; for an element of an instance array or a loop
    /// generate block, the array's name with `index` set.
    pub name: String,
    pub index: Option<i64>,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    pub kind: SdfScopeKind,
    pub cell_type: String,
    pub ports: Vec<SdfTargetPort>,
    pub paths: Vec<SdfTargetPath>,
    pub checks: Vec<SdfTargetCheck>,
}

impl SdfScope {
    pub fn new(name: String, index: Option<i64>, kind: SdfScopeKind, cell_type: String) -> Self {
        Self {
            name,
            index,
            parent: None,
            children: Vec::new(),
            kind,
            cell_type,
            ports: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
        }
    }

    /// The level as an SDF hierarchy component: `u`, `u[3]`, `esc\.inst`
    /// (characters other than letters, digits and `_` escaped).
    pub fn component(&self) -> String {
        let mut text = escaped(&self.name);
        if let Some(index) = self.index {
            text.push_str(&format!("[{index}]"));
        }
        text
    }
}

/// The annotation targets of a model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SdfTargets {
    scopes: Vec<SdfScope>,
    roots: Vec<u32>,
}

impl SdfTargets {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a scope under `parent` (a top instance without one) and return
    /// its index.
    pub fn add_scope(&mut self, parent: Option<u32>, mut scope: SdfScope) -> u32 {
        let index = self.scopes.len() as u32;
        scope.parent = parent;
        self.scopes.push(scope);
        match parent {
            Some(parent) => self.scopes[parent as usize].children.push(index),
            None => self.roots.push(index),
        }
        index
    }

    pub fn scope(&self, index: u32) -> &SdfScope {
        &self.scopes[index as usize]
    }

    pub fn scope_mut(&mut self, index: u32) -> &mut SdfScope {
        &mut self.scopes[index as usize]
    }

    pub fn scopes(&self) -> &[SdfScope] {
        &self.scopes
    }

    pub fn roots(&self) -> &[u32] {
        &self.roots
    }

    /// The dotted hierarchical name of a scope, from its top instance.
    pub fn path_of(&self, index: u32) -> String {
        let mut parts = Vec::new();
        let mut current = Some(index);
        while let Some(scope) = current {
            parts.push(self.scope(scope).component());
            current = self.scope(scope).parent;
        }
        parts.reverse();
        parts.join(".")
    }

    /// The scope a dotted hierarchical name from a top instance names, in
    /// the notation of [`SdfTargets::path_of`]: components split at an
    /// unescaped `.`, `\` escaping the next character, and a trailing
    /// `[n]` selecting an array element.
    pub fn find(&self, path: &str) -> Option<u32> {
        let mut candidates = self.roots.clone();
        let mut found = None;
        for (name, index) in split_path(path)? {
            let scope = candidates.iter().copied().find(|candidate| {
                let scope = self.scope(*candidate);
                scope.name == name && scope.index == index
            })?;
            found = Some(scope);
            candidates = self.scope(scope).children.clone();
        }
        found
    }

    /// Every scope at or below `root`, depth first in tree order.
    pub fn subtree(&self, root: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(scope) = stack.pop() {
            out.push(scope);
            stack.extend(self.scope(scope).children.iter().rev());
        }
        out
    }

    /// One line per scope, port, path and check: a deterministic listing
    /// for review and tests.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let condition = |condition: &SdfTargetCondition| match condition {
            SdfTargetCondition::None => String::new(),
            SdfTargetCondition::If(Some(expr)) => format!(" if {expr}"),
            SdfTargetCondition::If(None) => " if <not an SDF condition>".to_owned(),
            SdfTargetCondition::IfNone => " ifnone".to_owned(),
        };
        let terminals = |terminals: &[SdfTerminal]| {
            terminals
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let event = |event: &SdfCheckEvent| {
            format!(
                "{}{}{}",
                if event.edges == 0 {
                    String::new()
                } else {
                    format!("edges 0x{:02x} ", event.edges)
                },
                event.terminal,
                condition(&event.condition)
            )
        };
        for index in 0..self.scopes.len() as u32 {
            let scope = self.scope(index);
            let kind = match scope.kind {
                SdfScopeKind::Module => "module",
                SdfScopeKind::Primitive => "primitive",
                SdfScopeKind::Generate => "generate",
            };
            out.push_str(&format!(
                "{} {kind} \"{}\"\n",
                self.path_of(index),
                scope.cell_type
            ));
            for port in &scope.ports {
                let direction = match port.direction {
                    SdfPortDirection::Input => "input",
                    SdfPortDirection::Output => "output",
                    SdfPortDirection::Inout => "inout",
                    SdfPortDirection::Other => "other",
                };
                let range = port
                    .range
                    .map(|(msb, lsb)| format!(" [{msb}:{lsb}]"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  port {direction}{range} {}\n",
                    escaped(&port.name)
                ));
            }
            for (number, path) in scope.paths.iter().enumerate() {
                let edge = match path.edge {
                    SdfTargetEdge::None => "",
                    SdfTargetEdge::Posedge => "posedge ",
                    SdfTargetEdge::Negedge => "negedge ",
                    SdfTargetEdge::Any => "edge ",
                };
                let rows = path
                    .rows
                    .iter()
                    .map(|row| {
                        format!(
                            "{}>{}@{}.{}.{}",
                            row.input, row.output, row.set, row.destination, row.path
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!(
                    "  path {number}: ({edge}{} => {}){} rows [{rows}]\n",
                    terminals(&path.inputs),
                    terminals(&path.outputs),
                    condition(&path.condition)
                ));
            }
            for (number, check) in scope.checks.iter().enumerate() {
                let rows = check
                    .rows
                    .iter()
                    .map(|row| {
                        let slot = match row.slot {
                            SdfLimitSlot::First => "first",
                            SdfLimitSlot::Second => "second",
                            SdfLimitSlot::Both => "both",
                        };
                        format!("{}.{}:{slot}", row.set, row.check)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!(
                    "  check {number}: {} reference {}{}{}{}{} rows [{rows}]\n",
                    check.kind.name(),
                    event(&check.reference),
                    check
                        .data
                        .as_ref()
                        .map(|data| format!(" data {}", event(data)))
                        .unwrap_or_default(),
                    match &check.stamp_condition {
                        SdfTargetCondition::None => String::new(),
                        other => format!(" stamp{}", condition(other)),
                    },
                    match &check.check_condition {
                        SdfTargetCondition::None => String::new(),
                        other => format!(" check{}", condition(other)),
                    },
                    if check.delayed { " delayed" } else { "" },
                ));
            }
        }
        out
    }
}

/// `name` with every character other than a letter, digit or `_` escaped.
pub(super) fn escaped(name: &str) -> String {
    let mut text = String::with_capacity(name.len());
    // Writing to a String cannot fail.
    let _ = write_identifier(&mut text, name);
    text
}

/// Split a [`SdfTargets::path_of`] name into unescaped components and
/// element indices; `None` when it is malformed.
fn split_path(path: &str) -> Option<Vec<(String, Option<i64>)>> {
    let mut components = Vec::new();
    let mut name = String::new();
    let mut index: Option<i64> = None;
    let mut characters = path.chars();
    let mut finish = |name: &mut String, index: &mut Option<i64>| {
        if name.is_empty() {
            return false;
        }
        components.push((std::mem::take(name), index.take()));
        true
    };
    while let Some(character) = characters.next() {
        match character {
            '\\' if index.is_none() => name.push(characters.next()?),
            '.' => {
                if !finish(&mut name, &mut index) {
                    return None;
                }
            }
            '[' if index.is_none() => {
                let mut digits = String::new();
                loop {
                    match characters.next()? {
                        ']' => break,
                        digit => digits.push(digit),
                    }
                }
                index = Some(digits.parse().ok()?);
                // Only a divider or the end may follow an element index.
                match characters.clone().next() {
                    None | Some('.') => {}
                    Some(_) => return None,
                }
            }
            _ if index.is_none() => name.push(character),
            _ => return None,
        }
    }
    if !finish(&mut name, &mut index) {
        return None;
    }
    Some(components)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_names_round_trip_through_find() {
        let mut targets = SdfTargets::new();
        let top = targets.add_scope(
            None,
            SdfScope::new("top".into(), None, SdfScopeKind::Module, "top".into()),
        );
        let escaped = targets.add_scope(
            Some(top),
            SdfScope::new("a.b[0]".into(), None, SdfScopeKind::Module, "M".into()),
        );
        let element = targets.add_scope(
            Some(escaped),
            SdfScope::new("g".into(), Some(3), SdfScopeKind::Generate, String::new()),
        );
        assert_eq!(targets.path_of(escaped), "top.a\\.b\\[0\\]");
        assert_eq!(targets.path_of(element), "top.a\\.b\\[0\\].g[3]");
        for scope in [top, escaped, element] {
            assert_eq!(targets.find(&targets.path_of(scope)), Some(scope));
        }
        for path in [
            "top.a.b[0]",
            "top.a\\.b[0]",
            "top.a\\.b\\[0\\].g",
            "top.a\\.b\\[0\\].g[3",
            "",
            "top.",
            ".top",
            "top\\",
        ] {
            assert_eq!(targets.find(path), None, "{path}");
        }
        assert_eq!(targets.subtree(top), vec![top, escaped, element]);
    }
}
