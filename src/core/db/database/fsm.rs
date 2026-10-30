//! FSM recognition pragmas (IEEE 1800-2009 40.4) bound to the signals and
//! parameters of each elaborated instance.
//!
//! The wrapper reports every comment whose text starts with the word `tool`,
//! the declaration a comment directly precedes the first declarator of, and
//! the instances of each definition that holds such a comment. This module
//! parses the `tool state_vector` and `tool enum` pragmas, checks their
//! placement, and resolves their names in each instance: one [`Fsm`] per
//! state vector and instance. Comments whose second word is another one are
//! not FSM pragmas and are ignored. A malformed or unresolved pragma is an
//! [`FsmPragmaError`] (the simulator rejects the design with it), never a
//! silently missing FSM.

use super::*;
use crate::ffi::slang::{FsmDeclaratorKind, FsmPragma, SourceRange};

/// One part of an FSM state expression: a whole signal, or a part-select of
/// one with the bounds as written (40.4.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsmStatePart {
    pub signal: NodeId,
    pub select: Option<(i64, i64)>,
}

/// One FSM of one instance (40.4): the state expression (a signal, a
/// part-select or a concatenation, most significant part first), the
/// optional next-state signal and the parameters that name its states, in
/// declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fsm {
    pub instance: NodeId,
    /// The FSM name: the state signal's name, or the name a part-select or
    /// concatenation pragma gives (40.4.2).
    pub name: String,
    pub enumeration: String,
    pub state: Vec<FsmStatePart>,
    pub next_state: Option<NodeId>,
    pub states: Vec<NodeId>,
    /// Position of the `tool state_vector` comment.
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// A malformed, misplaced or unresolved FSM pragma.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsmPragmaError {
    pub file: String,
    /// One-based line and UTF-16 column of the comment.
    pub line: u32,
    pub column: u32,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Signal(String),
    Select(String, i64, i64),
    Concat(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Parsed {
    StateVector {
        target: Target,
        name: Option<String>,
        enumeration: Option<String>,
    },
    Enum(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Number(i64),
    Punct(char),
}

fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some(&(at, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_alphabetic() || c == '_' {
            let mut end = at;
            while let Some(&(index, c)) = chars.peek() {
                if !(c.is_ascii_alphanumeric() || c == '_' || c == '$') {
                    break;
                }
                end = index + c.len_utf8();
                chars.next();
            }
            out.push(Token::Word(text[at..end].to_owned()));
        } else if c.is_ascii_digit() {
            let mut end = at;
            while let Some(&(index, c)) = chars.peek() {
                if !c.is_ascii_digit() {
                    break;
                }
                end = index + 1;
                chars.next();
            }
            let value = text[at..end]
                .parse::<i64>()
                .map_err(|_| format!("the bound `{}` is too large", &text[at..end]))?;
            out.push(Token::Number(value));
        } else if matches!(c, '[' | ']' | ':' | '{' | '}' | ',') {
            out.push(Token::Punct(c));
            chars.next();
        } else {
            return Err(format!("unexpected `{c}`"));
        }
    }
    Ok(out)
}

/// Parse the text of a comment that starts with `tool`. `None` when the next
/// word is neither `state_vector` nor `enum`: another tool's pragma.
fn parse(text: &str) -> Option<Result<Parsed, String>> {
    let mut words = text.split_whitespace();
    if words.next() != Some("tool") {
        return None;
    }
    let keyword = words.next()?;
    if keyword != "state_vector" && keyword != "enum" {
        return None;
    }
    let rest = text
        .trim_start()
        .strip_prefix("tool")?
        .trim_start()
        .strip_prefix(keyword)?;
    Some(tokens(rest).and_then(|tokens| {
        if keyword == "enum" {
            parse_enum(&tokens)
        } else {
            parse_state_vector(&tokens)
        }
    }))
}

fn word(tokens: &[Token], at: usize) -> Option<&str> {
    match tokens.get(at) {
        Some(Token::Word(word)) if word != "enum" => Some(word),
        _ => None,
    }
}

fn describe(token: Option<&Token>) -> String {
    match token {
        None => "the end of the pragma".to_owned(),
        Some(Token::Word(word)) => format!("`{word}`"),
        Some(Token::Number(value)) => format!("`{value}`"),
        Some(Token::Punct(c)) => format!("`{c}`"),
    }
}

fn parse_enum(tokens: &[Token]) -> Result<Parsed, String> {
    match (word(tokens, 0), tokens.get(1)) {
        (Some(name), None) => Ok(Parsed::Enum(name.to_owned())),
        (Some(_), extra) => Err(format!(
            "expected the end of the pragma, found {}",
            describe(extra)
        )),
        (None, found) => Err(format!(
            "`tool enum` needs an enumeration name, found {}",
            describe(found)
        )),
    }
}

fn parse_state_vector(tokens: &[Token]) -> Result<Parsed, String> {
    let mut at;
    let target = if tokens.first() == Some(&Token::Punct('{')) {
        let mut signals = Vec::new();
        at = 1;
        loop {
            let Some(signal) = word(tokens, at) else {
                return Err(format!(
                    "expected a signal name in the concatenation, found {} (bit-selects and \
                     part-selects cannot be concatenated, IEEE 1800-2009 40.4.3)",
                    describe(tokens.get(at))
                ));
            };
            signals.push(signal.to_owned());
            at += 1;
            match tokens.get(at) {
                Some(Token::Punct(',')) => at += 1,
                Some(Token::Punct('}')) => {
                    at += 1;
                    break;
                }
                other => {
                    return Err(format!(
                        "expected `,` or `}}` in the concatenation, found {}",
                        describe(other)
                    ))
                }
            }
        }
        Target::Concat(signals)
    } else {
        let Some(signal) = word(tokens, 0) else {
            return Err(format!(
                "`tool state_vector` needs a signal, part-select or concatenation, found {}",
                describe(tokens.first())
            ));
        };
        at = 1;
        if tokens.get(1) == Some(&Token::Punct('[')) {
            match (tokens.get(2), tokens.get(3), tokens.get(4), tokens.get(5)) {
                (
                    Some(Token::Number(left)),
                    Some(Token::Punct(':')),
                    Some(Token::Number(right)),
                    Some(Token::Punct(']')),
                ) => {
                    at = 6;
                    Target::Select(signal.to_owned(), *left, *right)
                }
                _ => {
                    return Err(
                        "a state vector part-select is `[msb:lsb]` with decimal bounds \
                         (IEEE 1800-2009 40.4.2)"
                            .to_owned(),
                    )
                }
            }
        } else {
            Target::Signal(signal.to_owned())
        }
    };
    let name = if matches!(target, Target::Signal(_)) {
        None
    } else {
        let name = word(tokens, at).ok_or_else(|| {
            format!(
                "a part-select or concatenation state vector needs an FSM name, found {} \
                 (IEEE 1800-2009 40.4.2, 40.4.3)",
                describe(tokens.get(at))
            )
        })?;
        at += 1;
        Some(name.to_owned())
    };
    let enumeration = match tokens.get(at) {
        None => None,
        Some(Token::Word(keyword)) if keyword == "enum" => {
            let enumeration = word(tokens, at + 1).ok_or_else(|| {
                format!(
                    "`enum` needs an enumeration name, found {}",
                    describe(tokens.get(at + 1))
                )
            })?;
            at += 2;
            Some(enumeration.to_owned())
        }
        Some(other) => {
            return Err(format!(
                "expected `enum` or the end of the pragma, found {}",
                describe(Some(other))
            ))
        }
    };
    if let Some(extra) = tokens.get(at) {
        return Err(format!(
            "expected the end of the pragma, found {}",
            describe(Some(extra))
        ));
    }
    if name.is_some() && enumeration.is_none() {
        return Err(
            "a part-select or concatenation state vector needs `enum` and an enumeration name \
             (IEEE 1800-2009 40.4.2, 40.4.3)"
                .to_owned(),
        );
    }
    Ok(Parsed::StateVector {
        target,
        name,
        enumeration,
    })
}

/// One pragma comment with the declarators it is bound to.
struct Comment<'a> {
    range: SourceRange,
    scope: Option<SourceRange>,
    text: &'a str,
    line_comment: bool,
    declarators: Vec<(FsmDeclaratorKind, SourceRange)>,
}

fn comments(pragmas: &[FsmPragma]) -> Vec<Comment<'_>> {
    let mut out: Vec<Comment<'_>> = Vec::new();
    for pragma in pragmas {
        if let Some(last) = out.last_mut().filter(|last| last.range == pragma.range) {
            if let Some(declarator) = pragma.declarator {
                last.declarators.push((declarator.kind, declarator.name));
            }
            continue;
        }
        out.push(Comment {
            range: pragma.range,
            scope: pragma.scope,
            text: &pragma.text,
            line_comment: pragma.line_comment,
            declarators: pragma
                .declarator
                .map(|declarator| vec![(declarator.kind, declarator.name)])
                .unwrap_or_default(),
        });
    }
    out
}

