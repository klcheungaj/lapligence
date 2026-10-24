//! `$sdf_annotate` (IEEE 1800-2009 32.9, IEEE 1364-2001 16.2) and `--sdf`.
//!
//! Every call's file is read, resolved against the design and valued when
//! the model is generated, before module paths are lowered, so lowering can
//! give the targets what annotation needs (a writable delay for each
//! annotated gate). After the timing checks are lowered the valued
//! operations are mapped onto model rows ([`IrSdfAnnotation`]); the call
//! applies them when it executes and `--sdf` files apply before simulation
//! starts. A file is applied as a whole or not at all: a file that cannot
//! be read or resolved, or a call with an invalid argument, annotates
//! nothing and is reported as a warning (decision A12-D5).

use super::*;
use crate::sim::ir::{
    IrModulePath, IrModulePathDestination, IrModulePathSet, IrPathPolarity, IrSdfAnnotation,
    IrSdfLoad, IrSdfOp, IrSdfTarget, PATH_TRANSITIONS, SDF_INCREMENT, SDF_MIPD_SHIFT, SDF_PERCENT,
};
use crate::sim::sdf::{
    apply, parse_config, parse_scale_factors, resolve, SdfAnnotation, SdfAppliedTarget,
    SdfCallOptions, SdfLimitSlot, SdfLimits, SdfMipdPolicy, SdfMtm, SdfPortTarget, SdfScaleType,
    SdfTargets, SdfTimebase, SdfTimingCheckKind, SdfWarning,
};

/// Warnings a file reports on standard error before they are summarized;
/// the log file has them all.
const SHOWN_FILE_WARNINGS: usize = 100;

/// One `--sdf` request of the command line: `[min:|typ:|max:][scope=]file`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SdfCommand {
    pub file: String,
    /// Hierarchical name of the annotation scope; `None` for the top
    /// instance (an error when the design has several).
    pub scope: Option<String>,
    /// `MINIMUM`, `TYPICAL` or `MAXIMUM`; `None` for TOOL_CONTROL.
    pub mtm: Option<String>,
}

impl SdfCommand {
    /// Parse the `--sdf` operand.
    ///
    /// The text before the first `:` is a selector only when it is a word of
    /// two or more letters, which must then be min, typ or max (so a typo
    /// such as `mx:` is an error). Anything else leaves the operand whole, so
    /// a Windows drive letter (`D:\dir\f.sdf`, `D:/f.sdf`, `D:f.sdf`), an
    /// extended (`\\?\D:\...`) or UNC (`\\server\share\f.sdf`) path and a
    /// `scope=D:\...` operand are read as written on every host.
    pub fn parse(text: &str) -> Result<Self, String> {
        let selector = text.split_once(':').filter(|(member, _)| {
            member.len() >= 2 && member.bytes().all(|b| b.is_ascii_alphabetic())
        });
        let (mtm, rest) = match selector {
            Some((member, rest)) => {
                let mtm = match member.to_ascii_lowercase().as_str() {
                    "min" => "MINIMUM",
                    "typ" => "TYPICAL",
                    "max" => "MAXIMUM",
                    _ => {
                        return Err(format!(
                            "`--sdf {text}`: the member before `:` must be min, typ or max"
                        ))
                    }
                };
                (Some(mtm.to_owned()), rest)
            }
            None => (None, text),
        };
        // A path that starts with a drive letter or `\\` names no scope, even
        // when a later directory name holds `=`.
        let (scope, file) = match rest.split_once('=') {
            Some((scope, file)) if !starts_with_drive_or_unc(rest) => {
                (Some(scope.to_owned()), file)
            }
            _ => (None, rest),
        };
        if file.is_empty() || scope.as_deref() == Some("") {
            return Err(format!(
                "`--sdf {text}` needs [min:|typ:|max:][scope=]file with a file name"
            ));
        }
        Ok(Self {
            file: file.to_owned(),
            scope,
            mtm,
        })
    }
}

