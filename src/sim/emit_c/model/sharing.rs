//! Exact sharing of owned emitted bodies; unrepresented operands stay in the key.
use super::*;
use crate::sim::ir::*;
use std::collections::HashSet;
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
        if matches!(object.ty, IrObjectType::String | IrObjectType::Chandle) {
            let name = format!("{}_llg_dep", object.c_name);
            registry.insert(
                name.clone(),
                Operand::pointer("sv4_t", "dependency".to_owned(), &name),
            );
        }
    }
    for array in model.arrays.iter().filter(|array| !array.activation) {
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
        let name = artifact.desc_name.strip_suffix("_desc").unwrap();
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

/// These are compiler-owned operands, never arbitrary C supplied through IR.
fn normalize(
    source: &str,
    name: &str,
    generated: bool,
    registry: &HashMap<String, Operand>,
    constants: &HashMap<String, Operand>,
    scalars: &mut ScalarOperands,
) -> Option<Normalized> {
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
        if let Some(label) = line.trim().strip_suffix(": ;") {
            if label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                let next = labels.len();
                labels
                    .entry(label.to_owned())
                    .or_insert_with(|| format!("_llg_body_label_{next}"));
            }
        }
    }
    let mut out = String::with_capacity(source.len());
    let mut operands = Vec::<Operand>::new();
    let mut pointer_slots = HashMap::<String, usize>::new();
    let mut index = 0;
    let mut arguments: Vec<(String, usize)> = Vec::new();
    let mut last_word = String::new();
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
            last_word = word.to_owned();
            if let Some(label) = labels.get(word) {
                out.push_str(label);
                continue;
            }
            if let Some(candidate) = registry
                .get(word)
                .or_else(|| generated.then(|| constants.get(word)).flatten())
            {
                let slot = if generated && !registry.contains_key(word) {
                    // Immutable constant identity does not constrain sharing;
                    // each occurrence can vary independently, like a literal.
                    let slot = operands.len();
                    operands.push(candidate.clone());
                    slot
                } else if let Some(slot) = pointer_slots.get(word) {
                    *slot
                } else {
                    let slot = operands.len();
                    operands.push(candidate.clone());
                    pointer_slots.insert(word.to_owned(), slot);
                    slot
                };
                out.push_str(&format!("llg_body_operand_{slot}"));
                continue;
            }
        } else if bytes[index].is_ascii_digit() {
            index = word_end(bytes, index);
            let value = &source[start..index];
            let identity = arguments
                .last()
                .is_some_and(|(call, arg)| match call.as_str() {
                    "llg_net_write"
                    | "llg_net_write_selected"
                    | "llg_nba_net_after"
                    | "llg_nba_net_selected_after" => *arg == 1,
                    "llg_activation_enter" => *arg < 2,
                    "llg_disable_target" | "llg_fork_group_new_target" => (1..=2).contains(arg),
                    "llg_pca_assign" | "llg_pca_assign_d" | "llg_pca_drive" | "llg_pca_drive_d" => {
                        *arg == 2
                    }
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
                byte if !byte.is_ascii_whitespace() => last_word.clear(),
                _ => {}
            }
        }
        if let Some(operand) = operand {
            let slot = operands.len();
            out.push_str(&format!("llg_body_operand_{slot}"));
            operands.push(operand);
        } else {
            out.push_str(&source[start..index]);
        }
    }
    Some(Normalized {
        source: out,
        operands,
    })
}

pub(super) struct Sharing {
    pub(super) declarations: String,
    pub(super) bodies: String,
    pub(super) prototypes: String,
    pub(super) spawns: HashMap<String, (String, String)>,
}

struct Candidate {
    owner: CoroutineId,
    name: String,
    normalized: Normalized,
    key: String,
    plain: Option<usize>,
}

pub(super) struct AdditionalOperands<'a> {
    pub pca_tables: &'a [(String, String, String)],
    pub constants: &'a super::super::constants::PackedConstants,
}