/// A parsed pragma of one definition, with its position.
struct Located {
    parsed: Parsed,
    declarators: Vec<(FsmDeclaratorKind, SourceRange)>,
    file: String,
    line: u32,
    column: u32,
}

type ScopeKey = (u64, u64, u64);

fn key(range: SourceRange) -> ScopeKey {
    (range.file_id, range.start, range.end)
}

/// The FSMs of every instance with pragmas, and the pragma errors.
pub(super) struct FsmTables {
    pub(super) fsms: Vec<Fsm>,
    pub(super) errors: Vec<FsmPragmaError>,
}

/// The members of one instance a pragma can name: module-level signals by
/// name, and signals and parameters by declared-name range.
struct Members {
    by_name: HashMap<String, NodeId>,
    by_range: HashMap<ScopeKey, NodeId>,
}

fn members(snapshot: &SlangSnapshot, nodes: &[Node], instance: NodeId) -> Members {
    let mut by_name = HashMap::new();
    let mut by_range = HashMap::new();
    for &child in &nodes[instance.index()].children {
        let node = &nodes[child.index()];
        let signal = matches!(node.kind, NodeKind::Var { .. } | NodeKind::Net { .. });
        if !signal && !matches!(node.kind, NodeKind::Param { .. }) {
            continue;
        }
        if signal {
            by_name.entry(node.name.clone()).or_insert(child);
        }
        if let Some(range) = snapshot
            .semantic_nodes
            .get(child.index())
            .and_then(|semantic| semantic.range)
        {
            by_range.entry(key(range)).or_insert(child);
        }
    }
    Members { by_name, by_range }
}

