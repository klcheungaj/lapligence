//! `undriven-signal` — read signals with no known active driver.
//!
//! The rule works over the owned elaborated database.  It combines procedural
//! and continuous activity with declaration initializers, primitive terminals,
//! and representable module-port flow, then reports each declared object at
//! its declaration when it is read but has no driver.

use std::collections::HashSet;

use crate::core::db::{ConstraintKind, Db, NodeId, NodeKind, RandQualifier};
use crate::core::lint::rules::analysis::{
    all_nodes, collect_driver_writes, collect_reads, connected_port_link_drivers,
    gate_terminal_drivers, gate_terminal_reads, is_signal, port_link_reads, signal_scope_path,
    subroutine_body, top_external_port_signals, unconnected_port_signals,
};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns about a signal that is read but has no known active driver.
pub struct UndrivenSignalRule;

impl LintRule for UndrivenSignalRule {
    fn id(&self) -> &'static str {
        "undriven-signal"
    }

    fn description(&self) -> &'static str {
        "warns when a declared signal is read but has no known driver"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let (reads, drivers) = design_activity(db);
        let mut exempt = top_external_port_signals(db);
        exempt.extend(unconnected_port_signals(db));
        // The built-in `option` and `type_option` members of covergroups,
        // coverpoints and crosses (SV 19.10) are coverage settings the
        // simulator holds, not signals.
        for info in db.covergroups().values() {
            let scopes = info
                .body
                .into_iter()
                .chain(info.coverpoints.iter().copied())
                .chain(info.crosses.iter().copied());
            for scope in scopes {
                exempt.extend(db.node(scope).children().iter().copied().filter(|child| {
                    matches!(db.node_kind(*child), NodeKind::Var { .. })
                        && matches!(db.node(*child).name.as_str(), "option" | "type_option")
                }));
            }
        }
        // A constraint `foreach` loop variable is bound by the solver's
        // iteration (SV 18.5.8.1), not driven by a process.
        exempt.extend(
            db.node_ids()
                .filter_map(|id| match db.node_kind(id) {
                    NodeKind::Constraint(ConstraintKind::Foreach { vars, .. }) => {
                        Some(vars.clone())
                    }
                    _ => None,
                })
                .flatten()
                .flatten(),
        );

        // `all_nodes` follows owned reference bases as well as structural
        // children, so a declaration can occur more than once in that walk.
        // Deduping by arena identity guarantees one finding per object.
        let mut seen = HashSet::new();
        let mut emitted = HashSet::new();
        let mut out = Vec::new();
        for id in all_nodes(db) {
            if !seen.insert(id) || !is_signal(db, id) {
                continue;
            }
            if !reads.contains(&id)
                || drivers.contains(&id)
                || exempt.contains(&id)
                || has_intrinsic_net_driver(db, id)
                || db.is_implicit_net(id)
                // A `rand`/`randc` property is assigned by every call of
                // `randomize()` (IEEE 1800-2009 18.4, 18.6.1).
                || db.rand_qualifier(id) != RandQualifier::None
            {
                continue;
            }
            let node = db.node(id);
            let key = (
                node.file().map(str::to_owned),
                node.line.max(1),
                node.col.max(1),
                node.name.clone(),
            );
            if !emitted.insert(key) {
                continue;
            }
            out.push(LintDiag {
                rule: "undriven-signal".to_string(),
                logical: None,
                severity: LintSeverity::Warning,
                file: node.file().map(str::to_owned),
                line: node.line.max(1),
                col: node.col.max(1),
                message: format!(
                    "signal `{}` in `{}` is read but has no known driver",
                    node.name,
                    signal_scope_path(db, id)
                ),
            });
        }
        out
    }
}