fn push_candidate(
    mut candidate: Candidate,
    groups: &mut Vec<Vec<Candidate>>,
    by_key: &mut HashMap<String, usize>,
) {
    let shapes = candidate
        .normalized
        .operands
        .iter()
        .map(|operand| format!("{}:{}", operand.declaration(), operand.shape()))
        .collect::<Vec<_>>()
        .join(";");
    candidate.key.push(':');
    candidate.key.push_str(&shapes);
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
    if min_instances == usize::MAX {
        return Ok(Sharing {
            declarations: String::new(),
            bodies: String::new(),
            prototypes: String::new(),
            spawns: HashMap::new(),
        });
    }
    let model = execution.ir();
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
    for (name, ty, shape) in additional.pca_tables {
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
    let frame_names = functions
        .values()
        .map(|artifact| {
            (
                format!(
                    "{}_frame_t",
                    artifact.desc_name.strip_suffix("_desc").unwrap()
                ),
                artifact.frame_type.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let spawn_names = model
        .spawn_list()
        .into_iter()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();
    let mut scalars = ScalarOperands::default();
    let mut groups = Vec::<Vec<Candidate>>::new();
    let mut by_key = HashMap::new();
    for artifact in functions
        .values()
        .chain(processes.iter().flatten())
        .chain(branches.values())
    {
        let name = artifact.desc_name.strip_suffix("_desc").unwrap();
        // Assertion-action roots are spawned outside the model startup tables.
        if matches!(artifact.owner, CoroutineId::Process(_)) && !spawn_names.contains(name) {
            continue;
        }
        let generated = artifact.display_name.contains('[');
        if let Some(normalized) = normalize(
            &artifact.source,
            name,
            generated,
            &registry,
            &constants,
            &mut scalars,
        ) {
            let shape = artifact.layout.render_typedef("llg_key_frame")?;
            let shape = rewrite_identifiers(&shape, |name| frame_names.get(name).cloned());
            let provenance = if artifact.pca_driver {
                "owned PCA driver"
            } else {
                &artifact.location
            };
            let key = format!(
                "{:?}:{provenance}:{shape}:{}",
                std::mem::discriminant(&artifact.owner),
                normalized.source
            );
            push_candidate(
                Candidate {
                    owner: artifact.owner,
                    name: name.to_owned(),
                    normalized,
                    key,
                    plain: None,
                },
                &mut groups,
                &mut by_key,
            );
        }
    }
    for (index, source) in plain.iter() {
        let function = &model.funcs[*index];
        if function.dpi.is_some() {
            continue;
        }
        if let Some(normalized) = normalize(
            source,
            &function.c_name,
            false,
            &registry,
            &constants,
            &mut scalars,
        ) {
            let key = format!(
                "plain:{:?}:{:?}:{:?}:{}",
                function.origin, function.formals, function.ret, normalized.source
            );
            push_candidate(
                Candidate {
                    owner: CoroutineId::Function(*index),
                    name: function.c_name.clone(),
                    normalized,
                    key,
                    plain: Some(*index),
                },
                &mut groups,
                &mut by_key,
            );
        }
    }
    let mut result = Sharing {
        declarations: String::new(),
        bodies: String::new(),
        prototypes: String::new(),
        spawns: HashMap::new(),
    };
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
        let mut replacements = HashMap::new();
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
                replacements.insert(format!("v{slot}"), expression);
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
            let field = format!("v{slot}");
            Some(replacements.get(&field).cloned().unwrap_or_else(|| {
                first.normalized.operands[slot]
                    .access()
                    .replace('@', &field)
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
            typedef = rewrite_identifiers(&typedef, |name| frame_names.get(name).cloned());
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
        for (member, candidate) in group.iter().enumerate() {
            let record = format!("llg_body_instance_{}_{}", sequence - 1, member);
            let values = varying
                .iter()
                .map(|slot| &*candidate.normalized.operands[*slot].value)
                .collect::<Vec<_>>();
            result.declarations.push_str(&format!(
                "static const {record_type} {record} = {{ {} }};\n",
                if values.is_empty() {
                    "0".to_owned()
                } else {
                    values.join(", ")
                }
            ));
            if let Some(index) = candidate.plain {
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
                    result.spawns.insert(
                        candidate.name.clone(),
                        (
                            format!("&{record}"),
                            format!("offsetof({frame}, _llg_instance)"),
                        ),
                    );
                } else {
                    artifact.source = retained(format!("static llg_co_status_t {}(llg_co_frame_t* co, llg_co_chain_t* ch) {{\n    return {body_name}(co, ch, &{record});\n}}\n", candidate.name));
                }
            }
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
        let registry = HashMap::new();
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
            let ordinary =
                normalize(&source, &name, false, &registry, &constants, &mut scalars).unwrap();
            assert!(ordinary.operands.is_empty());
            let normalized =
                normalize(&source, &name, true, &registry, &constants, &mut scalars).unwrap();
            assert_eq!(normalized.operands.len(), 2);
            let key = normalized.source.clone();
            push_candidate(
                Candidate {
                    owner: CoroutineId::Process(index),
                    name,
                    normalized,
                    key,
                    plain: None,
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
        let registry = HashMap::new();
        let constants = HashMap::new();
        let mut scalars = ScalarOperands::default();
        let normalized = ["p_0", "p_1"].map(|name| {
            let source =
                format!("static void {name}(void) {{ f(7ULL, \"text\"); g(7ULL, 9ULL); }}");
            normalize(&source, name, true, &registry, &constants, &mut scalars).unwrap()
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
}