impl FsmTables {
    pub(super) fn from_slang(
        snapshot: &SlangSnapshot,
        nodes: &[Node],
        positions: &mut SourcePositions<'_>,
    ) -> Result<Self, DbError> {
        let mut tables = FsmTables {
            fsms: Vec::new(),
            errors: Vec::new(),
        };
        if snapshot.fsm_pragmas.is_empty() {
            return Ok(tables);
        }
        let mut scopes: HashMap<ScopeKey, Vec<Located>> = HashMap::new();
        let mut order: Vec<ScopeKey> = Vec::new();
        for comment in comments(&snapshot.fsm_pragmas) {
            let file = snapshot
                .files
                .iter()
                .find(|file| file.id == comment.range.file_id)
                .ok_or_else(|| DbError::InvalidSnapshot("FSM pragma file is missing".into()))?;
            let delimiters = if comment.line_comment {
                ("//", "")
            } else {
                ("/*", "*/")
            };
            let written = usize::try_from(comment.range.start)
                .ok()
                .zip(usize::try_from(comment.range.end).ok())
                .and_then(|(start, end)| file.text.get(start..end));
            if written != Some(&format!("{}{}{}", delimiters.0, comment.text, delimiters.1)) {
                return Err(DbError::InvalidSnapshot(
                    "FSM pragma text differs from its source".into(),
                ));
            }
            let Some(parsed) = parse(comment.text) else {
                continue;
            };
            let (file, line, column, _, _) = positions.range_position(comment.range)?;
            let file = file.map(|file| file.to_string()).unwrap_or_default();
            let error = |message: String| FsmPragmaError {
                file: file.clone(),
                line,
                column,
                message,
            };
            let parsed = match parsed {
                Ok(parsed) => parsed,
                Err(message) => {
                    tables
                        .errors
                        .push(error(format!("malformed FSM pragma: {message}")));
                    continue;
                }
            };
            let Some(scope) = comment.scope else {
                tables.errors.push(error(
                    "an FSM pragma must be inside the module that declares its signals \
                     (IEEE 1800-2009 40.4.1)"
                        .to_owned(),
                ));
                continue;
            };
            if matches!(parsed, Parsed::Enum(_)) && comment.declarators.is_empty() {
                tables.errors.push(error(
                    "a `tool enum` pragma must directly follow the type and range of a signal \
                     or parameter declaration (IEEE 1800-2009 40.4.1, 40.4.6)"
                        .to_owned(),
                ));
                continue;
            }
            let entry = scopes.entry(key(scope)).or_insert_with(|| {
                order.push(key(scope));
                Vec::new()
            });
            entry.push(Located {
                parsed,
                declarators: comment.declarators,
                file,
                line,
                column,
            });
        }
        let mut instances: HashMap<ScopeKey, Vec<NodeId>> = HashMap::new();
        for instance in &snapshot.fsm_instances {
            let id = usize::try_from(instance.instance)
                .ok()
                .filter(|index| *index < nodes.len())
                .map(NodeId::from_index)
                .ok_or_else(|| DbError::InvalidSnapshot("FSM instance is out of range".into()))?;
            instances.entry(key(instance.scope)).or_default().push(id);
        }
        for scope in order {
            let pragmas = &scopes[&scope];
            let mut reported = HashSet::new();
            for &instance in instances.get(&scope).map(Vec::as_slice).unwrap_or(&[]) {
                let members = members(snapshot, nodes, instance);
                let mut errors = Vec::new();
                let fsms = bind(pragmas, &members, nodes, instance, &mut errors);
                // Every instance of a definition resolves the same names;
                // report each problem once.
                for error in errors {
                    if reported.insert((error.line, error.column, error.message.clone())) {
                        tables.errors.push(error);
                    }
                }
                tables.fsms.extend(fsms);
            }
        }
        Ok(tables)
    }
}

