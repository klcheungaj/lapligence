//! Exact sharing of owned emitted bodies; unrepresented operands stay in the key.
use super::super::owned::model::{SharedSpawns, SpawnClass};
use super::*;
use crate::sim::ir::*;
use std::collections::HashSet;
use std::fmt::Write;
use std::rc::Rc;

pub(super) const DEFAULT_SHARE_MIN_INSTANCES: usize = 4;
const SELF: &str = "llg_body_self";

pub(super) fn threshold() -> Result<usize, String> {
    match std::env::var("LLG_SHARE_MIN_INSTANCES") {
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_SHARE_MIN_INSTANCES),
        Ok(value) if value == "unlimited" => Ok(usize::MAX),
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| {
                "LLG_SHARE_MIN_INSTANCES must be a positive integer or unlimited".to_owned()
            }),
        Err(error) => Err(format!("LLG_SHARE_MIN_INSTANCES: {error}")),
    }
}

/// Declaration, layout shape and record access of an operand. One value is
/// shared by every occurrence of a registry entry or scalar type.
struct OperandKind {
    declaration: String,
    shape: String,
    access: &'static str,
}

/// One normalized body operand. Every sharing candidate keeps its operands
/// until its group is resolved, so an operand is two reference-counted
/// pointers rather than four owned strings per occurrence.
#[derive(Clone)]
struct Operand {
    kind: Rc<OperandKind>,
    value: Rc<str>,
}

impl Operand {
    fn new(declaration: String, shape: String, value: String, access: &'static str) -> Self {
        Self {
            kind: Rc::new(OperandKind {
                declaration,
                shape,
                access,
            }),
            value: value.into(),
        }
    }
    fn pointer(ty: &str, shape: String, name: &str) -> Self {
        Self::new(format!("{ty}* @"), shape, format!("&{name}"), "(*I->@)")
    }
    #[cfg(test)]
    fn scalar(ty: &str, value: String) -> Self {
        Self::new(format!("{ty} @"), ty.to_owned(), value, "I->@")
    }
    fn declaration(&self) -> &str {
        &self.kind.declaration
    }
    fn shape(&self) -> &str {
        &self.kind.shape
    }
    fn access(&self) -> &'static str {
        self.kind.access
    }
}

/// Scalar operand kinds and literal values interned across one sharing pass:
/// the same literals recur in every instance of a generated body.
#[derive(Default)]
struct ScalarOperands {
    kinds: HashMap<&'static str, Rc<OperandKind>>,
    values: HashSet<Rc<str>>,
}

impl ScalarOperands {
    fn operand(&mut self, ty: &'static str, value: &str) -> Operand {
        let kind = self
            .kinds
            .entry(ty)
            .or_insert_with(|| {
                Rc::new(OperandKind {
                    declaration: format!("{ty} @"),
                    shape: ty.to_owned(),
                    access: "I->@",
                })
            })
            .clone();
        let value = match self.values.get(value) {
            Some(value) => value.clone(),
            None => {
                let value = Rc::<str>::from(value);
                self.values.insert(value.clone());
                value
            }
        };
        Operand { kind, value }
    }
}