/// `X:` (a drive letter) or `\\` (UNC or extended-length) at the start of
/// `text`: Windows path shapes no hierarchical scope name takes.
fn starts_with_drive_or_unc(text: &str) -> bool {
    let bytes = text.as_bytes();
    (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        || text.starts_with("\\\\")
}

/// An annotation request: a `$sdf_annotate` call or a `--sdf` file, with
/// the arguments it gives.
#[derive(Clone, PartialEq, Eq, Hash)]
struct SdfRequest {
    file: String,
    scope: NodeId,
    config: Option<String>,
    log: Option<String>,
    mtm: Option<String>,
    scale: Option<String>,
    scale_type: Option<String>,
}

/// The annotations of a design, what lowering gives their targets, and the
/// calls that apply them.
#[derive(Default)]
pub(in super::super) struct SdfState {
    /// Each call: the annotation it applies, or `None` when it annotates
    /// nothing.
    calls: HashMap<NodeId, Option<u32>>,
    /// The valued file of each annotation and its INTERCONNECT_MIPD
    /// policy, by index.
    annotations: Vec<(String, SdfAnnotation, SdfMipdPolicy)>,
    /// Annotations applied before simulation starts (`--sdf`).
    startup: Vec<u32>,
    /// Requests already read: their annotation, or `None`.
    read: HashMap<SdfRequest, Option<u32>>,
    /// The design node of each catalog scope.
    nodes: Vec<NodeId>,
    /// The writable delay slot of each gate a DEVICE entry annotates.
    gates: HashMap<NodeId, u32>,
    /// Instances whose `$setuphold`/`$recrem` limits an annotation rewrites
    /// while negative timing checks are enabled.
    pairs: HashSet<NodeId>,
    /// Instances with an annotated timing check limit.
    checks: HashSet<NodeId>,
    gate_order: Vec<NodeId>,
    /// Interconnect loads: each annotated load port bit (instance, port
    /// index, bit offset in the port), its distinct sources (entries from 1)
    /// and, once lowered, its interconnect path.
    loads: HashMap<(NodeId, u32, u32), u32>,
    load_order: Vec<SdfLoadState>,
    /// Log files and their contents, in the order first written.
    logs: Vec<(String, String)>,
}

/// The load port bits of one net group: the port net's signal, each load
/// and its bit in the net, and the port's name.
struct LoadGroup {
    group: usize,
    signal: usize,
    bits: Vec<(usize, u32)>,
    name: String,
}

/// One interconnect load port bit.
struct SdfLoadState {
    instance: NodeId,
    port: u32,
    bit: u32,
    sources: HashMap<SdfPortTarget, u32>,
    /// The load's `IrModel::sdf_loads` index once its path exists.
    load: Option<u32>,
}

impl Codegen<'_> {
    /// Read the files of every `$sdf_annotate` call and `--sdf` request.
    /// Runs after the time scales are collected and before net groups and
    /// module paths. A design without either costs one scan of its nodes.
    pub(in super::super) fn read_sdf_annotations(
        &mut self,
        tops: &[NodeId],
        commands: &[SdfCommand],
    ) -> Result<(), String> {
        let calls = self
            .db
            .node_ids()
            .filter(|id| matches!(self.kind(*id), NodeKind::SysCall { name } if name == "$sdf_annotate"))
            .collect::<Vec<_>>();
        if calls.is_empty() && commands.is_empty() {
            return Ok(());
        }
        let (catalog, nodes) = self.build_sdf_catalog(tops);
        let scope_of = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (*node, index as u32))
            .collect::<HashMap<_, _>>();
        let mut state = SdfState {
            nodes,
            ..SdfState::default()
        };
        for command in commands {
            let request = self.sdf_command_request(tops, command)?;
            let origin = format!("--sdf {}", command.file);
            if let Some(annotation) =
                self.read_sdf_request(&mut state, &catalog, &scope_of, &origin, request)
            {
                state.startup.push(annotation);
            }
        }
        for call in calls {
            let origin = self.source_location(call);
            let mut annotation = match self.sdf_call_request(call)? {
                Ok(request) => {
                    self.read_sdf_request(&mut state, &catalog, &scope_of, &origin, request)
                }
                Err(message) => {
                    self.warnings.push(format!(
                        "$sdf_annotate at {origin}: {message}; the call annotates nothing"
                    ));
                    None
                }
            };
            // A call that leads its initial procedure applies before
            // simulation starts, like `--sdf` (decision A12-D1).
            if let Some(index) = annotation.filter(|_| self.sdf_call_leads(call)) {
                state.startup.push(index);
                annotation = None;
            }
            state.calls.insert(call, annotation);
        }
        self.sdf = Some(Box::new(state));
        Ok(())
    }

    /// Whether `call` runs at time 0 before its process can be suspended:
    /// it is in an `initial` procedure of a module (not a program), reached
    /// through `begin`-`end` blocks only, after declarations, assignments
    /// without timing controls and system tasks that do not end the
    /// simulation. Nothing can observe whether such a call ran before or
    /// after the other time-0 activity (IEEE 1800-2009 4.7), so applying it
    /// before simulation starts gives the time-0 transitions its delays
    /// whatever order the processes start in.
    fn sdf_call_leads(&self, call: NodeId) -> bool {
        let mut node = call;
        loop {
            let Some(parent) = self.node(node).parent else {
                return false;
            };
            match self.kind(parent) {
                NodeKind::Process {
                    kind: ProcessKind::Initial,
                } => {
                    return self
                        .owning_inst(parent)
                        .is_some_and(|instance| !self.db.is_program_instance(instance));
                }
                NodeKind::Stmt(StmtKind::Begin) => {
                    let earlier = self
                        .node(parent)
                        .children
                        .iter()
                        .take_while(|child| **child != node);
                    for statement in earlier {
                        let runs_through = match self.kind(*statement) {
                            NodeKind::Var { .. } | NodeKind::Stmt(StmtKind::Empty) => true,
                            NodeKind::Stmt(StmtKind::Assign { delay, .. }) => delay.is_none(),
                            NodeKind::SysCall { name } => {
                                !matches!(name.as_str(), "$finish" | "$stop" | "$exit" | "$fatal")
                            }
                            _ => false,
                        };
                        if !runs_through {
                            return false;
                        }
                    }
                    node = parent;
                }
                _ => return false,
            }
        }
    }

    /// The constant text of a string argument: a literal, a parameter, or a
    /// conversion of one.
    fn sdf_constant_text(&self, node: NodeId) -> Option<String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { value, .. }) => {
                decoded_string_bytes(value).ok().and_then(|bytes| {
                    let start = bytes
                        .iter()
                        .position(|byte| *byte != 0)
                        .unwrap_or(bytes.len());
                    String::from_utf8(bytes[start..].to_vec()).ok()
                })
            }
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.sdf_constant_text(*operand),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self
                .param_vals
                .get(target)
                .and_then(|value| value.to_string_val()),
            _ => None,
        }
    }

    /// The request of one call, or the reason it annotates nothing. A file
    /// name or option that is not constant is an error: llg reads the files
    /// when the model is generated.
    fn sdf_call_request(&self, call: NodeId) -> Result<Result<SdfRequest, String>, String> {
        let args = self.node(call).children.clone();
        let origin = self.source_location(call);
        let empty = |arg: &NodeId| self.db.semantic_detail(*arg) == Some("EmptyArgument");
        let text = |index: usize, what: &str| -> Result<Option<String>, String> {
            match args.get(index) {
                None => Ok(None),
                Some(arg) if empty(arg) => Ok(None),
                Some(arg) => self.sdf_constant_text(*arg).map(Some).ok_or_else(|| {
                    format!(
                        "$sdf_annotate at {origin}: the {what} must be a constant string (a \
                         literal or a parameter); llg reads SDF files when the model is \
                         generated"
                    )
                }),
            }
        };
        let Some(file) = text(0, "SDF file name")? else {
            return Err(format!("$sdf_annotate at {origin} needs an SDF file name"));
        };
        let scope = match args.get(1) {
            Some(arg) if !empty(arg) => match self.kind(*arg) {
                NodeKind::Expr(ExprKind::ScopeRef { target }) => *target,
                _ => {
                    return Err(format!(
                        "$sdf_annotate at {origin}: the second argument must name a module \
                         instance"
                    ))
                }
            },
            _ => match self.owning_inst(call) {
                Some(instance) => instance,
                None => {
                    return Ok(Err(
                        "a call outside a module instance needs a module_instance argument".into(),
                    ))
                }
            },
        };
        Ok(Ok(SdfRequest {
            file,
            scope,
            config: text(2, "configuration file name")?,
            log: text(3, "log file name")?,
            mtm: text(4, "mtm_spec")?,
            scale: text(5, "scale_factors")?,
            scale_type: text(6, "scale_type")?,
        }))
    }

    fn sdf_command_request(
        &self,
        tops: &[NodeId],
        command: &SdfCommand,
    ) -> Result<SdfRequest, String> {
        let scope = match &command.scope {
            None => match tops {
                [top] => *top,
                _ => {
                    return Err(format!(
                        "`--sdf {}` needs a scope (scope=file): the design has {} top \
                         instances",
                        command.file,
                        tops.len()
                    ))
                }
            },
            Some(name) => self.sdf_scope_named(tops, name).ok_or_else(|| {
                format!(
                    "`--sdf`: the design has no module instance `{name}` to annotate `{}`",
                    command.file
                )
            })?,
        };
        Ok(SdfRequest {
            file: command.file.clone(),
            scope,
            config: None,
            log: None,
            mtm: command.mtm.clone(),
            scale: None,
            scale_type: None,
        })
    }

    /// The instance a hierarchical name (`tb.u1`, `tb.g[2].u`) names.
    fn sdf_scope_named(&self, tops: &[NodeId], name: &str) -> Option<NodeId> {
        let (targets, nodes) = self.build_sdf_catalog(tops);
        targets.find(name).map(|index| nodes[index as usize])
    }

    /// Read, resolve and value one request; warnings and the log file
    /// describe what it did. `None` when it annotates nothing.
    fn read_sdf_request(
        &mut self,
        state: &mut SdfState,
        catalog: &SdfTargets,
        scope_of: &HashMap<NodeId, u32>,
        origin: &str,
        request: SdfRequest,
    ) -> Option<u32> {
        if let Some(known) = state.read.get(&request) {
            return *known;
        }
        let mut log = String::new();
        let result = self.value_sdf_request(state, catalog, scope_of, &request, &mut log);
        let annotation = match result {
            Ok(annotation) => Some(annotation),
            Err(message) => {
                log.push_str(&format!(
                    "$sdf_annotate(\"{}\"): {message}; nothing is annotated\n",
                    request.file
                ));
                self.warnings.push(format!(
                    "$sdf_annotate at {origin}: {message}; the call annotates nothing"
                ));
                None
            }
        };
        if let Some(path) = &request.log {
            match state.logs.iter_mut().find(|(known, _)| known == path) {
                Some((_, text)) => text.push_str(&log),
                None => state.logs.push((path.clone(), log)),
            }
        }
        state.read.insert(request, annotation);
        annotation
    }

    fn value_sdf_request(
        &mut self,
        state: &mut SdfState,
        catalog: &SdfTargets,
        scope_of: &HashMap<NodeId, u32>,
        request: &SdfRequest,
        log: &mut String,
    ) -> Result<u32, String> {
        let Some(&scope) = scope_of.get(&request.scope) else {
            return Err(format!(
                "`{}` is not a module instance that can be annotated",
                self.node(request.scope).name
            ));
        };
        let mut call_options = SdfCallOptions {
            tool_member: self.db.delay_selection().operand(),
            ..SdfCallOptions::default()
        };
        let options = &mut call_options;
        if let Some(path) = &request.config {
            let text = std::fs::read_to_string(path)
                .map_err(|error| format!("cannot read the configuration file `{path}`: {error}"))?;
            let config = parse_config(&text)
                .map_err(|error| format!("configuration file `{path}`:{error}"))?;
            options.mtm = config.mtm.unwrap_or(options.mtm);
            options.scale = config.scale.unwrap_or(options.scale);
            options.scale_type = config.scale_type.unwrap_or(options.scale_type);
            options.mipd = config.mipd.unwrap_or(options.mipd);
            options.turnoff = config.turnoff.unwrap_or(options.turnoff);
        }
        if let Some(mtm) = &request.mtm {
            options.mtm = SdfMtm::parse(mtm).ok_or_else(|| {
                format!(
                    "the mtm_spec `{mtm}` is not MAXIMUM, MINIMUM, TYPICAL or TOOL_CONTROL \
                     (IEEE 1800-2009 Table 32-5)"
                )
            })?;
        }
        if let Some(scale) = &request.scale {
            options.scale = parse_scale_factors(scale).ok_or_else(|| {
                format!(
                    "the scale_factors `{scale}` are not three non-negative numbers \
                     `min:typ:max` (IEEE 1800-2009 32.9)"
                )
            })?;
        }
        if let Some(scale_type) = &request.scale_type {
            options.scale_type = SdfScaleType::parse(scale_type).ok_or_else(|| {
                format!(
                    "the scale_type `{scale_type}` is not FROM_MINIMUM, FROM_TYPICAL, \
                     FROM_MAXIMUM or FROM_MTM (IEEE 1800-2009 Table 32-6)"
                )
            })?;
        }
        let scale = options
            .scale
            .iter()
            .map(|factor| factor.to_string())
            .collect::<Vec<_>>()
            .join(":");
        let header = format!(
            "$sdf_annotate(\"{}\") into {}: mtm_spec {}, scale_factors {scale}, scale_type {}, \
             INTERCONNECT_MIPD {}, TURNOFF_DELAY {}\n",
            request.file,
            catalog.path_of(scope),
            options.mtm.keyword(),
            options.scale_type.keyword(),
            options.mipd.keyword(),
            options.turnoff.keyword(),
        );
        let file = std::fs::File::open(&request.file)
            .map_err(|error| format!("cannot open the SDF file `{}`: {error}", request.file))?;
        let plan = resolve(
            std::io::BufReader::new(file),
            catalog,
            scope,
            SdfLimits::default(),
        )
        .map_err(|error| format!("{}:{error}", request.file))?;
        let annotation = apply(
            &plan,
            catalog,
            &call_options,
            SdfTimebase {
                design_precision_fs: self.design_precision_fs,
            },
        );
        log.push_str(&header);
        log.push_str(&plan.render(catalog));
        let warnings = plan
            .warnings
            .iter()
            .chain(&annotation.warnings)
            .collect::<Vec<&SdfWarning>>();
        for warning in annotation.warnings.iter() {
            log.push_str(&format!("warning {}: {}\n", warning.pos, warning.message));
        }
        for warning in warnings.iter().take(SHOWN_FILE_WARNINGS) {
            self.warnings.push(format!(
                "{}:{}: {}",
                request.file, warning.pos, warning.message
            ));
        }
        if warnings.len() > SHOWN_FILE_WARNINGS {
            self.warnings.push(format!(
                "{}: {} more warnings{}",
                request.file,
                warnings.len() - SHOWN_FILE_WARNINGS,
                if request.log.is_some() {
                    " (the log file has them all)"
                } else {
                    ""
                }
            ));
        }
        let options = self.model.timing_check_options;
        let options_mipd = call_options.mipd;
        for op in &annotation.ops {
            match op.target {
                SdfAppliedTarget::Primitive { scope } => {
                    let gate = state.nodes[scope as usize];
                    if !state.gates.contains_key(&gate) {
                        state.gates.insert(gate, state.gate_order.len() as u32);
                        state.gate_order.push(gate);
                    }
                }
                SdfAppliedTarget::Port { load, source } => {
                    let instance = state.nodes[load.scope as usize];
                    for bit in sdf_port_bits(catalog, &load) {
                        let key = (instance, load.port, bit);
                        let index = *state.loads.entry(key).or_insert_with(|| {
                            state.load_order.push(SdfLoadState {
                                instance,
                                port: load.port,
                                bit,
                                sources: HashMap::new(),
                                load: None,
                            });
                            (state.load_order.len() - 1) as u32
                        });
                        if let Some(source) = source {
                            let sources = &mut state.load_order[index as usize].sources;
                            let next = sources.len() as u32 + 1;
                            sources.entry(source).or_insert(next);
                        }
                    }
                }
                SdfAppliedTarget::Check { scope, .. } => {
                    state.checks.insert(state.nodes[scope as usize]);
                    self.sdf_note_pair(state, catalog, op, options);
                }
                SdfAppliedTarget::Path { .. } => {}
            }
        }
        let index = state.annotations.len() as u32;
        state
            .annotations
            .push((request.file.clone(), annotation, options_mipd));
        Ok(index)
    }

    /// Record the instance of an annotated `$setuphold`/`$recrem` limit when
    /// negative timing checks are enabled.
    fn sdf_note_pair(
        &self,
        state: &mut SdfState,
        catalog: &SdfTargets,
        op: &crate::sim::sdf::SdfAppliedOp,
        options: crate::sim::ir::IrTimingCheckOptions,
    ) {
        if let SdfAppliedTarget::Check { scope, check, .. } = op.target {
            if options.checks
                && options.negative
                && matches!(
                    catalog.scope(scope).checks[check as usize].kind,
                    SdfTimingCheckKind::SetupHold | SdfTimingCheckKind::RecRem
                )
            {
                state.pairs.insert(state.nodes[scope as usize]);
            }
        }
    }

    /// Give every interconnect load port bit a path stage on the port's own
    /// net group: an interconnect path whose delays start at 0 (32.4.4,
    /// 32.4.5). Runs after the module paths are lowered; a load whose net
    /// cannot take the stage is reported and not annotated.
    pub(in super::super) fn lower_sdf_loads(&mut self) -> Result<(), String> {
        let Some(mut state) = self.sdf.take() else {
            return Ok(());
        };
        let result = self.lower_sdf_loads_in(&mut state);
        self.sdf = Some(state);
        result
    }

    fn lower_sdf_loads_in(&mut self, state: &mut SdfState) -> Result<(), String> {
        if state.load_order.is_empty() {
            return Ok(());
        }
        let taken = self
            .model
            .module_paths
            .iter()
            .flat_map(|set| set.destinations())
            .map(|destination| destination.group())
            .collect::<HashSet<_>>();
        // Loads by net group, in first-annotated order. A flat netlist can
        // annotate loads on many groups, so groups and reported warnings are
        // found through sets rather than scans.
        let mut groups: Vec<LoadGroup> = Vec::new();
        let mut group_index: HashMap<usize, usize> = HashMap::new();
        let mut reported: HashSet<String> = self.warnings.iter().cloned().collect();
        for (index, load) in state.load_order.iter().enumerate() {
            let port = self
                .node(load.instance)
                .children
                .iter()
                .copied()
                .filter(|child| matches!(self.kind(*child), NodeKind::Port { .. }))
                .nth(load.port as usize);
            let Some(port) = port else { continue };
            let name = format!(
                "{}.{}",
                self.source_path(&self.instance_path_of(load.instance)),
                self.node(port).name
            );
            let NodeKind::Port { low, .. } = self.kind(port) else {
                continue;
            };
            let terminal = self
                .port_child_target(port, *low)
                .ok_or_else(|| "the port has no internal net".to_owned())
                .and_then(|expression| self.path_terminal(expression));
            let group = terminal.as_ref().ok().and_then(|terminal| {
                let signal = self.model.signal(terminal.info.ir);
                let (group, _) = signal.net_driver?;
                (signal.net_alias.is_empty()
                    && signal.alias.is_none()
                    && self.model.net_groups[group].width == terminal.info.width
                    && load.bit < terminal.width
                    && !taken.contains(&group))
                .then_some((group, terminal.info.ir, terminal.lsb + load.bit))
            });
            let Some((group, signal, bit)) = group else {
                let message = format!(
                    "the interconnect delays into `{name}` are not annotated: llg delays the \
                     values of an input or inout port net of its own that no module path \
                     drives"
                );
                if reported.insert(message.clone()) {
                    self.warnings.push(message);
                }
                continue;
            };
            match group_index.get(&group) {
                Some(&known) => groups[known].bits.push((index, bit)),
                None => {
                    group_index.insert(group, groups.len());
                    groups.push(LoadGroup {
                        group,
                        signal,
                        bits: vec![(index, bit)],
                        name,
                    });
                }
            }
        }
        for LoadGroup {
            group,
            signal,
            bits,
            name,
        } in groups
        {
            let set = self.model.module_paths.len() as u32;
            let mut paths = Vec::with_capacity(bits.len());
            for (position, (index, bit)) in bits.iter().enumerate() {
                paths.push(
                    IrModulePath::new(
                        0,
                        *bit,
                        1,
                        *bit,
                        1,
                        false,
                        IrPathPolarity::Unknown,
                        [0; PATH_TRANSITIONS],
                        name.clone(),
                    )
                    .with_interconnect(),
                );
                let load = &mut state.load_order[*index];
                load.load = Some(self.model.sdf_loads.len() as u32);
                self.model.sdf_loads.push(IrSdfLoad {
                    set,
                    destination: 0,
                    path: position as u32,
                    entries: load.sources.len() as u32 + 1,
                });
            }
            self.model.module_paths.push(IrModulePathSet::new(
                name,
                vec![signal],
                vec![IrModulePathDestination::new(group, paths)],
            ));
        }
        Ok(())
    }

    /// Whether an annotation rewrites a timing check limit of `instance`.
    pub(in super::super) fn sdf_check_instance(&self, instance: NodeId) -> bool {
        self.sdf
            .as_ref()
            .is_some_and(|state| state.checks.contains(&instance))
    }

    /// Whether an annotation rewrites a `$setuphold`/`$recrem` limit of
    /// `instance` while negative timing checks are enabled.
    pub(in super::super) fn sdf_pair_instance(&self, instance: NodeId) -> bool {
        self.sdf
            .as_ref()
            .is_some_and(|state| state.pairs.contains(&instance))
    }

    /// The writable delay slot of `gate` when an annotation targets it.
    pub(in super::super) fn sdf_gate_slot(&self, gate: NodeId) -> Option<u32> {
        self.sdf.as_ref()?.gates.get(&gate).copied()
    }

    /// The annotation a `$sdf_annotate` call applies: `Some(None)` when it
    /// annotates nothing, `None` for a call the pre-pass did not see.
    pub(in super::super) fn sdf_call_annotation(&self, call: NodeId) -> Option<Option<u32>> {
        self.sdf.as_ref()?.calls.get(&call).copied()
    }

    /// Map the valued operations onto the model rows lowering made, after
    /// module paths and timing checks are lowered.
    pub(in super::super) fn map_sdf_annotations(&mut self, tops: &[NodeId]) -> Result<(), String> {
        let Some(mut state) = self.sdf.take() else {
            return Ok(());
        };
        let (catalog, _) = self.build_sdf_catalog(tops);
        let mut unmapped = Vec::new();
        let mut negative_limits = Vec::new();
        for (file, annotation, mipd) in &state.annotations {
            let mut ir = IrSdfAnnotation {
                file: file.clone(),
                ops: Vec::new(),
                values: Vec::new(),
            };
            for op in &annotation.ops {
                let values = annotation.values_of(op);
                let flags = if op.increment { SDF_INCREMENT } else { 0 }
                    | if op.percent { SDF_PERCENT } else { 0 }
                    | (mipd.code() << SDF_MIPD_SHIFT);
                let before = ir.ops.len();
                let mut push = |target: IrSdfTarget, values: &[i64]| {
                    let first = ir.values.len() as u32;
                    ir.values.extend_from_slice(values);
                    ir.ops.push(IrSdfOp {
                        target,
                        flags,
                        delay: op.masks.delay,
                        reject: op.masks.reject,
                        error: op.masks.error,
                        derive: op.masks.derive,
                        first,
                    });
                };
                match op.target {
                    SdfAppliedTarget::Path {
                        scope,
                        path,
                        input,
                        output,
                    } => {
                        for row in &catalog.scope(scope).paths[path as usize].rows {
                            if row.input == input && row.output == output {
                                push(
                                    IrSdfTarget::Path {
                                        set: row.set,
                                        destination: row.destination,
                                        path: row.path,
                                    },
                                    values,
                                );
                            }
                        }
                    }
                    SdfAppliedTarget::Check { scope, check, slot } => {
                        let declared = &catalog.scope(scope).checks[check as usize];
                        let combined = matches!(
                            declared.kind,
                            SdfTimingCheckKind::SetupHold | SdfTimingCheckKind::RecRem
                        );
                        // A resettled pair takes the declared limit itself.
                        if let Some(pair) = declared.rows.iter().find_map(|row| row.pair) {
                            let set = declared.rows[0].set;
                            let after = (declared.kind == SdfTimingCheckKind::SetupHold)
                                == (slot == SdfLimitSlot::Second);
                            push(IrSdfTarget::Pair { set, pair, after }, values);
                            continue;
                        }
                        // Without negative timing checks a negative limit of
                        // a combined check counts as 0 (31.9.4).
                        let clamped = [values[0].max(0)];
                        let values = if combined && values[0] < 0 {
                            negative_limits.push(format!(
                                "{file}:{}: the negative limit is annotated as 0: negative \
                                 limits count as 0 unless negative timing checks are \
                                 enabled (--neg-timing-checks; IEEE 1800-2009 31.9.4)",
                                op.pos
                            ));
                            &clamped[..]
                        } else {
                            values
                        };
                        for row in &declared.rows {
                            let second = match (slot, row.slot) {
                                (SdfLimitSlot::First, SdfLimitSlot::First | SdfLimitSlot::Both)
                                | (SdfLimitSlot::Second, SdfLimitSlot::Second) => false,
                                (SdfLimitSlot::Second, SdfLimitSlot::Both) => true,
                                _ => continue,
                            };
                            push(
                                IrSdfTarget::Check {
                                    set: row.set,
                                    check: row.check,
                                    second,
                                },
                                values,
                            );
                        }
                    }
                    SdfAppliedTarget::Primitive { scope } => {
                        let gate = state.nodes[scope as usize];
                        if let Some(&slot) = state.gates.get(&gate) {
                            push(IrSdfTarget::Gate { slot }, values);
                        }
                    }
                    SdfAppliedTarget::Port { load, source } => {
                        let instance = state.nodes[load.scope as usize];
                        for bit in sdf_port_bits(&catalog, &load) {
                            let Some(&index) = state.loads.get(&(instance, load.port, bit)) else {
                                continue;
                            };
                            let entry = &state.load_order[index as usize];
                            let Some(load) = entry.load else { continue };
                            let source = source.map_or(0, |source| entry.sources[&source]);
                            push(
                                IrSdfTarget::Load {
                                    load,
                                    entry: source,
                                },
                                values,
                            );
                        }
                    }
                }
                if ir.ops.len() == before {
                    unmapped.push(format!(
                        "{file}:{}: the {} has no simulated target; it is not annotated",
                        op.pos,
                        match op.target {
                            SdfAppliedTarget::Path { .. } => "path delay",
                            SdfAppliedTarget::Check { .. } => "timing check limit",
                            SdfAppliedTarget::Primitive { .. } => "primitive delay",
                            SdfAppliedTarget::Port { .. } => "interconnect delay",
                        }
                    ));
                }
            }
            self.model.sdf_annotations.push(ir);
        }
        let mut reported: HashSet<String> = self.warnings.iter().cloned().collect();
        for message in unmapped.into_iter().chain(negative_limits) {
            if reported.insert(message.clone()) {
                self.warnings.push(message);
            }
        }
        // Gate lowering replaces each slot's delays with the gate's own.
        self.model.sdf_gate_delays =
            vec![crate::sim::ir::IrTransitionDelay::uniform(0); state.gate_order.len()];
        self.model.sdf_startup = std::mem::take(&mut state.startup);
        self.sdf_logs = std::mem::take(&mut state.logs);
        // Calls are still lowered after this; keep what they read.
        state.annotations.clear();
        state.read.clear();
        self.sdf = Some(state);
        Ok(())
    }
}