/// Resolve the pragmas of one definition in one of its instances.
fn bind(
    pragmas: &[Located],
    members: &Members,
    nodes: &[Node],
    instance: NodeId,
    errors: &mut Vec<FsmPragmaError>,
) -> Vec<Fsm> {
    let fail = |errors: &mut Vec<FsmPragmaError>, pragma: &Located, message: String| {
        errors.push(FsmPragmaError {
            file: pragma.file.clone(),
            line: pragma.line,
            column: pragma.column,
            message,
        });
    };
    // Declarations carrying `tool enum`: the declarators as instance members.
    let mut signal_enums: Vec<(&Located, &str, Vec<NodeId>)> = Vec::new();
    let mut parameter_enums: Vec<(&Located, &str, Vec<NodeId>)> = Vec::new();
    for pragma in pragmas {
        let Parsed::Enum(enumeration) = &pragma.parsed else {
            continue;
        };
        let mut bound = Vec::new();
        let mut missing = false;
        let mut kind = FsmDeclaratorKind::Signal;
        for (declarator_kind, range) in &pragma.declarators {
            kind = *declarator_kind;
            let member = members
                .by_range
                .get(&key(*range))
                .copied()
                .filter(|member| {
                    let parameter = matches!(nodes[member.index()].kind, NodeKind::Param { .. });
                    parameter == (kind == FsmDeclaratorKind::Parameter)
                });
            match member {
                Some(member) => bound.push(member),
                None => missing = true,
            }
        }
        if missing {
            fail(
                errors,
                pragma,
                format!(
                    "the `tool enum {enumeration}` pragma must be on a module-level {} \
                     declaration (IEEE 1800-2009 40.4.1, 40.4.6)",
                    if kind == FsmDeclaratorKind::Parameter {
                        "value parameter"
                    } else {
                        "signal"
                    }
                ),
            );
            continue;
        }
        if kind == FsmDeclaratorKind::Parameter {
            parameter_enums.push((pragma, enumeration, bound));
        } else {
            signal_enums.push((pragma, enumeration, bound));
        }
    }
    let definition = match &nodes[instance.index()].kind {
        NodeKind::ModuleInst { def_name, .. } => def_name.as_str(),
        _ => nodes[instance.index()].name.as_str(),
    };
    let mut fsms: Vec<Fsm> = Vec::new();
    for pragma in pragmas {
        let Parsed::StateVector {
            target,
            name,
            enumeration,
        } = &pragma.parsed
        else {
            continue;
        };
        let resolve = |errors: &mut Vec<FsmPragmaError>, signal: &str| {
            let found = members.by_name.get(signal).copied();
            if found.is_none() {
                fail(
                    errors,
                    pragma,
                    format!(
                        "`{signal}` is not a signal of module `{definition}`: an FSM pragma \
                         belongs in the module that declares its state vector (IEEE 1800-2009 \
                         40.4.1)"
                    ),
                );
            }
            found
        };
        let (state, fsm_name, single) = match target {
            Target::Signal(signal) => {
                let Some(node) = resolve(errors, signal) else {
                    continue;
                };
                (
                    vec![FsmStatePart {
                        signal: node,
                        select: None,
                    }],
                    signal.clone(),
                    Some(node),
                )
            }
            Target::Select(signal, left, right) => {
                let Some(node) = resolve(errors, signal) else {
                    continue;
                };
                (
                    vec![FsmStatePart {
                        signal: node,
                        select: Some((*left, *right)),
                    }],
                    name.clone().unwrap_or_default(),
                    None,
                )
            }
            Target::Concat(signals) => {
                let mut parts = Vec::new();
                for signal in signals {
                    let Some(node) = resolve(errors, signal) else {
                        break;
                    };
                    if parts.iter().any(|part: &FsmStatePart| part.signal == node) {
                        fail(
                            errors,
                            pragma,
                            format!("`{signal}` appears twice in the state vector concatenation"),
                        );
                        break;
                    }
                    parts.push(FsmStatePart {
                        signal: node,
                        select: None,
                    });
                }
                if parts.len() != signals.len() {
                    continue;
                }
                (parts, name.clone().unwrap_or_default(), None)
            }
        };
        // 40.4.1: the enumeration name comes with the pragma or from a
        // `tool enum` pragma in the state signal's own declaration.
        let declared = single.and_then(|node| {
            signal_enums
                .iter()
                .find(|(_, _, bound)| bound.first() == Some(&node))
                .map(|(_, enumeration, _)| *enumeration)
        });
        let enumeration = match (enumeration.as_deref(), declared) {
            (Some(written), Some(declared)) if written != declared => {
                fail(
                    errors,
                    pragma,
                    format!(
                        "the state vector names enumeration `{written}` but its declaration \
                         names `{declared}` (IEEE 1800-2009 40.4.1)"
                    ),
                );
                continue;
            }
            (Some(written), _) => written.to_owned(),
            (None, Some(declared)) => declared.to_owned(),
            (None, None) => {
                fail(
                    errors,
                    pragma,
                    format!(
                        "the state vector `{fsm_name}` needs an enumeration name: `enum` in this \
                         pragma or a `tool enum` pragma in its declaration (IEEE 1800-2009 40.4.1)"
                    ),
                );
                continue;
            }
        };
        if fsms.iter().any(|fsm| fsm.enumeration == enumeration) {
            fail(
                errors,
                pragma,
                format!("enumeration `{enumeration}` already names another FSM of this module"),
            );
            continue;
        }
        if fsms.iter().any(|fsm| fsm.name == fsm_name) {
            fail(
                errors,
                pragma,
                format!("FSM `{fsm_name}` is declared twice in this module"),
            );
            continue;
        }
        // 40.4.4, 40.4.5: the next-state signal is the first declarator of a
        // `tool enum` declaration, or the second when the first is the state
        // signal itself.
        let mut next: Vec<NodeId> = Vec::new();
        for (_, declared, bound) in &signal_enums {
            if *declared != enumeration {
                continue;
            }
            let candidate = if bound.first().copied() == single {
                bound.get(1).copied()
            } else {
                bound.first().copied()
            };
            if let Some(candidate) = candidate.filter(|candidate| !next.contains(candidate)) {
                next.push(candidate);
            }
        }
        if next.len() > 1 {
            fail(
                errors,
                pragma,
                format!(
                    "enumeration `{enumeration}` names {} next-state signals (IEEE 1800-2009 \
                     40.4.4)",
                    next.len()
                ),
            );
            continue;
        }
        let states: Vec<NodeId> = parameter_enums
            .iter()
            .filter(|(_, declared, _)| *declared == enumeration)
            .flat_map(|(_, _, bound)| bound.iter().copied())
            .collect();
        if states.is_empty() {
            fail(
                errors,
                pragma,
                format!(
                    "no `parameter /* tool enum {enumeration} */` declaration lists the states \
                     of FSM `{fsm_name}`; llg does not extract FSM states automatically (IEEE \
                     1800-2009 40.4, 40.4.6)"
                ),
            );
            continue;
        }
        fsms.push(Fsm {
            instance,
            name: fsm_name,
            enumeration,
            state,
            next_state: next.first().copied(),
            states,
            file: pragma.file.clone(),
            line: pragma.line,
            column: pragma.column,
        });
    }
    // An enumeration that no state vector claims (by name, or through the
    // declaration of its signal) declares nothing usable.
    let mut claimed: HashSet<&str> = HashSet::new();
    for pragma in pragmas {
        let Parsed::StateVector {
            target,
            enumeration,
            ..
        } = &pragma.parsed
        else {
            continue;
        };
        if let Some(enumeration) = enumeration {
            claimed.insert(enumeration);
        }
        if let Target::Signal(signal) = target {
            let node = members.by_name.get(signal.as_str()).copied();
            for (_, declared, bound) in &signal_enums {
                if node.is_some() && bound.first().copied() == node {
                    claimed.insert(declared);
                }
            }
        }
    }
    for (pragma, enumeration, _) in signal_enums.iter().chain(&parameter_enums) {
        if !claimed.contains(enumeration) {
            fail(
                errors,
                pragma,
                format!(
                    "enumeration `{enumeration}` names no FSM state vector of this module \
                     (IEEE 1800-2009 40.4.1)"
                ),
            );
        }
    }
    fsms
}