fn registry(
    model: &IrModel,
    functions: &CoroutineArtifacts,
    branches: &BTreeMap<CoroutineId, CoroutineArtifact>,
) -> HashMap<String, Operand> {
    let mut registry = HashMap::new();
    for signal in &model.signals {
        if signal.net_driver.is_none()
            && signal.alias.is_none()
            && (!signal.omit || !signal.net_alias.is_empty())
        {
            registry.insert(
                signal.c_name.clone(),
                Operand::pointer(
                    if signal.ty.width() == 0 {
                        "double"
                    } else {
                        "sv4_t"
                    },
                    format!("{:?}", signal.ty),
                    &signal.c_name,
                ),
            );
        }
    }
    for (index, signal) in model.signals.iter().enumerate() {
        if !signal.net_alias.is_empty() {
            let name = format!("llg_net_alias_{index}");
            registry.insert(
                name.clone(),
                Operand::pointer("llg_net_alias_t", format!("{:?}", signal.ty), &name),
            );
        }
    }
    for group in &model.net_groups {
        registry.insert(
            group.c_name.clone(),
            Operand::pointer(
                "llg_net_t",
                format!("{}:{}:{:?}", group.width, group.n_drivers, group.kind),
                &group.c_name,
            ),
        );
        if group.n_drivers > 0 {
            let name = format!("{}__cells", group.c_name);
            registry.insert(
                name.clone(),
                Operand::new(
                    format!("sv4_t (*@)[{}]", group.n_drivers),
                    format!("{}:{}:{}", group.width, group.signed, group.n_drivers),
                    format!("&{name}"),
                    "(*I->@)",
                ),
            );
        }
    }
    // A path-stamp process names its instance's module path set; as an
    // operand, the processes of one cell definition share one body.
    for index in 0..model.module_paths().len() {
        let name = format!("llg_path_set_{index}");
        registry.insert(
            name.clone(),
            Operand::pointer("llg_module_path_set_t", "module path set".to_owned(), &name),
        );
    }
    // A sequential UDP instance steps its own state record; as an operand,
    // the gates of one primitive definition share one body.
    for state in 0..model.udp_states().len() {
        if let Some(ty) = super::super::owned::udp::state_type(model, state) {
            let name = format!("llg_udp_state_{state}");
            registry.insert(name.clone(), Operand::pointer(&ty, ty.clone(), &name));
        }
    }
    for event in &model.events {
        if !event.is_array() {
            registry.insert(
                event.c_name.clone(),
                Operand::pointer("llg_event_t", "event".to_owned(), &event.c_name),
            );
        }
    }
    for object in &model.objects {
        let ty = match object.ty {
            IrObjectType::String => "llg_string_t",
            IrObjectType::Process => "llg_process_handle_t*",
            _ => "void*",
        };
        registry.insert(
            object.c_name.clone(),
            Operand::pointer(ty, format!("{:?}", object.ty), &object.c_name),
        );
        if object.ty.has_change_marker() {
            let name = format!("{}_llg_dep", object.c_name);
            registry.insert(
                name.clone(),
                Operand::pointer("sv4_t", "dependency".to_owned(), &name),
            );
        }
    }
    for array in model.arrays.iter().filter(|array| array.is_model_storage()) {
        let ty = if array.real { "double" } else { "sv4_t" };
        let shape = format!(
            "{}:{}:{}:{:?}:{}",
            array.elem_width, array.signed, array.two_state, array.dims, array.shortreal
        );
        registry.insert(
            array.c_name.clone(),
            Operand::new(
                if array.sparse() {
                    "llg_fixed_array_t *@".to_owned()
                } else {
                    format!("{ty} (*@)[{}]", array.total)
                },
                shape,
                format!("&{}", array.c_name),
                "(*I->@)",
            ),
        );
        let name = format!("{}_llg_contents_dep", array.c_name);
        registry.insert(
            name.clone(),
            Operand::pointer("sv4_t", "dependency".to_owned(), &name),
        );
    }
    for (index, function) in model.funcs.iter().enumerate() {
        if super::super::owned::model::inline_template(function) {
            continue;
        }
        for local in &function.locals {
            let ty = if local.string {
                "llg_string_t"
            } else if local.real {
                "double"
            } else {
                "sv4_t"
            };
            registry.insert(
                local.c_name().to_owned(),
                Operand::pointer(
                    ty,
                    format!("{}:{}", local.width, local.signed),
                    local.c_name(),
                ),
            );
        }
        if !function.automatic {
            if let Some(ty) = function.ret.filter(|_| function.return_signal.is_none()) {
                let name = format!("_llg_ret_{index}");
                registry.insert(
                    name.clone(),
                    Operand::pointer(
                        if ty.width() == 0 { "double" } else { "sv4_t" },
                        format!("{ty:?}"),
                        &name,
                    ),
                );
            }
            if function.ret_string || function.ret_chandle {
                let name = format!("_llg_native_ret_{index}");
                registry.insert(
                    name.clone(),
                    Operand::pointer(
                        if function.ret_string {
                            "llg_string_t"
                        } else {
                            "void*"
                        },
                        "native return".to_owned(),
                        &name,
                    ),
                );
            }
        }
        let (ret, params) = if functions.contains_key(&index) {
            (
                "llg_co_status_t".to_owned(),
                "llg_co_frame_t*, llg_co_chain_t*".to_owned(),
            )
        } else {
            (
                function_return_type(function).to_owned(),
                owned_func_param_fields(function)
                    .iter()
                    .map(|(ty, _)| ty.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };
        registry.insert(
            function.c_name.clone(),
            Operand::new(
                format!("{ret} (*@)({params})"),
                format!("{:?}:{:?}", function.ret, function.formals),
                function.c_name.clone(),
                "I->@",
            ),
        );
    }
    for artifact in functions.values().chain(branches.values()) {
        registry.insert(
            artifact.desc_name.clone(),
            Operand::pointer(
                "const llg_co_desc_t",
                artifact.frame_type.clone(),
                &artifact.desc_name,
            ),
        );
        let name = artifact.body_name();
        if !registry.contains_key(name) {
            registry.insert(
                name.to_owned(),
                Operand::new(
                    "llg_co_status_t (*@)(llg_co_frame_t*, llg_co_chain_t*)".to_owned(),
                    artifact.frame_type.clone(),
                    name.to_owned(),
                    "I->@",
                ),
            );
        }
    }
    registry
}

/// Whether a body belongs to one iteration of a loop generate construct (or
/// one element of an instance array), whose genvar or index payloads differ
/// per instance. Labels and diagnostic names spell such a scope as written,
/// `tb.g[3].always` (see the lowering's generate-scope collection); only the
/// C identifiers encode the brackets. A false positive only turns literals
/// of one sharing class into record fields.
pub(super) fn generate_iteration(label: &str) -> bool {
    label.contains('[')
}

fn is_port_link(execution: &ExecutionModel, artifact: &CoroutineArtifact) -> bool {
    match artifact.owner {
        CoroutineId::Process(index) => execution
            .processes()
            .get(index)
            .and_then(|process| execution.ir().processes.get(process.semantic_process))
            .is_some_and(|process| process.port_link()),
        _ => false,
    }
}

/// What two bodies of one sharing class agree on besides their normalized
/// text; it is part of the class key.
#[derive(PartialEq, Eq, Hash)]
enum Site<'a> {
    Coroutine(std::mem::Discriminant<CoroutineId>, &'a str),
    Evaluator,
    Plain(&'a crate::sim::semantic::Origin),
}

/// A body that may join a sharing class.
#[derive(Clone, Copy)]
enum Body<'a> {
    Coroutine(&'a CoroutineArtifact),
    Evaluator(&'a str, &'a str),
    Plain(usize, &'a str),
}

impl<'a> Body<'a> {
    fn source(self) -> &'a str {
        match self {
            Body::Coroutine(artifact) => &artifact.source,
            Body::Evaluator(_, source) | Body::Plain(_, source) => source,
        }
    }

    fn site(self, model: &'a IrModel) -> Site<'a> {
        match self {
            Body::Coroutine(artifact) => {
                Site::Coroutine(std::mem::discriminant(&artifact.owner), &artifact.location)
            }
            Body::Evaluator(..) => Site::Evaluator,
            Body::Plain(index, _) => Site::Plain(model.funcs[index].origin()),
        }
    }

    /// Normalization replaces words and one-line literals only, so bodies
    /// whose line counts differ never share a text.
    fn lines(self) -> usize {
        self.source().bytes().filter(|byte| *byte == b'\n').count()
    }
}

/// The bodies, in order, whose `key` is that of at least `min_instances` of
/// them: no other body can be in a class of that size.
fn repeated<'a, K: std::hash::Hash + Eq>(
    bodies: Vec<Body<'a>>,
    min_instances: usize,
    key: impl Fn(Body<'a>) -> K,
) -> Vec<Body<'a>> {
    let mut counts = HashMap::new();
    for body in &bodies {
        *counts.entry(key(*body)).or_insert(0usize) += 1;
    }
    bodies
        .into_iter()
        .filter(|body| counts[&key(*body)] >= min_instances)
        .collect()
}

/// The names that are operands wherever a body spells them.
struct Operands {
    registry: HashMap<String, Operand>,
    /// Packed constants, operands of the bodies whose literals vary.
    constants: HashMap<String, Operand>,
    filter: NameFilter,
}

impl Operands {
    fn new(registry: HashMap<String, Operand>, constants: HashMap<String, Operand>) -> Self {
        let filter = registry
            .keys()
            .chain(constants.keys())
            .map(String::as_str)
            .collect();
        Self {
            registry,
            constants,
            filter,
        }
    }

    /// The operand a body names `word`, and whether it is a constant.
    fn get(&self, word: &str, generated: bool) -> Option<(&Operand, bool)> {
        if !self.filter.may_hold(word) {
            return None;
        }
        match self.registry.get(word) {
            Some(operand) => Some((operand, false)),
            None if generated => Some((self.constants.get(word)?, true)),
            None => None,
        }
    }
}

struct Normalized {
    source: String,
    operands: Vec<Operand>,
}

fn word_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    index
}

fn quoted_end(bytes: &[u8], mut index: usize) -> usize {
    let quote = bytes[index];
    index += 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            byte if byte == quote => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// The entity of a statement counter access `<counters>[<entity>]` whose
/// name ends at `index`, and the index after its `]`.
fn statement_counter_at(bytes: &[u8], index: usize) -> Option<(u32, usize)> {
    let digits = bytes.get(index + 1..)?;
    let length = digits
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if bytes.get(index) != Some(&b'[') || length == 0 || digits.get(length) != Some(&b']') {
        return None;
    }
    let entity = std::str::from_utf8(&digits[..length]).ok()?.parse().ok()?;
    Some((entity, index + 1 + length + 1))
}

/// The lowest statement counter entity `source` names, if any. A body's
/// counter accesses become `counters[base + offset]` with this as the
/// instance-record base: instances whose bodies count the same statements
/// at the same relative entities share one body and one record field.
fn statement_counter_base(source: &str, counters: &str) -> Option<u32> {
    let bytes = source.as_bytes();
    source
        .match_indices(counters)
        .filter(|(at, _)| {
            *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_')
        })
        .filter_map(|(at, _)| statement_counter_at(bytes, at + counters.len()))
        .map(|(entity, _)| entity)
        .min()
}

/// These are compiler-owned operands, never arbitrary C supplied through IR.
fn normalize(
    source: &str,
    name: &str,
    generated: bool,
    named: &Operands,
    scalars: &mut ScalarOperands,
) -> Option<Normalized> {
    crate::work::count(crate::work::Work::NormalizedBytes, source.len());
    if source
        .lines()
        .skip(1)
        .any(|line| line.trim_start().starts_with("static "))
    {
        return None;
    }
    let source = source.replacen(&format!(" {name}("), &format!(" {SELF}("), 1);
    let bytes = source.as_bytes();
    let mut labels = HashMap::new();
    for line in source.lines() {
        if let Some(label) = line.trim_end().strip_suffix(": ;").map(str::trim_start) {
            if label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                let next = labels.len();
                labels
                    .entry(label)
                    .or_insert_with(|| format!("_llg_body_label_{next}"));
            }
        }
    }
    let mut out = String::with_capacity(source.len());
    let mut copied = 0;
    let mut operands = Vec::<Operand>::new();
    let mut pointer_slots = HashMap::<&str, usize>::new();
    let mut index = 0;
    let mut arguments: Vec<(&str, usize)> = Vec::new();
    let mut last_word = "";
    let counters = super::super::owned::coverage::statement_counters();
    let mut counter_base = None;
    let mut counter_slot = None;
    let operand_at = |out: &mut String, slot: usize| {
        let _ = write!(out, "llg_body_operand_{slot}");
    };
    while index < bytes.len() {
        let start = index;
        let mut operand = None;
        if bytes[index..].starts_with(b"/*") {
            index = source[index + 2..]
                .find("*/")
                .map_or(bytes.len(), |n| index + n + 4);
        } else if bytes[index..].starts_with(b"//") {
            index = source[index..]
                .find('\n')
                .map_or(bytes.len(), |n| index + n);
        } else if matches!(bytes[index], b'"' | b'\'') {
            index = quoted_end(bytes, index);
            if bytes[start] == b'"' {
                operand = Some(scalars.operand("const char*", &source[start..index]));
            }
        } else if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            index = word_end(bytes, index);
            let word = &source[start..index];
            last_word = word;
            if word == counters {
                let base =
                    *counter_base.get_or_insert_with(|| statement_counter_base(&source, counters));
                if let (Some(base), Some((entity, end))) =
                    (base, statement_counter_at(bytes, index))
                {
                    // Statement coverage (40.2.2): one base per body, so a
                    // shared record carries one integer, not a counter
                    // pointer per statement.
                    let slot = *counter_slot.get_or_insert_with(|| {
                        operands.push(scalars.operand("uint32_t", &format!("{base}u")));
                        operands.len() - 1
                    });
                    out.push_str(&source[copied..start]);
                    let _ = match entity - base {
                        0 => write!(out, "{word}[llg_body_operand_{slot}]"),
                        // Size arithmetic: the offset folds into the address
                        // displacement (a 32-bit sum would have to wrap).
                        offset => {
                            write!(out, "{word}[(size_t)llg_body_operand_{slot} + {offset}u]")
                        }
                    };
                    index = end;
                    copied = index;
                    continue;
                }
            }
            if let Some(label) = labels.get(word) {
                out.push_str(&source[copied..start]);
                out.push_str(label);
                copied = index;
                continue;
            }
            if let Some((candidate, constant)) = named.get(word, generated) {
                let slot = if constant {
                    // Immutable constant identity does not constrain sharing;
                    // each occurrence can vary independently, like a literal.
                    operands.push(candidate.clone());
                    operands.len() - 1
                } else {
                    *pointer_slots.entry(word).or_insert_with(|| {
                        operands.push(candidate.clone());
                        operands.len() - 1
                    })
                };
                out.push_str(&source[copied..start]);
                operand_at(&mut out, slot);
                copied = index;
                continue;
            }
        } else if bytes[index].is_ascii_digit() {
            index = word_end(bytes, index);
            let value = &source[start..index];
            // A net driver slot (`.drivers[slot]` or a write/inertial slot
            // argument) is the instance's contribution identity: drivers of
            // one net in different generate iterations differ only here. An
            // SDF-annotated gate's delay slot is likewise its own.
            let preceding = &source[..start];
            // The call object a frame takes as its vpiOrigin is the
            // instance's own source call.
            let identity = preceding.ends_with(".drivers[")
                || preceding.trim_end().ends_with("llg_vpi_next_origin =")
                || preceding.ends_with("llg_sdf_gate_delays[")
                || arguments.last().is_some_and(|(call, arg)| match *call {
                    "llg_net_write"
                    | "llg_net_write_selected"
                    | "llg_nba_net_after"
                    | "llg_nba_net_selected_after" => *arg == 1,
                    "llg_inertial_net" | "llg_inertial_selected_net" | "llg_inertial_part_net" => {
                        *arg == 2
                    }
                    "llg_inertial_enable_net" => (2..=3).contains(arg),
                    "llg_activation_enter" => *arg < 2,
                    "llg_disable_target" | "llg_fork_group_new_target" => (1..=2).contains(arg),
                    _ => false,
                });
            if identity {
                operand = Some(scalars.operand(
                    if value.ends_with("ULL") {
                        "uint64_t"
                    } else if value.ends_with('u') {
                        "uint32_t"
                    } else {
                        "int"
                    },
                    value,
                ));
            } else if generated && value.ends_with("ULL") {
                operand = Some(scalars.operand("uint64_t", value));
            } else if generated
                && value.ends_with('u')
                && source[..start].trim_end().ends_with(".lsb =")
            {
                // The low bit of a selected wait dependency, a payload like
                // the selection index the body reads.
                operand = Some(scalars.operand("uint32_t", value));
            } else if generated
                && !value.ends_with('u')
                && source[..start].trim_end().ends_with(['(', ','])
                && source[index..].trim_start().starts_with([',', ')'])
                && arguments.last().is_some_and(|(call, arg)| match *call {
                    "sv4_part_select" => (1..=2).contains(arg),
                    "sv4_part_select_to" => (2..=3).contains(arg),
                    // The bits of a shared net group an instance's
                    // narrow contribution writes.
                    "sv4_select_plan_part" => (1..=2).contains(arg),
                    _ => false,
                })
            {
                // A constant part-select bound that depends on the iteration
                // (`v[i % 28 +: 4]` once its index is folded), a payload like
                // the selection index above. Only a whole `int64_t` argument
                // moves into the record, so no C expression changes type.
                operand = Some(scalars.operand("int64_t", value));
            }
        } else {
            index += 1;
            match bytes[start] {
                b'(' => {
                    arguments.push((std::mem::take(&mut last_word), 0));
                }
                b')' => {
                    arguments.pop();
                }
                b',' => {
                    if let Some((_, arg)) = arguments.last_mut() {
                        *arg += 1;
                    }
                }
                byte if !byte.is_ascii_whitespace() => last_word = "",
                _ => {}
            }
        }
        if let Some(operand) = operand {
            out.push_str(&source[copied..start]);
            operand_at(&mut out, operands.len());
            copied = index;
            operands.push(operand);
        }
    }
    out.push_str(&source[copied..]);
    Some(Normalized {
        source: out,
        operands,
    })
}

pub(super) struct Sharing {
    pub(super) declarations: String,
    pub(super) bodies: String,
    pub(super) prototypes: String,
    pub(super) spawns: SharedSpawns,
    /// Condition evaluators replaced by a shared body: evaluator name to the
    /// shared function and the context (its instance record) to pass it.
    pub(super) evaluators: HashMap<String, (String, String)>,
}

struct Candidate {
    owner: CoroutineId,
    name: String,
    normalized: Normalized,
    key: String,
    plain: Option<usize>,
    /// A module path or timing check condition evaluator.
    evaluator: bool,
}

pub(super) struct AdditionalOperands<'a> {
    pub net_tables: &'a [(String, String, String)],
    /// Hoisted per-site process state `(name, element type, element count)`.
    pub site_statics: &'a [(String, &'static str, Option<u64>)],
    pub constants: &'a super::super::constants::PackedConstants,
    /// Functions with a `void* context` argument, by name: the condition
    /// evaluators (`static void <name>(sv4_t* out, void* context)`) of module
    /// path and timing check sets and the redrive functions
    /// (`static void <name>(void* context)`). Shared ones read their
    /// instance record through `context`.
    pub evaluators: &'a mut BTreeMap<String, String>,
    /// The `evaluators` whose literal payloads may vary per instance: those
    /// of a generate iteration (`generate_iteration`) or a port link.
    pub generated_evaluators: &'a HashSet<String>,
}

