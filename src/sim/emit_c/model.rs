//! Whole-model assembly, storage declarations, processes, and initialization.

use super::constants::{
    c_string_literal, emit_const, emit_const_for_real, emit_const_for_vector, round_shortreal,
};
use super::context::RCtx;
use super::expressions::{coerce_two_state, packed_default};
use super::names::{bound_identifiers, rewrite_identifiers, BoundedIdentifiers};
use super::statements::{render_stmt_impl as render_stmt, wait_any_text_in_region};
use super::EmitError;
use crate::sim::execution::{
    CallMechanism, CoroutineId, ExecutionAnalysis, ExecutionModel, ExecutionTerminator,
    ScheduleRegion, SuspensionOperation, TriggerPlan,
};
use crate::sim::ir::{
    IrConcurrentAssertionKind, IrFunc, IrModel, IrProcessKind, IrSequence, IrType, IrVpiObjectKind,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

mod interfaces;
use interfaces::{
    function_return_type, render_virtual_interface_call_bodies,
    render_virtual_interface_call_prototypes, render_virtual_interface_runtime,
};
mod classes;
use classes::{
    render_class_decls, render_virtual_dispatch_bodies, render_virtual_dispatch_prototypes,
};
mod assertions;
use assertions::{assertion_predicate_name, assertion_sequence_name, sampled_domain_callback_name};
mod storage;
use storage::{render_signal_decls, render_static_local_decls};
mod vpi;
use vpi::{render_vpi_compile_calls, render_vpi_metadata};
mod functions;
use functions::{block_stmts_of, func_params, func_prototype};
mod dpi;
use dpi::{dpi_external_prototype, dpi_helpers, internal_return_type, render_dpi_thunk};
mod processes;
use processes::process_runtime_name;
mod initialization;
mod pca_batches;
mod sharing;

/// The recursion depth guard shared by emitted functions and DPI thunks.
const LLG_MAX_FUNC_DEPTH: u32 = 256;

pub(super) fn owned_func_params(function: &IrFunc) -> String {
    func_params(function)
}
pub(super) fn owned_func_param_fields(function: &IrFunc) -> Vec<(String, String)> {
    functions::func_param_fields(function)
}
pub(super) fn owned_dpi_thunk(function: &IrFunc) -> Result<String, String> {
    render_dpi_thunk(function)
}

// ── Model rendering ───────────────────────────────────────────────────────────

/// Render the complete `model.c` for a lowered (and optimized) IR model:
/// the header comment the driver parses, signal/net/array storage, function
/// prototypes and bodies, process functions, and `main()`.
pub fn render(execution: &ExecutionModel) -> Result<String, EmitError> {
    Ok(render_with_symbols(execution)?.source)
}

pub(crate) fn render_with_symbols(
    execution: &ExecutionModel,
) -> Result<BoundedIdentifiers, EmitError> {
    render_bounded(execution, sharing::threshold().map_err(EmitError::new)?)
}

#[cfg(test)]
pub(in crate::sim::emit_c) fn render_with_sharing_threshold(
    execution: &ExecutionModel,
    threshold: usize,
) -> Result<String, EmitError> {
    Ok(render_bounded(execution, threshold)?.source)
}

fn render_bounded(
    execution: &ExecutionModel,
    threshold: usize,
) -> Result<BoundedIdentifiers, EmitError> {
    execution.validate().map_err(EmitError::InvalidIr)?;
    let capacity = execution
        .packed_capacity()
        .map_err(EmitError::InvalidIr)?
        .max(64);
    if capacity >= u128::from(super::LLG_WIDTH_LIMIT) {
        return Err(EmitError::new(format!(
            "packed width {capacity} reaches the C runtime exclusive limit {}",
            super::LLG_WIDTH_LIMIT
        )));
    }
    super::owned::model::check_model(execution.ir()).map_err(EmitError::new)?;
    let mut execution = execution.clone();
    let (_, upper_bounds) = render_coroutine_functions(&execution).map_err(EmitError::new)?;
    let forced = upper_bounds
        .iter()
        .filter_map(|(function, size)| {
            super::frame_layout::requires_arena(*size, execution.analysis().options().embed_limit)
                .then_some(*function)
        })
        .collect::<BTreeSet<_>>();
    execution
        .reanalyze_with_forced_arena_callees(&forced)
        .map_err(EmitError::InvalidIr)?;
    render_model(&execution, threshold).map_err(EmitError::new)
}

struct CoroutineArtifact {
    source: String,
    layout: super::frame_layout::FrameLayout,
    frame_type: String,
    desc_name: String,
    display_name: String,
    location: String,
    owner: CoroutineId,
    root: bool,
    shared_entry: Option<String>,
    pca_batches: Vec<super::statements::pca_batches::Batch>,
}

type CoroutineArtifacts = BTreeMap<usize, CoroutineArtifact>;
type CoroutineUpperBounds = BTreeMap<usize, usize>;

fn origin_location(origin: &crate::sim::semantic::Origin) -> String {
    match origin {
        crate::sim::semantic::Origin::Source {
            path, line, column, ..
        } => format!("{path}:{line}:{column}"),
        crate::sim::semantic::Origin::Synthetic { reason } => format!("<synthetic: {reason}>"),
    }
}

fn render_coroutine_functions(
    execution: &ExecutionModel,
) -> Result<(CoroutineArtifacts, CoroutineUpperBounds), String> {
    let model = execution.ir();
    let mut artifacts = BTreeMap::new();
    let mut upper_bounds = BTreeMap::new();
    for &index in execution.analysis().callee_first_functions() {
        let function = &model.funcs[index];
        let ctx = RCtx {
            model,
            func: Some(function),
            sampled: false,
            activation_label: None,
        };
        let (source, layout) = super::owned::model::coroutine_function(
            &ctx,
            function,
            index,
            execution.analysis(),
            &upper_bounds,
        )?;
        let upper_bound = layout.upper_bound()?;
        upper_bounds.insert(index, upper_bound);
        artifacts.insert(
            index,
            CoroutineArtifact {
                source,
                layout,
                frame_type: format!("{}_frame_t", function.c_name),
                desc_name: format!("{}_desc", function.c_name),
                display_name: function.diagnostic_name().to_owned(),
                location: origin_location(function.origin()),
                owner: CoroutineId::Function(index),
                root: false,
                shared_entry: None,
                pca_batches: Vec::new(),
            },
        );
    }
    Ok((artifacts, upper_bounds))
}

fn render_coroutine_processes(
    execution: &ExecutionModel,
    upper_bounds: &BTreeMap<usize, usize>,
) -> Result<Vec<Option<CoroutineArtifact>>, String> {
    let model = execution.ir();
    let ctx = RCtx {
        model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    execution
        .processes()
        .iter()
        .enumerate()
        .map(|(index, executable)| {
            let process = &model.processes[executable.semantic_process];
            if process.kind() == IrProcessKind::Final {
                return Ok(None);
            }
            let (source, layout, pca_batches) = super::owned::model::coroutine_process(
                &ctx,
                process,
                index,
                executable,
                execution.analysis(),
                upper_bounds,
            )?;
            Ok(Some(CoroutineArtifact {
                source,
                layout,
                frame_type: format!("{}_frame_t", process.c_name),
                desc_name: format!("{}_desc", process.c_name),
                display_name: process.label().to_owned(),
                location: origin_location(process.origin()),
                owner: CoroutineId::Process(index),
                root: true,
                shared_entry: None,
                pca_batches,
            }))
        })
        .collect()
}

fn render_coroutine_branches(
    execution: &ExecutionModel,
    upper_bounds: &BTreeMap<usize, usize>,
) -> Result<BTreeMap<CoroutineId, CoroutineArtifact>, String> {
    fn pre_name(pre: &crate::sim::ir::IrPreFn) -> &str {
        match pre {
            crate::sim::ir::IrPreFn::Branch { c_name, .. }
            | crate::sim::ir::IrPreFn::CapturedBranch { c_name, .. }
            | crate::sim::ir::IrPreFn::MonEval { c_name, .. }
            | crate::sim::ir::IrPreFn::EventAssign { c_name, .. }
            | crate::sim::ir::IrPreFn::DisplayEval { c_name, .. }
            | crate::sim::ir::IrPreFn::RealEval { c_name, .. }
            | crate::sim::ir::IrPreFn::DeferredAssertion { c_name, .. }
            | crate::sim::ir::IrPreFn::ForceEval { c_name, .. } => c_name,
        }
    }
    let model = execution.ir();
    let ctx = RCtx {
        model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut artifacts = BTreeMap::new();
    for (function, definition) in model.funcs.iter().enumerate() {
        for (helper, pre) in definition.pre_fns.iter().enumerate() {
            let owner = CoroutineId::FunctionBranch { function, helper };
            if !execution.analysis().is_coroutine(owner) {
                continue;
            }
            let (source, layout) = super::owned::model::coroutine_branch(
                &ctx,
                pre,
                owner,
                execution.analysis(),
                upper_bounds,
            )?;
            let name = pre_name(pre);
            artifacts.insert(
                owner,
                CoroutineArtifact {
                    source,
                    layout,
                    frame_type: format!("{name}_frame_t"),
                    desc_name: format!("{name}_desc"),
                    display_name: format!("{}.fork", definition.diagnostic_name()),
                    location: origin_location(definition.origin()),
                    owner,
                    root: true,
                    shared_entry: None,
                    pca_batches: Vec::new(),
                },
            );
        }
    }
    for (process, definition) in model.processes.iter().enumerate() {
        for (helper, pre) in definition.pre_fns.iter().enumerate() {
            let owner = CoroutineId::ProcessBranch { process, helper };
            if !execution.analysis().is_coroutine(owner) {
                continue;
            }
            let (source, layout) = super::owned::model::coroutine_branch(
                &ctx,
                pre,
                owner,
                execution.analysis(),
                upper_bounds,
            )?;
            let name = pre_name(pre);
            artifacts.insert(
                owner,
                CoroutineArtifact {
                    source,
                    layout,
                    frame_type: format!("{name}_frame_t"),
                    desc_name: format!("{name}_desc"),
                    display_name: format!("{}.fork", definition.label()),
                    location: origin_location(definition.origin()),
                    owner,
                    root: true,
                    shared_entry: None,
                    pca_batches: Vec::new(),
                },
            );
        }
    }
    Ok(artifacts)
}

fn render_coroutine_metadata(
    artifact: &CoroutineArtifact,
    analysis: &ExecutionAnalysis,
    model: &IrModel,
) -> Result<String, String> {
    let sites = analysis
        .sites(artifact.owner)
        .ok_or_else(|| format!("missing coroutine sites for {:?}", artifact.owner))?;
    let mut ordered = sites.values().collect::<Vec<_>>();
    ordered.sort_by_key(|site| site.resume());
    let mut out = format!(
        "static const llg_co_site_t {}_sites[{}] = {{\n    {{0}},\n",
        artifact.desc_name,
        ordered.len() + 1
    );
    for site in ordered {
        let call = artifact
            .layout
            .calls()
            .iter()
            .find(|call| call.resume == site.resume());
        let (callee, offset) = match (site.operation(), site.mechanism(), call) {
            (
                SuspensionOperation::Call {
                    callee: Some(callee),
                },
                Some(CallMechanism::Polled { .. }),
                Some(call),
            ) => (
                format!("&{}_desc", model.funcs[*callee].c_name),
                format!("offsetof({}, {})", artifact.frame_type, call.path),
            ),
            _ => ("NULL".to_owned(), "0".to_owned()),
        };
        out.push_str(&format!(
            "    {{ {callee}, {offset}, 0, {} }},\n",
            c_string_literal(
                &site
                    .origin()
                    .map(origin_location)
                    .unwrap_or_else(|| artifact.location.clone())
            )
        ));
    }
    out.push_str("};\n");
    let entry = artifact
        .desc_name
        .strip_suffix("_desc")
        .ok_or_else(|| format!("invalid coroutine descriptor name {}", artifact.desc_name))?;
    out.push_str(&format!(
        "static const llg_co_desc_t {} = {{ {}, {}, sizeof({}), {}_sites, {}, 0 }};\n",
        artifact.desc_name,
        artifact.shared_entry.as_deref().unwrap_or(entry),
        c_string_literal(&artifact.display_name),
        artifact.frame_type,
        artifact.desc_name,
        sites.len() + 1
    ));
    Ok(out)
}

/// One frame typedef shared by every coroutine whose layout it describes.
struct SharedFrameType {
    name: String,
    typedef: String,
    root: bool,
    anchored: bool,
}

/// Emit one typedef per distinct coroutine frame layout.
///
/// Every instance of one process or task (generate loops, repeated module
/// instances) has an identical frame layout, so a struct per coroutine
/// repeated the same definition thousands of times. Frames are grouped by
/// their rendered layout after replacing each embedded callee frame type with
/// its own group, so grouping is exact and independent of function names.
/// Artifacts are visited callee-first, so a callee's group always exists when
/// a caller is grouped. A layout used by one coroutine keeps its
/// `<fn>_frame_t` name; a shared layout is named `llg_shared_frame_<k>_t`.
/// Each artifact's `frame_type` and source are rewritten to the shared name;
/// the returned list holds each typedef once, in callee-first order.
fn share_frame_types(
    execution: &ExecutionModel,
    functions: &mut CoroutineArtifacts,
    processes: &mut [Option<CoroutineArtifact>],
    branches: &mut BTreeMap<CoroutineId, CoroutineArtifact>,
) -> Result<Vec<SharedFrameType>, String> {
    const SELF_NAME: &str = "__llg_frame_self__";
    const GROUP_PREFIX: &str = "__llg_frame_group_";
    let mut by_index = functions
        .iter_mut()
        .map(|(index, artifact)| (*index, artifact))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = Vec::new();
    for index in execution.analysis().callee_first_functions() {
        if let Some(artifact) = by_index.remove(index) {
            ordered.push(artifact);
        }
    }
    ordered.extend(by_index.into_values());
    ordered.extend(processes.iter_mut().flatten());
    ordered.extend(branches.values_mut());

    // Group every frame by its layout text with callee frame types replaced
    // by group tokens.
    let mut group_of_type = HashMap::<String, usize>::new();
    let mut group_by_key = HashMap::<String, usize>::new();
    let mut groups = Vec::<(String, String, usize)>::new(); // key, first type, members
    let mut member_groups = Vec::with_capacity(ordered.len());
    for artifact in &ordered {
        let text = artifact.layout.render_typedef(SELF_NAME)?;
        let key = rewrite_identifiers(&text, |identifier| {
            group_of_type
                .get(identifier)
                .map(|group| format!("{GROUP_PREFIX}{group}__"))
        });
        let group = match group_by_key.get(&key) {
            Some(group) => *group,
            None => {
                groups.push((key.clone(), artifact.frame_type.clone(), 0));
                group_by_key.insert(key, groups.len() - 1);
                groups.len() - 1
            }
        };
        groups[group].2 += 1;
        group_of_type.insert(artifact.frame_type.clone(), group);
        member_groups.push(group);
    }

    let mut shared = 0usize;
    let names = groups
        .iter()
        .map(|(_, first_type, members)| {
            if *members > 1 {
                shared += 1;
                format!("llg_shared_frame_{}_t", shared - 1)
            } else {
                first_type.clone()
            }
        })
        .collect::<Vec<_>>();
    let mut frame_types = groups
        .iter()
        .zip(&names)
        .map(|((key, _, _), name)| {
            let typedef = rewrite_identifiers(key, |identifier| {
                if identifier == SELF_NAME {
                    return Some(name.clone());
                }
                identifier
                    .strip_prefix(GROUP_PREFIX)
                    .and_then(|rest| rest.strip_suffix("__"))
                    .and_then(|group| group.parse::<usize>().ok())
                    .and_then(|group| names.get(group).cloned())
            });
            SharedFrameType {
                name: name.clone(),
                typedef,
                root: false,
                anchored: false,
            }
        })
        .collect::<Vec<_>>();

    let renamed = group_of_type
        .iter()
        .filter(|(original, group)| **original != names[**group])
        .map(|(original, group)| (original.clone(), names[*group].clone()))
        .collect::<HashMap<_, _>>();
    for (artifact, group) in ordered.into_iter().zip(member_groups) {
        if artifact.root {
            frame_types[group].root = true;
        } else {
            frame_types[group].anchored = true;
        }
        artifact.frame_type = names[group].clone();
        if !renamed.is_empty() {
            artifact.source = rewrite_identifiers(&artifact.source, |identifier| {
                renamed.get(identifier).cloned()
            });
        }
    }
    Ok(frame_types)
}

fn render_model(
    execution: &ExecutionModel,
    threshold: usize,
) -> Result<BoundedIdentifiers, String> {
    let model = execution.ir();
    let (mut coroutine_functions, frame_upper_bounds) = render_coroutine_functions(execution)?;
    let mut coroutine_processes = render_coroutine_processes(execution, &frame_upper_bounds)?;
    let mut coroutine_branches = render_coroutine_branches(execution, &frame_upper_bounds)?;
    let frame_types = share_frame_types(
        execution,
        &mut coroutine_functions,
        &mut coroutine_processes,
        &mut coroutine_branches,
    )?;
    let mut plain_functions = BTreeMap::new();
    for (index, function) in model.funcs.iter().enumerate() {
        if !super::owned::model::inline_template(function)
            && !coroutine_functions.contains_key(&index)
        {
            let ctx = RCtx {
                model,
                func: Some(function),
                sampled: false,
                activation_label: None,
            };
            plain_functions.insert(index, super::owned::model::function(&ctx, function)?);
        }
    }
    let pca_tables = pca_batches::collect(model, &mut coroutine_processes)?;
    let sharing = sharing::share(
        execution,
        &mut coroutine_functions,
        &mut coroutine_processes,
        &mut coroutine_branches,
        &mut plain_functions,
        threshold,
        &pca_tables.operands,
    )?;
    let mut out = format!(
        "// llg-generated C11 model for design `{}`\n",
        model.design_name
    );
    out.push_str(&format!(
        "#define LLG_MODEL_VALUE_ABI {}\n",
        super::VALUE_ABI_VERSION
    ));
    out.push_str("#define LLG_MODEL_PROCESS_ABI 3\n");
    if model.waveform {
        out.push_str("#define LLG_WAVEFORM 1\n");
    }
    out.push_str("#include \"llg_rt.h\"\n");
    out.push_str("#if LLG_MODEL_PROCESS_ABI != LLG_PROCESS_ABI_VERSION\n#error \"generated model process ABI does not match llg_rt.h\"\n#endif\n");
    out.push_str("#include \"llg_random.h\"\n");
    out.push_str("#include \"llg_vpi.h\"\n");
    out.push_str("_Static_assert(LLG_MODEL_VALUE_ABI == LLG_VALUE_ABI_VERSION, \"regenerate model: incompatible value ownership ABI\");\n");
    if !model.containers.is_empty() {
        out.push_str("#include \"llg_container.h\"\n");
    }
    out.push_str("#include \"llg_string.h\"\n");
    if model.funcs.iter().any(|func| func.dpi_import().is_some()) {
        out.push_str("#include \"svdpi.h\"\n");
    }
    if model.waveform {
        out.push_str("#include \"llg_wave.h\"\n");
    }
    out.push_str(
        "\n#include <stdio.h>\n#include <stdlib.h>\n#include <math.h>\n#include <string.h>\n\n",
    );
    if !sharing.bodies.is_empty() || !pca_tables.operands.is_empty() {
        out.push_str("#if defined(__GNUC__) && !defined(__clang__)\n#define LLG_MODEL_SHARED __attribute__((noipa))\n#elif defined(__clang__)\n#define LLG_MODEL_SHARED __attribute__((noinline))\n#elif defined(_MSC_VER)\n#define LLG_MODEL_SHARED __declspec(noinline)\n#else\n#define LLG_MODEL_SHARED\n#endif\n");
    }
    let mut entries = BTreeSet::new();
    for artifact in coroutine_functions
        .values()
        .chain(coroutine_branches.values())
        .chain(coroutine_processes.iter().filter_map(Option::as_ref))
    {
        let entry = artifact
            .desc_name
            .strip_suffix("_desc")
            .ok_or_else(|| format!("invalid coroutine descriptor name {}", artifact.desc_name))?;
        if artifact.shared_entry.is_none() && entries.insert(entry) {
            out.push_str(&format!(
                "static llg_co_status_t {entry}(llg_co_frame_t* co, llg_co_chain_t* ch);\n"
            ));
        }
        out.push_str(&format!(
            "static const llg_co_desc_t {};\n",
            artifact.desc_name
        ));
    }
    for frame_type in &frame_types {
        out.push_str(&frame_type.typedef);
        if frame_type.root {
            out.push_str(&format!("LLG_CO_ROOT_FRAME_OK({});\n", frame_type.name));
        }
        if frame_type.anchored {
            out.push_str(&format!("LLG_CO_ANCHORED_OK({});\n", frame_type.name));
        }
    }
    // Shared root frames are declared with the records below, after model storage.
    let mut coroutine_metadata = String::new();
    for &index in execution.analysis().callee_first_functions() {
        let artifact = &coroutine_functions[&index];
        coroutine_metadata.push_str(&render_coroutine_metadata(
            artifact,
            execution.analysis(),
            model,
        )?);
    }
    for artifact in coroutine_processes.iter().filter_map(Option::as_ref) {
        coroutine_metadata.push_str(&render_coroutine_metadata(
            artifact,
            execution.analysis(),
            model,
        )?);
    }
    for artifact in coroutine_branches.values() {
        coroutine_metadata.push_str(&render_coroutine_metadata(
            artifact,
            execution.analysis(),
            model,
        )?);
    }
    out.push_str("/* signals start all-X; driven by processes and link processes */\n");
    if model.containers.iter().any(|container| {
        matches!(
            container.kind,
            crate::sim::ir::IrContainerKind::Associative {
                key: crate::sim::ir::IrAssocKey::String
            }
        )
    }) {
        out.push_str(super::owned::containers::key_adapters());
    }
    super::owned::native::helpers(&mut out);
    render_class_decls(model, &mut out);
    super::owned::udp::tables(model, &mut out);
    render_signal_decls(model, &mut out);
    render_vpi_metadata(model, &mut out);
    render_vpi_compile_calls(model, &mut out);
    render_static_local_decls(model, &mut out);
    super::owned::model::persistent_returns(model, &mut out);
    for container in &model.containers {
        out.push_str(&super::containers::declaration_and_init(container)?.0);
    }
    for object in &model.objects {
        if object.ty == crate::sim::ir::IrObjectType::String {
            out.push_str(&format!(
                "static sv4_t {}_llg_dep = SV4_EMPTY;\n",
                object.c_name
            ));
        }
        let ty = match object.ty {
            crate::sim::ir::IrObjectType::String => "llg_string_t",
            crate::sim::ir::IrObjectType::Chandle => "void *",
            crate::sim::ir::IrObjectType::Semaphore => "void *",
            crate::sim::ir::IrObjectType::Process => "llg_process_handle_t *",
        };
        out.push_str(&format!("static {ty} {} = {{0}};\n", object.c_name));
    }
    out.push('\n');
    // Arrays start all-X; elements are filled in `main()` (a function call
    // is not a valid static initializer).
    for a in model.arrays.iter().filter(|array| !array.activation) {
        if a.sparse() {
            out.push_str(&format!(
                "llg_fixed_array_t {} = {{0}};\nstatic sv4_t {}_llg_contents_dep = SV4_EMPTY;\n",
                a.c_name, a.c_name
            ));
            continue;
        }
        out.push_str(&format!(
            "{} {}[{}];\n",
            if a.real { "double" } else { "sv4_t" },
            a.c_name,
            a.total
        ));
        out.push_str(&format!(
            "static sv4_t {}_llg_contents_dep = SV4_EMPTY;\n\
             static sv4_t {}_llg_element_deps[{}];\n",
            a.c_name, a.c_name, a.total
        ));
    }
    for (array_index, array) in model.arrays.iter().enumerate() {
        for (index, signal) in &array.net_elements {
            let signal = &model.signals[*signal];
            let name = format!("llg_array_net_{array_index}_{index}");
            let bindings = if let Some((group, slot)) = signal.net_driver {
                (0..array.elem_width)
                    .map(|bit| crate::sim::ir::IrNetAliasBinding {
                        group,
                        slot,
                        signal_bit: bit,
                        group_bit: bit,
                    })
                    .collect::<Vec<_>>()
            } else {
                signal.net_alias.clone()
            };
            let parts = bindings
                .iter()
                .map(|binding| {
                    format!(
                        "{{ &{}, {}, {}, {} }}",
                        model.net_groups[binding.group].c_name,
                        binding.slot,
                        binding.signal_bit,
                        binding.group_bit
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "static const llg_net_alias_part_t {name}_parts[] = {{ {parts} }};\n"
            ));
            out.push_str(&format!("static llg_net_alias_t {name} = {{ .storage = &{}[{index}], .width = {}, .is_signed = {}, .parts = {name}_parts, .n_parts = {}, .publication_target = &{}[{index}] }};\n",
                array.c_name, array.elem_width, u8::from(array.signed), array.elem_width, array.c_name));
        }
    }
    for container in &model.containers {
        out.push_str(&format!(
            "static sv4_t {}_llg_contents_dep = SV4_EMPTY;\n\
             static sv4_t {}_llg_shape_dep = SV4_EMPTY;\n",
            container.c_name, container.c_name
        ));
    }
    out.push('\n');
    render_virtual_interface_runtime(model, &mut out);
    if model.waveform {
        out.push_str(
            "static uint64_t llg_wave_final_time;\n\
             static void llg_wave_capture_final_time(void) {\n\
             \x20   llg_wave_final_time = llg_time();\n\
             \x20   return;\n\
             }\n\n",
        );
    }
    // Functions/tasks become static C functions (prototypes first so bodies
    // may call each other regardless of declaration order), emitted before
    // any process code references them.
    if model.funcs.iter().any(|func| func.dpi_import().is_some()) {
        out.push_str(dpi_helpers());
    }
    for (index, f) in model.funcs.iter().enumerate() {
        if super::owned::model::inline_template(f) {
            continue;
        }
        if execution.analysis().is_coroutine_function(index) {
            continue;
        } else {
            out.push_str(&func_prototype(f)?);
        }
    }
    out.push_str(&pca_tables.declarations);
    out.push_str(&sharing.declarations);
    out.push_str(&sharing.prototypes);
    out.push_str(&coroutine_metadata);
    render_virtual_dispatch_prototypes(model, &mut out);
    render_virtual_interface_call_prototypes(model, &mut out);
    render_virtual_dispatch_bodies(model, &mut out);
    let ctx = RCtx {
        model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    for (index, f) in model.funcs.iter().enumerate() {
        if super::owned::model::inline_template(f) {
            continue;
        }
        for (helper, pre) in f.pre_fns.iter().enumerate() {
            let owner = CoroutineId::FunctionBranch {
                function: index,
                helper,
            };
            if let Some(artifact) = coroutine_branches.get(&owner) {
                out.push_str(&artifact.source);
            } else {
                out.push_str(&super::owned::model::pre_function(&ctx, pre)?);
            }
        }
        if let Some(artifact) = coroutine_functions.get(&index) {
            out.push_str(&artifact.source);
        } else {
            out.push_str(&plain_functions[&index]);
        }
    }
    render_virtual_interface_call_bodies(model, &mut out);
    // Three passes lower comb drivers, links, then always/initial processes,
    // so every comb process, link, and process runs at t=0 in that order;
    // push order equals spawn order.
    for (index, executable) in execution.processes().iter().enumerate() {
        let p = &model.processes[executable.semantic_process];
        for (helper, pre) in p.pre_fns.iter().enumerate() {
            let owner = CoroutineId::ProcessBranch {
                process: executable.semantic_process,
                helper,
            };
            if let Some(artifact) = coroutine_branches.get(&owner) {
                out.push_str(&artifact.source);
            } else {
                out.push_str(&super::owned::model::pre_function(&ctx, pre)?);
            }
        }
        if let Some(artifact) = &coroutine_processes[index] {
            out.push_str(&artifact.source);
        } else {
            out.push_str(&super::owned::model::process(&ctx, p, executable)?);
        }
    }
    out.push_str(&sharing.bodies);
    out.push_str(&super::owned::assertions::callbacks(model)?);
    out.push_str(&super::owned::assertions::registrations(model)?);
    super::owned::model::storage_lifecycle(model, &mut out)?;
    out.push_str(&super::owned::model::main(execution, &sharing.spawns)?);
    let external = model
        .funcs
        .iter()
        .filter_map(|function| function.dpi_import().map(|dpi| dpi.c_name()))
        .collect::<BTreeSet<_>>();
    Ok(bound_identifiers(out, &external))
}

#[cfg(test)]
mod tests;