impl Db {
    /// Indices in `snapshot.diagnostics` of the frontend's own
    /// unused-parameter warnings for parameters that a `tool enum` pragma
    /// names as FSM states (IEEE 1800-2009 40.4.1): the pragma uses them, so
    /// both binaries drop these warnings from their projections. Matches the
    /// warning's position with the parameter declaration's (byte columns),
    /// using owned data only; empty, without any scan, for a design without
    /// FSMs. `snapshot` must be the one this database was built from.
    pub fn fsm_state_parameter_warnings(&self, snapshot: &SlangSnapshot) -> Vec<usize> {
        if self.fsms.is_empty() {
            return Vec::new();
        }
        let states = self
            .fsms
            .iter()
            .flat_map(|fsm| fsm.states.iter())
            .filter_map(|state| {
                let node = self.node(*state);
                Some((node.file.as_deref()?, node.line, node.col))
            })
            .collect::<HashSet<_>>();
        let files = snapshot
            .files
            .iter()
            .map(|file| (file.id, file))
            .collect::<HashMap<_, _>>();
        let mut lines = HashMap::new();
        snapshot
            .diagnostics
            .iter()
            .enumerate()
            .filter_map(|(index, diagnostic)| {
                if diagnostic.provider != crate::ffi::slang::DiagnosticProvider::Analysis
                    || diagnostic.name != "UnusedParameter"
                {
                    return None;
                }
                let range = diagnostic.primary?;
                let file = files.get(&range.file_id)?;
                let (line, column) = lines
                    .entry(range.file_id)
                    .or_insert_with(|| super::expression_import::SourceLines::new(&file.text))
                    .position(usize::try_from(range.start).ok()?)
                    .ok()?;
                states
                    .contains(&(file.name.as_str(), line, column))
                    .then_some(index)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pragma_text_parses_every_40_4_form() {
        let state = |text: &str| parse(text).expect("an FSM pragma");
        assert_eq!(
            state(" tool state_vector cs "),
            Ok(Parsed::StateVector {
                target: Target::Signal("cs".into()),
                name: None,
                enumeration: None
            })
        );
        assert_eq!(
            state(" tool state_vector cs enum MY_FSM"),
            Ok(Parsed::StateVector {
                target: Target::Signal("cs".into()),
                name: None,
                enumeration: Some("MY_FSM".into())
            })
        );
        assert_eq!(
            state("tool state_vector r[3:1] f enum e"),
            Ok(Parsed::StateVector {
                target: Target::Select("r".into(), 3, 1),
                name: Some("f".into()),
                enumeration: Some("e".into())
            })
        );
        assert_eq!(
            state(" tool state_vector {a , b,c} f enum\n   e "),
            Ok(Parsed::StateVector {
                target: Target::Concat(vec!["a".into(), "b".into(), "c".into()]),
                name: Some("f".into()),
                enumeration: Some("e".into())
            })
        );
        assert_eq!(state(" tool enum myFSM "), Ok(Parsed::Enum("myFSM".into())));
        assert_eq!(parse(" tool chain notes"), None);
        assert_eq!(parse(" tool"), None);
        for (text, message) in [
            ("tool enum", "needs an enumeration name"),
            ("tool enum a b", "expected the end"),
            ("tool state_vector", "needs a signal"),
            ("tool state_vector cs extra", "expected `enum`"),
            ("tool state_vector cs enum", "needs an enumeration name"),
            ("tool state_vector r[3] f enum e", "[msb:lsb]"),
            ("tool state_vector r[3:0] enum e", "needs an FSM name"),
            ("tool state_vector r[3:0] f", "needs `enum`"),
            (
                "tool state_vector {a, b[1]} f enum e",
                "expected `,` or `}`",
            ),
            ("tool state_vector {a, } f enum e", "expected a signal name"),
            ("tool state_vector cs enum e ;", "unexpected `;`"),
            (
                "tool state_vector r[99999999999999999999:0] f enum e",
                "too large",
            ),
        ] {
            let error = state(text).expect_err(text);
            assert!(error.contains(message), "{text}: {error}");
        }
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;

    fn compile(source: &str) -> Db {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "fsm.sv", source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&output.snapshot).unwrap()
    }

    fn names(db: &Db, nodes: &[NodeId]) -> Vec<String> {
        nodes
            .iter()
            .map(|node| db.node(*node).name.clone())
            .collect()
    }

    #[test]
    fn pragmas_bind_signals_states_and_instances() {
        let db = compile(
            "module fsm;\n\
             /* tool state_vector cs */\n\
             logic [1:0] /* tool enum myFSM */ cs, ns, other;\n\
             parameter [1:0] // tool enum myFSM\n  IDLE = 0, RUN = 1, DONE = 2;\n\
             reg [31:0] q;\n\
             reg [31:0] /* tool enum M2 */ qn;\n\
             // tool state_vector q enum M2\n\
             parameter /* tool enum M2 */ p1 = 10, p2 = 11;\n\
             logic [7:0] r; logic a, b;\n\
             /* tool state_vector r[5:4] part enum E3 */\n\
             localparam /* tool enum E3 */ X0 = 0, X1 = 3;\n\
             /* tool state_vector {a, b} pair enum E4 */\n\
             parameter /* tool enum E4 */ Y0 = 1;\n\
             // tool chain: not an FSM pragma\n\
             endmodule\n\
             module top; fsm u1(); fsm u2(); endmodule\n",
        );
        assert_eq!(db.fsm_pragma_errors(), &[]);
        let summary: Vec<_> = db
            .fsms()
            .iter()
            .map(|fsm| {
                (
                    db.node(fsm.instance).full_name.to_string(),
                    fsm.name.clone(),
                    fsm.enumeration.clone(),
                    fsm.state
                        .iter()
                        .map(|part| (db.node(part.signal).name.clone(), part.select))
                        .collect::<Vec<_>>(),
                    fsm.next_state.map(|node| db.node(node).name.clone()),
                    names(&db, &fsm.states),
                    fsm.line,
                )
            })
            .collect();
        let expected = |instance: &str| {
            vec![
                (
                    instance.to_owned(),
                    "cs".to_owned(),
                    "myFSM".to_owned(),
                    vec![("cs".to_owned(), None)],
                    Some("ns".to_owned()),
                    vec!["IDLE".to_owned(), "RUN".to_owned(), "DONE".to_owned()],
                    2,
                ),
                (
                    instance.to_owned(),
                    "q".to_owned(),
                    "M2".to_owned(),
                    vec![("q".to_owned(), None)],
                    Some("qn".to_owned()),
                    vec!["p1".to_owned(), "p2".to_owned()],
                    8,
                ),
                (
                    instance.to_owned(),
                    "part".to_owned(),
                    "E3".to_owned(),
                    vec![("r".to_owned(), Some((5, 4)))],
                    None,
                    vec!["X0".to_owned(), "X1".to_owned()],
                    11,
                ),
                (
                    instance.to_owned(),
                    "pair".to_owned(),
                    "E4".to_owned(),
                    vec![("a".to_owned(), None), ("b".to_owned(), None)],
                    None,
                    vec!["Y0".to_owned()],
                    13,
                ),
            ]
        };
        let mut all = expected("top.u1");
        all.extend(expected("top.u2"));
        assert_eq!(summary, all);
    }

    #[test]
    fn misplaced_and_unresolved_pragmas_are_errors() {
        let db = compile(
            "// tool state_vector outside enum E0\n\
             module fsm;\n\
             logic [1:0] cs, ns;\n\
             /* tool enum lost */ logic [1:0] x;\n\
             /* tool state_vector missing enum E1 */\n\
             parameter /* tool enum E1 */ A = 0;\n\
             /* tool state_vector cs */\n\
             /* tool state_vector ns enum E2 */\n\
             /* tool state_vector cs enum E9 */ /* tool state_vector q[1] f enum e */\n\
             initial begin : blk logic [1:0] /* tool enum E5 */ local_state; end\n\
             parameter /* tool enum Orphan */ B = 1;\n\
             endmodule\n\
             module top; fsm u1(); fsm u2(); endmodule\n",
        );
        let errors: Vec<_> = db
            .fsm_pragma_errors()
            .iter()
            .map(|error| (error.line, error.message.clone()))
            .collect();
        let expect = [
            (1, "must be inside the module"),
            (4, "must directly follow the type"),
            (5, "`missing` is not a signal of module `fsm`"),
            (7, "needs an enumeration name"),
            (8, "no `parameter /* tool enum E2 */`"),
            (9, "no `parameter /* tool enum E9 */`"),
            (9, "[msb:lsb]"),
            (10, "module-level signal"),
            (11, "`Orphan` names no FSM state vector"),
        ];
        assert_eq!(errors.len(), expect.len(), "{errors:#?}");
        for (line, message) in expect {
            assert!(
                errors
                    .iter()
                    .any(|(at, text)| *at == line && text.contains(message)),
                "line {line}: {message} in {errors:#?}"
            );
        }
        assert!(db.fsms().is_empty());
    }

    #[test]
    fn frontend_unused_warnings_of_state_parameters_are_found() {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "fsm.sv",
                "module fsm(input logic clk);\n\
                 /* tool state_vector cs enum E */\n\
                 logic [1:0] cs = 0;\n\
                 parameter [1:0] /* tool enum E */ IDLE = 0, RUN = 1, DONE = 2;\n\
                 parameter UNREAD = 5;\n\
                 always_ff @(posedge clk) cs <= cs + 2'd1;\n\
                 endmodule\n\
                 module top; logic clk; fsm u1(clk); fsm u2(clk); endmodule\n",
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let db = Db::from_slang(&output.snapshot).unwrap();
        let unused = |index: usize| {
            let diagnostic = &output.snapshot.diagnostics[index];
            (diagnostic.name == "UnusedParameter").then(|| diagnostic.message.clone())
        };
        let all = (0..output.snapshot.diagnostics.len())
            .filter_map(unused)
            .collect::<Vec<_>>();
        assert!(
            ["IDLE", "RUN", "DONE", "UNREAD"]
                .iter()
                .all(|name| all.iter().any(|message| message.contains(name))),
            "the frontend reports every unread parameter: {all:?}"
        );
        let mut quiet = db
            .fsm_state_parameter_warnings(&output.snapshot)
            .into_iter()
            .map(|index| unused(index).expect("an unused-parameter warning"))
            .collect::<Vec<_>>();
        quiet.sort();
        quiet.dedup();
        assert_eq!(quiet.len(), 3, "{quiet:?}");
        for name in ["IDLE", "RUN", "DONE"] {
            assert!(
                quiet.iter().any(|message| message.contains(name)),
                "{quiet:?}"
            );
        }
        assert!(!quiet.iter().any(|message| message.contains("UNREAD")));
        let plain = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "plain.sv",
                "module top; parameter P = 1; endmodule\n",
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let plain_db = Db::from_slang(&plain.snapshot).unwrap();
        assert!(plain_db
            .fsm_state_parameter_warnings(&plain.snapshot)
            .is_empty());
    }
}