fn push_candidate(
    mut candidate: Candidate,
    groups: &mut Vec<Vec<Candidate>>,
    by_key: &mut HashMap<String, usize>,
) {
    candidate.key.push(':');
    for operand in &candidate.normalized.operands {
        candidate.key.push_str(operand.declaration());
        candidate.key.push(':');
        candidate.key.push_str(operand.shape());
        candidate.key.push(';');
    }
    candidate.normalized.operands.shrink_to_fit();
    let key = std::mem::take(&mut candidate.key);
    let next = groups.len();
    let group = *by_key.entry(key).or_insert_with(|| {
        groups.push(Vec::new());
        next
    });
    if !groups[group].is_empty() {
        candidate.normalized.source = String::new();
    }
    groups[group].push(candidate);
}

pub(super) fn share(
    execution: &ExecutionModel,
    functions: &mut CoroutineArtifacts,
    processes: &mut [Option<CoroutineArtifact>],
    branches: &mut BTreeMap<CoroutineId, CoroutineArtifact>,
    plain: &mut BTreeMap<usize, String>,
    min_instances: usize,
    additional: AdditionalOperands<'_>,
) -> Result<Sharing, String> {
    let mut result = Sharing {
        declarations: String::new(),
        bodies: String::new(),
        prototypes: String::new(),
        spawns: SharedSpawns::default(),
        evaluators: HashMap::new(),
    };
    if min_instances == usize::MAX {
        return Ok(result);
    }
    let model = execution.ir();
    let spawn_names = model
        .spawn_list()
        .into_iter()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();
    let coroutines = functions
        .values()
        .chain(processes.iter().flatten())
        .chain(branches.values())
        // Assertion-action roots are spawned outside the model startup tables.
        .filter(|artifact| {
            !matches!(artifact.owner, CoroutineId::Process(_))
                || spawn_names.contains(artifact.body_name())
        })
        .map(Body::Coroutine);
    // Module path and timing check conditions come from specify blocks,
    // which are module items and never inside a generate scope (1800-2009
    // A.1.4 `non_port_module_item`): their payloads are the module's parameters and stay structural.
    // A redrive function may belong to a generate iteration or a port link.
    let evaluators = additional
        .evaluators
        .iter()
        .map(|(name, source)| Body::Evaluator(name, source));
    let plain_bodies = plain
        .iter()
        .filter(|(index, _)| model.funcs[**index].dpi.is_none())
        .map(|(index, source)| Body::Plain(*index, source));
    // Few bodies have a counterpart at all; only theirs are read.
    let bodies = repeated(
        coroutines.chain(evaluators).chain(plain_bodies).collect(),
        min_instances,
        |body| body.site(model),
    );
    let bodies = repeated(bodies, min_instances, |body| {
        (body.site(model), body.lines())
    });
    if bodies.is_empty() {
        return Ok(result);
    }
    let mut registry = registry(model, functions, branches);
    let constants = additional
        .constants
        .operands()
        .into_iter()
        .map(|(name, width, signed)| {
            let operand = Operand::pointer("const sv4_t", format!("{width}:{signed}"), &name);
            (name, operand)
        })
        .collect::<HashMap<_, _>>();
    for (name, ty, shape) in additional.net_tables {
        registry.insert(
            name.clone(),
            Operand::new(
                format!("const {ty}* @"),
                shape.clone(),
                name.clone(),
                "I->@",
            ),
        );
    }
    // Each process's own state: a scalar is addressed (`&name`), an array
    // decays to its first element (`&name[index]`).
    for (name, ty, len) in additional.site_statics {
        let operand = match len {
            None => Operand::pointer(ty, format!("site:{ty}"), name),
            Some(len) => Operand::new(
                format!("{ty}* @"),
                format!("site:{ty}[{len}]"),
                name.clone(),
                "I->@",
            ),
        };
        registry.insert(name.clone(), operand);
    }
    let frame_names = functions
        .values()
        .map(|artifact| {
            (
                format!("{}_frame_t", artifact.body_name()),
                artifact.frame_type.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let operands = Operands::new(registry, constants);
    let mut scalars = ScalarOperands::default();
    let mut groups = Vec::<Vec<Candidate>>::new();
    let mut by_key = HashMap::new();
    for body in bodies {
        let (name, generated) = match body {
            // Literal payloads may vary per instance in generate-scope bodies
            // and in port links, whose selected actual bits are the instance's
            // connection rather than its module's code.
            Body::Coroutine(artifact) => (
                artifact.body_name(),
                generate_iteration(&artifact.display_name) || is_port_link(execution, artifact),
            ),
            Body::Evaluator(name, _) => (name, additional.generated_evaluators.contains(name)),
            Body::Plain(index, _) => {
                let function = &model.funcs[index];
                (
                    function.c_name.as_str(),
                    generate_iteration(function.diagnostic_name()),
                )
            }
        };
        let Some(normalized) = normalize(body.source(), name, generated, &operands, &mut scalars)
        else {
            continue;
        };
        let key = match body {
            Body::Coroutine(artifact) => {
                let shape = artifact.layout.render_typedef("llg_key_frame")?;
                let shape = rewrite_identifiers(&shape, |name| frame_names.get(name));
                format!(
                    "{:?}:{}:{shape}:{}",
                    std::mem::discriminant(&artifact.owner),
                    artifact.location,
                    normalized.source
                )
            }
            Body::Evaluator(..) => format!("evaluator:{}", normalized.source),
            Body::Plain(index, _) => {
                let function = &model.funcs[index];
                format!(
                    "plain:{:?}:{:?}:{:?}:{}",
                    function.origin, function.formals, function.ret, normalized.source
                )
            }
        };
        let (owner, plain, evaluator) = match body {
            Body::Coroutine(artifact) => (artifact.owner, None, false),
            Body::Evaluator(..) => (CoroutineId::Function(usize::MAX), None, true),
            Body::Plain(index, _) => (CoroutineId::Function(index), Some(index), false),
        };
        push_candidate(
            Candidate {
                owner,
                name: name.to_owned(),
                normalized,
                key,
                plain,
                evaluator,
            },
            &mut groups,
            &mut by_key,
        );
    }
    let mut sequence = 0;
    for group in groups
        .into_iter()
        .filter(|group| group.len() >= min_instances)
    {
        let first = &group[0];
        let body_name = format!("llg_shared_body_{sequence}");
        let record_type = format!("llg_body_record_{sequence}_t");
        sequence += 1;
        let mut varying = Vec::new();
        let mut replacements = vec![None; first.normalized.operands.len()];
        for (slot, operand) in first.normalized.operands.iter().enumerate() {
            if group
                .iter()
                .all(|candidate| candidate.normalized.operands[slot].value == operand.value)
            {
                let expression = if operand.access().starts_with("(*") {
                    operand
                        .value
                        .strip_prefix('&')
                        .unwrap_or(&operand.value)
                        .to_owned()
                } else {
                    operand.value.to_string()
                };
                replacements[slot] = Some(expression);
            } else {
                varying.push(slot);
            }
        }
        // Invariant operands retain their direct references, avoiding record loads.
        let mut source = rewrite_identifiers(&first.normalized.source, |name| {
            let slot = name
                .strip_prefix("llg_body_operand_")?
                .parse::<usize>()
                .ok()?;
            Some(replacements[slot].clone().unwrap_or_else(|| {
                first.normalized.operands[slot]
                    .access()
                    .replace('@', &format!("v{slot}"))
            }))
        });
        result.declarations.push_str("typedef struct {\n");
        for &slot in &varying {
            result.declarations.push_str(&format!(
                "    {};\n",
                first.normalized.operands[slot]
                    .declaration()
                    .replace('@', &format!("v{slot}"))
            ));
        }
        if varying.is_empty() {
            result.declarations.push_str("    unsigned char unused;\n");
        }
        result
            .declarations
            .push_str(&format!("}} {record_type};\n"));
        let root = matches!(first.owner, CoroutineId::Process(_));
        let mut shared_frame = None;
        if root {
            let CoroutineId::Process(index) = first.owner else {
                unreachable!()
            };
            let artifact = processes[index].as_ref().unwrap();
            let frame_name = format!("llg_body_frame_{}_t", sequence - 1);
            let mut typedef = artifact.layout.render_typedef(&frame_name)?;
            // Embedded callee types have already been canonicalized in the body.
            typedef = rewrite_identifiers(&typedef, |name| frame_names.get(name));
            typedef = typedef.replacen(
                "    llg_co_frame_t co;\n",
                "    llg_co_frame_t co;\n    const void* _llg_instance;\n",
                1,
            );
            result.declarations.push_str(&typedef);
            result
                .declarations
                .push_str(&format!("LLG_CO_ROOT_FRAME_OK({frame_name});\n"));
            source = rewrite_identifiers(&source, |name| {
                (name == artifact.frame_type).then(|| frame_name.clone())
            });
            let declaration = format!("    {frame_name}* F = ({frame_name}*)co;\n");
            source = source.replacen(&declaration, &format!("{declaration}    const {record_type}* restrict I = F->_llg_instance;\n    (void)I;\n"), 1);
            shared_frame = Some(frame_name);
        } else if first.evaluator {
            // The evaluator signature already takes a context pointer.
            let open = source
                .find(" {\n")
                .ok_or_else(|| "missing condition evaluator signature".to_owned())?;
            source.insert_str(
                open + 3,
                &format!(
                    "    const {record_type}* restrict I = (const {record_type}*)context;\n    (void)I;\n"
                ),
            );
        } else {
            let open = source
                .find(" {\n")
                .ok_or_else(|| "missing owned procedure signature".to_owned())?;
            source.insert_str(
                open - 1,
                &format!(", const {record_type}* _llg_instance_arg"),
            );
            let open = source.find(" {\n").unwrap();
            source.insert_str(
                open + 3,
                &format!(
                    "    const {record_type}* restrict I = _llg_instance_arg;\n    (void)I;\n"
                ),
            );
        }
        source = source
            .replacen("static ", "static LLG_MODEL_SHARED ", 1)
            .replacen(SELF, &body_name, 1);
        result.prototypes.push_str(
            source
                .split_once(" {\n")
                .ok_or_else(|| "missing shared procedure signature".to_owned())?
                .0,
        );
        result.prototypes.push_str(";\n");
        result.bodies.push_str(&source);
        // One record array per class: a member's record is one initializer
        // row, not a named object.
        let records = format!("llg_body_instances_{}", sequence - 1);
        result.declarations.push_str(&format!(
            "static const {record_type} {records}[{}] = {{\n",
            group.len()
        ));
        for candidate in &group {
            let values = varying
                .iter()
                .map(|slot| &*candidate.normalized.operands[*slot].value)
                .collect::<Vec<_>>();
            result.declarations.push_str(&format!(
                "    {{ {} }},\n",
                if values.is_empty() {
                    "0".to_owned()
                } else {
                    values.join(", ")
                }
            ));
        }
        result.declarations.push_str("};\n");
        let mut class = shared_frame.as_ref().map(|frame| SpawnClass {
            body: body_name.clone(),
            records: records.clone(),
            record_type: record_type.clone(),
            frame: frame.clone(),
            string_fields: varying
                .iter()
                .filter(|slot| first.normalized.operands[**slot].declaration() == "const char* @")
                .map(|slot| {
                    (
                        format!("v{slot}"),
                        group
                            .iter()
                            .map(|candidate| candidate.normalized.operands[*slot].value.to_string())
                            .collect(),
                    )
                })
                .collect(),
            members: Vec::with_capacity(group.len()),
            desc: None,
        });
        for (member, candidate) in group.iter().enumerate() {
            let record = format!("{records}[{member}]");
            if candidate.evaluator {
                additional.evaluators.remove(&candidate.name);
                result.evaluators.insert(
                    candidate.name.clone(),
                    (body_name.clone(), format!("&{record}")),
                );
            } else if let Some(index) = candidate.plain {
                let function = &model.funcs[index];
                let params = owned_func_param_fields(function)
                    .iter()
                    .map(|(_, name)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret = function_return_type(function);
                let call = format!("{body_name}({params}, &{record})");
                plain.insert(
                    index,
                    retained(format!(
                        "static {ret} {}({}) {{\n    {}{call};\n}}\n",
                        candidate.name,
                        owned_func_params(function),
                        if ret == "void" { "" } else { "return " }
                    )),
                );
            } else {
                let artifact = match candidate.owner {
                    CoroutineId::Function(index) => functions.get_mut(&index).unwrap(),
                    CoroutineId::Process(index) => processes[index].as_mut().unwrap(),
                    _ => branches.get_mut(&candidate.owner).unwrap(),
                };
                if let Some(frame) = &shared_frame {
                    artifact.frame_type = frame.clone();
                    artifact.shared_entry = Some(body_name.clone());
                    // Release the body: the shared entry replaces it.
                    artifact.source = String::new();
                    if let Some(class) = class.as_mut() {
                        result.spawns.members.insert(
                            candidate.name.clone(),
                            (result.spawns.classes.len(), member),
                        );
                        class.members.push(candidate.owner);
                    }
                } else {
                    artifact.source = retained(format!("static llg_co_status_t {}(llg_co_frame_t* co, llg_co_chain_t* ch) {{\n    return {body_name}(co, ch, &{record});\n}}\n", candidate.name));
                }
            }
        }
        if let Some(class) = class {
            result.spawns.classes.push(class);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_groups_preserve_typed_constant_occurrences() {
        let constants = [
            ("constant_a", "65:false"),
            ("constant_b", "65:false"),
            ("constant_signed", "65:true"),
            ("constant_wide", "128:false"),
        ]
        .into_iter()
        .map(|(name, shape)| {
            (
                name.to_owned(),
                Operand::pointer("const sv4_t", shape.to_owned(), name),
            )
        })
        .collect();
        let named = Operands::new(HashMap::new(), constants);
        let mut scalars = ScalarOperands::default();
        let mut groups = Vec::new();
        let mut by_key = HashMap::new();
        for (index, second) in [
            "constant_a",
            "constant_b",
            "constant_signed",
            "constant_wide",
        ]
        .into_iter()
        .enumerate()
        {
            let name = format!("p_{index}");
            let source = format!("static void {name}(void) {{ sv4_add(constant_a, {second}); }}");
            let ordinary = normalize(&source, &name, false, &named, &mut scalars).unwrap();
            assert!(ordinary.operands.is_empty());
            let normalized = normalize(&source, &name, true, &named, &mut scalars).unwrap();
            assert_eq!(normalized.operands.len(), 2);
            let key = normalized.source.clone();
            push_candidate(
                Candidate {
                    owner: CoroutineId::Process(index),
                    name,
                    normalized,
                    key,
                    plain: None,
                    evaluator: false,
                },
                &mut groups,
                &mut by_key,
            );
        }
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].len(), 2);
        assert!(!groups[0][0].normalized.source.is_empty());
        assert!(groups[0][1].normalized.source.is_empty());
        assert!(groups
            .iter()
            .flatten()
            .all(|candidate| candidate.key.is_empty()));
        assert_eq!(&*groups[0][0].normalized.operands[1].value, "&constant_a");
        assert_eq!(&*groups[0][1].normalized.operands[1].value, "&constant_b");
        assert_eq!(groups[1][0].normalized.operands[1].shape(), "65:true");
        assert_eq!(groups[2][0].normalized.operands[1].shape(), "128:false");
    }

    #[test]
    fn sharing_retains_one_normalized_body_per_exact_group() {
        for count in [32, 512] {
            let mut groups = Vec::new();
            let mut by_key = HashMap::new();
            for index in 0..count {
                push_candidate(
                    Candidate {
                        owner: CoroutineId::Process(index),
                        name: format!("p_{index}"),
                        normalized: Normalized {
                            source: "identical body".to_owned(),
                            operands: vec![Operand::scalar("uint64_t", index.to_string())],
                        },
                        key: "identical key".to_owned(),
                        plain: None,
                        evaluator: false,
                    },
                    &mut groups,
                    &mut by_key,
                );
            }
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].len(), count);
            assert_eq!(
                groups[0]
                    .iter()
                    .map(|candidate| candidate.normalized.source.len())
                    .sum::<usize>(),
                "identical body".len()
            );
            assert!(groups[0].iter().all(|candidate| candidate.key.is_empty()));
            for (index, candidate) in groups[0].iter().enumerate() {
                assert_eq!(candidate.owner, CoroutineId::Process(index));
                assert_eq!(&*candidate.normalized.operands[0].value, index.to_string());
            }
        }
    }

    #[test]
    fn scalar_operands_share_kinds_and_literal_values() {
        let named = Operands::new(HashMap::new(), HashMap::new());
        let mut scalars = ScalarOperands::default();
        let normalized = ["p_0", "p_1"].map(|name| {
            let source =
                format!("static void {name}(void) {{ f(7ULL, \"text\"); g(7ULL, 9ULL); }}");
            normalize(&source, name, true, &named, &mut scalars).unwrap()
        });
        let [first, second] = &normalized;
        assert_eq!(first.source, second.source);
        let values = |normalized: &Normalized| {
            normalized
                .operands
                .iter()
                .map(|operand| (operand.declaration().to_owned(), operand.value.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            values(first),
            [
                ("uint64_t @", "7ULL"),
                ("const char* @", "\"text\""),
                ("uint64_t @", "7ULL"),
                ("uint64_t @", "9ULL"),
            ]
            .map(|(declaration, value)| (declaration.to_owned(), value.to_owned()))
        );
        assert_eq!(values(first), values(second));
        for (left, right) in first.operands.iter().zip(&second.operands) {
            assert!(Rc::ptr_eq(&left.kind, &right.kind));
            assert!(Rc::ptr_eq(&left.value, &right.value));
        }
        assert!(Rc::ptr_eq(
            &first.operands[0].value,
            &first.operands[2].value
        ));
        assert!(Rc::ptr_eq(&first.operands[0].kind, &first.operands[3].kind));
        assert_eq!(first.operands[1].shape(), "const char*");
        assert_eq!(first.operands[1].access(), "I->@");
    }

    #[test]
    fn statement_counters_share_one_base_per_body() {
        let named = Operands::new(HashMap::new(), HashMap::new());
        let mut scalars = ScalarOperands::default();
        let body = |name: &str, entities: [u32; 3]| {
            format!(
                "static void {name}(void) {{\n    ++llg_scov[{}];\n    ++llg_scov[{}];\n    if (x) ++llg_scov[{}];\n    ++xllg_scov[3];\n}}\n",
                entities[0], entities[1], entities[2]
            )
        };
        let mut normalize_body = |name: &str, entities| {
            normalize(&body(name, entities), name, false, &named, &mut scalars).unwrap()
        };
        // Bodies of two instances: the same statements at a different base.
        let first = normalize_body("p_0", [12, 10, 13]);
        let second = normalize_body("p_1", [22, 20, 23]);
        assert_eq!(first.source, second.source);
        assert!(first
            .source
            .contains("++llg_scov[(size_t)llg_body_operand_0 + 2u];"));
        assert!(first.source.contains("++llg_scov[llg_body_operand_0];"));
        assert!(first
            .source
            .contains("++llg_scov[(size_t)llg_body_operand_0 + 3u];"));
        // Other words are left alone.
        assert!(first.source.contains("++xllg_scov[3];"));
        let values = |normalized: &Normalized| {
            normalized
                .operands
                .iter()
                .map(|operand| (operand.declaration().to_owned(), operand.value.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            values(&first),
            [("uint32_t @".to_owned(), "10u".to_owned())]
        );
        assert_eq!(
            values(&second),
            [("uint32_t @".to_owned(), "20u".to_owned())]
        );
        // Different relative entities never share a body.
        let third = normalize_body("p_2", [31, 30, 34]);
        assert_ne!(first.source, third.source);
        assert_eq!(statement_counter_base("++llg_scov[];", "llg_scov"), None);
        assert_eq!(statement_counter_at(b"llg_scov[7]", 8), Some((7, 11)));
        assert_eq!(statement_counter_at(b"llg_scov[7", 8), None);
        assert_eq!(statement_counter_at(b"llg_scov[x]", 8), None);
    }
}