/// Gather all activity represented by the owned database.
fn design_activity(db: &Db) -> (HashSet<NodeId>, HashSet<NodeId>) {
    let mut reads = HashSet::new();
    let mut drivers = HashSet::new();

    for id in all_nodes(db) {
        match db.node_kind(id) {
            NodeKind::Process { .. } | NodeKind::ContAssign { .. } => {
                reads.extend(collect_reads(db, id));
                drivers.extend(collect_driver_writes(db, id));
            }
            NodeKind::FuncTask { .. } => {
                // Function/task bodies are not process nodes, but their
                // assignments still describe activity on captured signals.
                if let Some(body) = subroutine_body(db, id) {
                    reads.extend(collect_reads(db, body));
                    drivers.extend(collect_driver_writes(db, body));
                }
            }
            _ => {}
        }
    }

    reads.extend(gate_terminal_reads(db));
    drivers.extend(gate_terminal_drivers(db));
    reads.extend(port_link_reads(db));
    drivers.extend(connected_port_link_drivers(db));

    // Variable and array declaration initializers are not process or
    // continuous-assignment nodes.  They are still active drivers, and their
    // expressions can contain reads of other captured signals.
    for (signal, init) in db.var_initializers() {
        drivers.insert(*signal);
        reads.extend(collect_reads(db, *init));
    }
    for (signal, meta) in db.arrays() {
        if let Some(init) = meta.init {
            drivers.insert(*signal);
            reads.extend(collect_reads(db, init));
        }
    }

    (reads, drivers)
}

