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
use crate::sim::ir::{IrSdfAnnotation, IrSdfOp, IrSdfTarget, SDF_INCREMENT, SDF_PERCENT};
use crate::sim::sdf::{
    apply, parse_config, parse_scale_factors, resolve, SdfAnnotation, SdfAppliedTarget,
    SdfCallOptions, SdfLimitSlot, SdfLimits, SdfMtm, SdfScaleType, SdfTargets, SdfTimebase,
    SdfWarning,
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
    pub fn parse(text: &str) -> Result<Self, String> {
        let (mtm, rest) = match text.split_once(':') {
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
        let (scope, file) = match rest.split_once('=') {
            Some((scope, file)) => (Some(scope.to_owned()), file),
            None => (None, rest),
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
    /// The valued file of each annotation, by index.
    annotations: Vec<(String, SdfAnnotation)>,
    /// Annotations applied before simulation starts (`--sdf`).
    startup: Vec<u32>,
    /// Requests already read: their annotation, or `None`.
    read: HashMap<SdfRequest, Option<u32>>,
    /// The design node of each catalog scope.
    nodes: Vec<NodeId>,
    /// The writable delay slot of each gate a DEVICE entry annotates.
    gates: HashMap<NodeId, u32>,
    gate_order: Vec<NodeId>,
    /// Log files and their contents, in the order first written.
    logs: Vec<(String, String)>,
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
            let annotation = match self.sdf_call_request(call)? {
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
            state.calls.insert(call, annotation);
        }
        self.sdf = Some(Box::new(state));
        Ok(())
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
        let mut log = format!("$sdf_annotate(\"{}\") at {origin}\n", request.file);
        let result = self.value_sdf_request(state, catalog, scope_of, &request, &mut log);
        let annotation = match result {
            Ok(annotation) => Some(annotation),
            Err(message) => {
                let message =
                    format!("$sdf_annotate at {origin}: {message}; the call annotates nothing");
                log.push_str(&format!("error: {message}\n"));
                self.warnings.push(message);
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
        let mut options = SdfCallOptions {
            tool_member: self.db.delay_selection().operand(),
            ..SdfCallOptions::default()
        };
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
            &options,
            SdfTimebase {
                design_precision_fs: self.design_precision_fs,
            },
        );
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
        for op in &annotation.ops {
            if let SdfAppliedTarget::Primitive { scope } = op.target {
                let gate = state.nodes[scope as usize];
                if !state.gates.contains_key(&gate) {
                    state.gates.insert(gate, state.gate_order.len() as u32);
                    state.gate_order.push(gate);
                }
            }
        }
        let index = state.annotations.len() as u32;
        state.annotations.push((request.file.clone(), annotation));
        Ok(index)
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
        for (file, annotation) in &state.annotations {
            let mut ir = IrSdfAnnotation {
                file: file.clone(),
                ops: Vec::new(),
                values: Vec::new(),
            };
            for op in &annotation.ops {
                let values = annotation.values_of(op);
                let flags = if op.increment { SDF_INCREMENT } else { 0 }
                    | if op.percent { SDF_PERCENT } else { 0 };
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
                        for row in &catalog.scope(scope).checks[check as usize].rows {
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
                    SdfAppliedTarget::Port { .. } => {}
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
        for message in unmapped {
            if !self.warnings.contains(&message) {
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