/// The bit offsets in its port (0 = least significant) a load port target
/// selects: every bit of the port, or the selected bits of its declared
/// range.
fn sdf_port_bits(catalog: &SdfTargets, target: &SdfPortTarget) -> Vec<u32> {
    let port = &catalog.scope(target.scope).ports[target.port as usize];
    let Some((left, right)) = port.range else {
        return vec![0];
    };
    let offset = |bit: i64| {
        if left >= right {
            bit - right
        } else {
            right - bit
        }
    };
    let (high, low) = target.bits.unwrap_or((left, right));
    let (first, last) = (high.min(low), high.max(low));
    (first..=last)
        .map(|bit| offset(bit) as u32)
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::SdfCommand;

    fn parsed(text: &str) -> (Option<&'static str>, Option<String>, String) {
        let command = SdfCommand::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"));
        let mtm = command.mtm.as_deref().map(|mtm| match mtm {
            "MINIMUM" => "min",
            "TYPICAL" => "typ",
            "MAXIMUM" => "max",
            other => panic!("unexpected selector {other}"),
        });
        (mtm, command.scope, command.file)
    }

    #[test]
    fn sdf_operand_selectors_and_scopes() {
        assert_eq!(parsed("gates.sdf"), (None, None, "gates.sdf".into()));
        assert_eq!(
            parsed("min:gates.sdf"),
            (Some("min"), None, "gates.sdf".into())
        );
        assert_eq!(
            parsed("TYP:gates.sdf"),
            (Some("typ"), None, "gates.sdf".into())
        );
        assert_eq!(
            parsed("max:tb.dut=dir/gates.sdf"),
            (Some("max"), Some("tb.dut".into()), "dir/gates.sdf".into())
        );
        assert_eq!(
            parsed("tb.dut=gates.sdf"),
            (None, Some("tb.dut".into()), "gates.sdf".into())
        );
    }

    #[test]
    fn sdf_operand_windows_paths_are_files_on_every_host() {
        let drive = r"D:\a\lapligence\tests\fixtures\sdf_gates.sdf";
        assert_eq!(parsed(drive), (None, None, drive.into()));
        assert_eq!(
            parsed("d:/work/gates.sdf"),
            (None, None, "d:/work/gates.sdf".into())
        );
        assert_eq!(parsed("C:gates.sdf"), (None, None, "C:gates.sdf".into()));
        assert_eq!(
            parsed(&format!("typ:{drive}")),
            (Some("typ"), None, drive.into())
        );
        assert_eq!(
            parsed(&format!("tb.dut={drive}")),
            (None, Some("tb.dut".into()), drive.into())
        );
        assert_eq!(
            parsed(&format!("max:tb={drive}")),
            (Some("max"), Some("tb".into()), drive.into())
        );
        let unc = r"\\server\share\gates.sdf";
        assert_eq!(parsed(unc), (None, None, unc.into()));
        let extended = r"\\?\D:\dir\gates.sdf";
        assert_eq!(parsed(extended), (None, None, extended.into()));
        assert_eq!(
            parsed(&format!("min:{extended}")),
            (Some("min"), None, extended.into())
        );
        // `=` inside a directory of a drive or UNC path names no scope.
        let equals = r"D:\a=b\gates.sdf";
        assert_eq!(parsed(equals), (None, None, equals.into()));
        let unc_equals = r"\\host\a=b\gates.sdf";
        assert_eq!(parsed(unc_equals), (None, None, unc_equals.into()));
    }

    #[test]
    fn sdf_operand_errors() {
        for text in [
            "mx:gates.sdf",
            "minimum:gates.sdf",
            "min:",
            "=gates.sdf",
            "tb=",
            "",
        ] {
            assert!(
                SdfCommand::parse(text).is_err(),
                "`{text}` must be rejected"
            );
        }
    }
}