/// Net declarations with an intrinsic pull/supply value are conservatively
/// considered driven even without an explicit assignment or terminal.
fn has_intrinsic_net_driver(db: &Db, id: NodeId) -> bool {
    matches!(
        db.node_kind(id),
        NodeKind::Net { net_type, .. }
            if matches!(
                *net_type,
                crate::core::db::NetType::Tri0
                    | crate::core::db::NetType::Tri1
                    | crate::core::db::NetType::Supply0
                    | crate::core::db::NetType::Supply1
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::build_design;

    fn run_rule(sv: &str, top: &str) -> Vec<LintDiag> {
        let (db, model) = build_design(sv, top);
        UndrivenSignalRule.check(&LintCtx {
            db: &db,
            model: &model,
        })
    }

    #[test]
    fn reports_one_internal_signal_read_without_a_driver() {
        let diags = run_rule(
            "module top;\n  logic floating;\n  logic sink;\n  assign sink = floating;\nendmodule\n",
            "top",
        );
        assert_eq!(diags.len(), 1, "one undriven signal: {diags:?}");
        assert_eq!(diags[0].rule, "undriven-signal");
        assert_eq!(diags[0].severity, LintSeverity::Warning);
        assert!(diags[0].message.contains("`floating`"));
        assert_eq!(diags[0].line, 2, "finding is at the declaration");
    }

    #[test]
    fn reports_undriven_signal_in_named_expression_actual() {
        let diags = run_rule(
            "module child(input wire i);\n  wire observed;\n  assign observed = i;\nendmodule\n\
             module top;\n  logic floating;\n  child u0 (.i(floating & 1'b1));\nendmodule\n",
            "top",
        );
        assert_eq!(
            diags.len(),
            1,
            "named actual reads one floating signal: {diags:?}"
        );
        assert!(diags[0].message.contains("`floating`"), "{diags:?}");
    }

    #[test]
    fn reports_undriven_signal_in_positional_expression_actual() {
        let diags = run_rule(
            "module child(input wire i);\n  wire observed;\n  assign observed = i;\nendmodule\n\
             module top;\n  logic floating;\n  child u0 (floating | 1'b0);\nendmodule\n",
            "top",
        );
        assert_eq!(
            diags.len(),
            1,
            "positional actual reads one floating signal: {diags:?}"
        );
        assert!(diags[0].message.contains("`floating`"), "{diags:?}");
    }

    #[test]
    fn procedural_and_continuous_drivers_are_quiet() {
        let diags = run_rule(
            "module top;\n  logic procedural, continuous;\n  always @* procedural = continuous;\n  assign continuous = procedural;\nendmodule\n",
            "top",
        );
        assert!(diags.is_empty(), "both signals have drivers: {diags:?}");
    }

    #[test]
    fn scalar_declaration_initializer_is_a_driver() {
        let diags = run_rule(
            "module top;\n  logic initialized = 1'b0;\n  logic sink;\n  assign sink = initialized;\nendmodule\n",
            "top",
        );
        assert!(
            diags.is_empty(),
            "initializer drives initialized: {diags:?}"
        );
    }

    #[test]
    fn external_top_input_and_inout_backing_signals_are_quiet() {
        let diags = run_rule(
            "module top(input wire input_signal, inout wire bidirectional);\n  wire input_sink;\n  wire inout_sink;\n  assign input_sink = input_signal;\n  assign inout_sink = bidirectional;\nendmodule\n",
            "top",
        );
        assert!(diags.is_empty(), "external ports are exempt: {diags:?}");
    }

    #[test]
    fn child_and_gate_outputs_are_drivers() {
        let diags = run_rule(
            "module child(input wire input_signal, output wire output_signal);\n  wire observed;\n  not g(output_signal, input_signal);\n  assign observed = output_signal;\nendmodule\n\
             module top;\n  wire input_signal;\n  wire output_signal;\n  assign input_signal = 1'b0;\n  child u0 (.input_signal(input_signal), .output_signal(output_signal));\nendmodule\n",
            "top",
        );
        assert!(
            diags.is_empty(),
            "connected child/gate flow has no undriven signal: {diags:?}"
        );
    }

    #[test]
    fn unconnected_child_input_and_implicit_net_are_not_duplicated() {
        let unconnected = run_rule(
            "module child(input wire input_signal);\n  wire observed;\n  assign observed = input_signal;\nendmodule\n\
             module top;\n  child u0 ();\nendmodule\n",
            "top",
        );
        assert!(
            unconnected.is_empty(),
            "unconnected-port owns the absent connection: {unconnected:?}"
        );

        let implicit = run_rule(
            "module child(input wire value); endmodule\nmodule top;\n  child u0(undeclared_signal);\nendmodule\n",
            "top",
        );
        assert!(
            implicit.is_empty(),
            "implicit-net owns the implicit declaration: {implicit:?}"
        );
    }

    #[test]
    fn selected_write_drives_the_whole_object() {
        let diags = run_rule(
            "module top;\n  logic [3:0] selected;\n  logic sink;\n  assign selected[0] = 1'b1;\n  assign sink = selected;\nendmodule\n",
            "top",
        );
        assert!(
            diags.is_empty(),
            "a selected write drives selected as a whole: {diags:?}"
        );
    }

    #[test]
    fn identical_instance_findings_collapse_to_one() {
        let diags = run_rule(
            "module child;\n  logic floating;\n  logic sink;\n  assign sink = floating;\nendmodule\n\
             module top;\n  child c0();\n  child c1();\nendmodule\n",
            "top",
        );
        assert_eq!(
            diags.len(),
            1,
            "cloned source declaration must produce one finding: {diags:?}"
        );
        assert!(diags[0].message.contains("`floating`"));
    }

    fn undriven_names(sv: &str, top: &str) -> Vec<String> {
        let mut names: Vec<String> = run_rule(sv, top)
            .iter()
            .map(|d| d.message.split('`').nth(1).unwrap_or_default().to_string())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn dpi_import_output_and_inout_actuals_are_drivers() {
        let names = undriven_names(
            "module top;\n\
               import \"DPI-C\" function int dpi_fn(input int i, output int o, inout int io);\n\
               import \"DPI-C\" task dpi_task(output int o, inout int io);\n\
               int in_value = 1;\n  int fn_out, fn_io, task_out, task_io, sink;\n\
               initial begin\n    sink = dpi_fn(in_value, fn_out, fn_io);\n    dpi_task(task_out, task_io);\n\
                 $display(fn_out, fn_io, task_out, task_io);\n  end\nendmodule\n",
            "top",
        );
        assert!(
            names.is_empty(),
            "actuals are written by the call: {names:?}"
        );
    }

    #[test]
    fn dpi_import_input_actual_is_not_a_driver() {
        let names = undriven_names(
            "module top;\n  import \"DPI-C\" function void dpi_fn(input int i, output int o);\n\
               int never_driven, out;\n\
               initial begin\n    dpi_fn(never_driven, out);\n    $display(out);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["never_driven"], "an input actual is only read");
    }

    #[test]
    fn systemverilog_subroutine_output_inout_and_ref_actuals_are_drivers() {
        let names = undriven_names(
            "module top;\n\
               task t(output int o, inout int io);\n    o = 1;\n    io = io + 1;\n  endtask\n\
               function int f(output int o);\n    o = 2;\n    return 0;\n  endfunction\n\
               task automatic r(ref int v);\n    v = 3;\n  endtask\n\
               int a, b, c, d, sink;\n\
               initial begin\n    t(a, b);\n    sink = f(c);\n    r(d);\n    $display(a, b, c, d);\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn selected_output_actual_drives_the_whole_object() {
        let names = undriven_names(
            "module top;\n  task t(output bit o);\n    o = 1'b1;\n  endtask\n\
               logic [3:0] word;\n  int array [2];\n\
               initial begin\n    t(word[1]);\n    t(array[0][0]);\n    $display(word, array[0]);\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn const_ref_and_input_actuals_stay_read_only() {
        let names = undriven_names(
            "module top;\n  task automatic t(const ref int v, input int i);\n    $display(v, i);\n  endtask\n\
               int ro_ref, ro_in;\n  initial t(ro_ref, ro_in);\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["ro_in", "ro_ref"]);
    }

    #[test]
    fn scan_and_read_system_task_destinations_are_drivers() {
        let names = undriven_names(
            "module top;\n  int fd = 0;\n  string text = \"7 8\";\n\
               int a, b, c, rc;\n  string line;\n  int mem [4];\n  int plus;\n\
               initial begin\n    rc = $sscanf(text, \"%d %d\", a, b);\n    rc = $fscanf(fd, \"%d\", c);\n\
                 rc = $fgets(line, fd);\n    rc = $fread(mem, fd);\n    rc = $value$plusargs(\"X=%d\", plus);\n\
                 $display(a, b, c, line, mem[0], plus, rc);\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn scan_source_and_format_arguments_are_not_drivers() {
        let names = undriven_names(
            "module top;\n  string source;\n  int fd_source;\n  int dest, rc;\n\
               initial begin\n    rc = $sscanf(source, \"%d\", dest);\n    rc = $fscanf(fd_source, \"%d\", dest);\n\
                 $display(dest, rc);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["fd_source", "source"]);
    }

    #[test]
    fn string_formatting_and_readmem_destinations_are_drivers() {
        let names = undriven_names(
            "module top;\n  string formatted, written;\n  logic [7:0] image [0:3];\n  int n = 3;\n\
               initial begin\n    $sformat(formatted, \"%0d\", n);\n    $swrite(written, \"%0d\", n);\n\
                 $readmemh(\"x.hex\", image);\n    $display(formatted, written, image[0]);\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn seed_arguments_are_driven_by_the_random_functions() {
        let names = undriven_names(
            "module top;\n  int seed1, seed2, r;\n\
               initial begin\n    r = $random(seed1);\n    r = $dist_uniform(seed2, 0, 9);\n    $display(seed1, seed2, r);\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn std_randomize_arguments_are_drivers() {
        let names = undriven_names(
            "module top;\n  int a;\n  bit [3:0] b;\n  int ok;\n\
               initial begin\n    ok = std::randomize(a, b);\n    $display(a, b, ok);\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn constraint_foreach_loop_variables_are_not_signals() {
        let names = undriven_names(
            "module top;\n  int a[];\n  int ok;\n\
               initial begin\n    ok = std::randomize(a) with { foreach (a[i]) a[i] == i; };\n    $display(ok);\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn plain_system_function_arguments_stay_reads() {
        let names = undriven_names(
            "module top;\n  int value;\n  initial begin\n    $display($clog2(value));\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["value"]);
    }

    #[test]
    fn increment_and_decrement_operators_drive_their_operand() {
        let names = undriven_names(
            "module top;\n  int a, b, c, d, e, f, never;\n  int sink;\n\
               initial begin\n    a++;\n    --b;\n    ++c;\n    d--;\n    sink = e++ + f;\n\
                 $display(a, b, c, d, e, f, never, sink);\n  end\nendmodule\n",
            "top",
        );
        // `f` is only read by the sum, `never` only by the display.
        assert_eq!(names, ["f", "never"], "{names:?}");
    }

    #[test]
    fn compound_assignments_drive_their_target() {
        let names = undriven_names(
            "module top;\n  int a, b, c, d, never;\n\
               initial begin\n    a += 1;\n    b <<= 1;\n    c |= 2;\n    d -= 1;\n\
                 $display(a, b, c, d, never);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["never"], "{names:?}");
    }

    #[test]
    fn increment_inside_a_subroutine_and_a_class_method_drives() {
        let names = undriven_names(
            "module top;\n  int in_task;\n  class C;\n    int count;\n\
                 function void bump(); count++; endfunction\n\
                 function int get(); return count; endfunction\n  endclass\n\
               task t(); in_task++; endtask\n  C c = new;\n\
               initial begin\n    t();\n    c.bump();\n    $display(in_task, c.get());\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn for_loop_variables_are_driven_by_the_loop() {
        let names = undriven_names(
            "module top;\n  int i, j, k, limit;\n  int sink;\n\
               initial begin\n    for (i = 0; i < 4; i++) sink = i;\n\
                 for (j = 0; j < 4; j = j + 1) sink = j;\n\
                 for (int m = 0; m < 4; m++) sink = m;\n\
                 for (k = 0, sink = 0; k < limit; k += 2) sink = k;\n\
                 $display(i, j, sink);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["limit"], "only the undriven bound is reported");
    }

    #[test]
    fn foreach_iterator_variables_are_driven_by_the_loop() {
        let names = undriven_names(
            "module top;\n  int arr [4];\n  int mat [2][3];\n  int sink;\n  int ro;\n\
               initial begin\n    foreach (arr[n]) sink = n;\n\
                 foreach (mat[p, q]) mat[p][q] = p + q;\n\
                 foreach (arr[r]) arr[r] = r + ro;\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(
            names,
            ["ro"],
            "only the genuinely undriven variable: {names:?}"
        );
    }

    #[test]
    fn foreach_in_a_function_body_is_driven_by_the_loop() {
        let names = undriven_names(
            "module top;\n  int arr [4];\n  function int total();\n    int s;\n    s = 0;\n\
                 foreach (arr[n]) s += arr[n] + n;\n    return s;\n  endfunction\n\
               initial begin\n    arr[0] = 1;\n    $display(total());\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn rand_properties_are_driven_by_randomize() {
        let names = undriven_names(
            "module top;\n  class C;\n    rand int x;\n    randc bit [1:0] y;\n    int fixed;\n\
                 function int sum(); return x + y + fixed; endfunction\n  endclass\n\
               C c = new;\n  initial begin\n    if (!c.randomize()) $display(\"fail\");\n\
                 $display(c.sum());\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(
            names,
            ["fixed"],
            "only the non-rand, never written property"
        );
    }

    #[test]
    fn rand_property_of_a_package_class_is_driven_by_randomize() {
        let names = undriven_names(
            "package p;\n  class K;\n    rand int a;\n    function int get(); return a; endfunction\n  endclass\nendpackage\n\
             module top;\n  p::K k = new;\n  initial begin\n    void'(k.randomize());\n    $display(k.get());\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn properties_written_only_by_methods_are_driven() {
        let names = undriven_names(
            "module top;\n  class C;\n    int a, b, c, d, e;\n    int never;\n    int arr [2];\n\
                 function new(); a = 1; endfunction\n\
                 function void set(int v); this.b = v; arr[0] = v; endfunction\n\
                 task run(); #1 c = 2; d += 1; e++; endtask\n\
                 function int get(); return a + b + c + d + e + never + arr[0]; endfunction\n  endclass\n\
               C o = new;\n  initial begin\n    o.set(1);\n    o.run();\n    $display(o.get());\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["never"], "{names:?}");
    }

    #[test]
    fn concatenated_net_targets_drive_each_member() {
        let names = undriven_names(
            "module top;\n  wire a, b, x, y;\n  wire [1:0] w, v;\n  wire [3:0] src = 4'b1010;\n  wire floating;\n\
               assign {a, b} = src[1:0];\n  assign #5 {x, y} = src[3:2];\n\
               assign {w[0], v} = 3'b101;\n  assign w[1] = 1'b0;\n\
               wire sink = a ^ b ^ x ^ y ^ w[0] ^ v[1] ^ floating;\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["floating"], "{names:?}");
    }

    #[test]
    fn concatenated_procedural_targets_drive_each_member() {
        let names = undriven_names(
            "module top;\n  logic a, b, c, d, e;\n  logic [1:0] pair;\n  logic floating;\n\
               initial begin\n    {a, b} = 2'b10;\n    {c, {d, e}} = 3'b101;\n    {pair[0], pair[1]} <= 2'b01;\n  end\n\
               initial $display(a, b, c, d, e, pair, floating);\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["floating"], "{names:?}");
    }

    #[test]
    fn streaming_target_drives_each_member() {
        let names = undriven_names(
            "module top;\n  bit [3:0] a, b;\n  bit [7:0] src = 8'h5a;\n  bit [3:0] floating;\n\
               initial begin\n    {>>{a, b}} = src;\n  end\n\
               initial $display(a, b, floating);\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["floating"], "{names:?}");
    }

    #[test]
    fn concatenated_read_is_not_a_driver() {
        let names = undriven_names(
            "module top;\n  wire a, b;\n  wire [1:0] sink;\n  assign sink = {a, b};\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["a", "b"], "a concat on the right only reads");
    }

    #[test]
    fn container_and_string_mutator_receivers_are_drivers() {
        let names = undriven_names(
            "module top;\n  int q[$];\n  int front[$];\n  int mid[$];\n  string s, t;\n  int popped[$];\n\
               class C;\n    int pending[$];\n    function void add(int v); pending.push_back(v); endfunction\n\
                 function int first(); return pending[0]; endfunction\n  endclass\n  C o = new;\n\
               initial begin\n    q.push_back(1);\n    front.push_front(2);\n    mid.insert(0, 3);\n\
                 s.putc(0, \"x\");\n    t.itoa(5);\n    o.add(4);\n\
                 $display(q[0], front[0], mid[0], s, t, o.first(), popped.pop_back());\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["popped"], "popping is not a driver: {names:?}");
    }

    #[test]
    fn mailbox_message_actuals_are_drivers() {
        let names = undriven_names(
            "module top;\n  mailbox #(int) mb = new;\n  int a, b;\n  int ok;\n\
               initial begin\n    mb.get(a);\n    ok = mb.try_get(b);\n    $display(a, b, ok);\n  end\nendmodule\n",
            "top",
        );
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn semaphore_get_count_is_not_a_driver() {
        let names = undriven_names(
            "module top;\n  semaphore sem = new(2);\n  int n;\n\
               initial begin\n    sem.get(n);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["n"], "the key count is an input: {names:?}");
    }

    #[test]
    fn randomize_variable_list_arguments_are_drivers() {
        let names = undriven_names(
            "module top;\n  class C;\n    rand int x;\n    int v;\n    int w;\n  endclass\n  C o = new;\n\
               initial begin\n    void'(o.randomize(v));\n    $display(o.v, o.w);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["w"], "{names:?}");
    }

    #[test]
    fn user_method_named_like_a_container_mutator_is_not_a_driver() {
        let names = undriven_names(
            "module top;\n  class Box;\n    function void insert(int v); endfunction\n  endclass\n  Box h;\n\
               initial h.insert(1);\nendmodule\n",
            "top",
        );
        assert_eq!(
            names,
            ["h"],
            "the call does not assign the handle: {names:?}"
        );
    }

    #[test]
    fn struct_member_writes_drive_the_owning_object() {
        let names = undriven_names(
            "module top;\n  typedef struct { int n; int tags[2]; int q[$]; } node_t;\n\
               typedef struct { node_t inner; int k; } outer_t;\n\
               node_t a, b, c;\n  outer_t o;\n  node_t never;\n  int sink;\n\
               function automatic int local_use();\n    node_t acc;\n    acc.n = 0;\n    acc.tags[1] = 2;\n    return acc.n + acc.tags[1];\n  endfunction\n\
               initial begin\n    a.n = 1;\n    b.tags[0] = 2;\n    c.q.push_back(3);\n    o.inner.n = 4;\n    o.k = 5;\n\
                 sink = a.n + b.tags[0] + c.q[0] + o.inner.n + o.k + local_use() + never.n;\n    $display(sink);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["never"], "{names:?}");
    }

    #[test]
    fn struct_member_write_does_not_drive_a_different_object() {
        let names = undriven_names(
            "module top;\n  typedef struct { int n; } node_t;\n  node_t written, other;\n  int sink;\n\
               initial begin\n    written.n = 1;\n    sink = written.n + other.n;\n    $display(sink);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["other"], "{names:?}");
    }

    #[test]
    fn compound_assignment_in_expression_position_drives_its_target() {
        let names = undriven_names(
            "module top;\n  int step, total, bound, x, never;\n\
               initial begin\n    for (int i = 0; i < 6; i++, step += 2) total = step;\n\
                 x = (bound += 1);\n    $display(step, total, bound, x, never);\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["never"], "{names:?}");
    }

    #[test]
    fn ref_port_actuals_are_drivers() {
        let names = undriven_names(
            "module child(ref logic [3:0] slot);\n  always_comb slot = 4'h5;\nendmodule\n\
             module top;\n  logic [3:0] cells [0:1];\n  logic [3:0] whole;\n  logic [3:0] floating;\n\
               child c0 (.slot(cells[0]));\n  child c1 (.slot(whole));\n\
               initial $display(cells[0], whole, floating);\nendmodule\n",
            "top",
        );
        assert_eq!(names, ["floating"], "{names:?}");
    }
}
