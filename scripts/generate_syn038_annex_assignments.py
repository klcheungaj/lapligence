import importlib.util
import json
import re
from collections import defaultdict, deque
from pathlib import Path

from pypdf import PdfReader


ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("annex", ROOT / "scripts/syn038_annex_inventory.py")
annex = importlib.util.module_from_spec(spec)
spec.loader.exec_module(annex)
ledger = (ROOT / "tests/syn038_coverage_ledger.md").read_text(encoding="utf-8")
addendum = (ROOT / "docs/specification/spec-reference-annex-a.md").read_text(encoding="utf-8")
addendum_rows = annex.inventory(addendum, ledger)
family = defaultdict(set)
for section, name, *_ in addendum_rows:
    family[name].add(section)
addendum_names = set(family)

pdf_sources = defaultdict(set)
pdf_sections = defaultdict(set)
parents = defaultdict(set)
name_pattern = re.compile(r"\b([a-z][a-z_0-9]*(?:,\d+)*)\s*::=")
heading = re.compile(r"^A\.\d+(?:\.\d+)*\s+[A-Z]")
for edition, filename, first, last in [
    ("V2001", "Verilog-1364-2001.pdf", 783, 809),
    ("SV2009", "SystemVerilog-1800-2009.pdf", 1095, 1144),
]:
    reader = PdfReader(ROOT / "docs/specification" / filename)
    sections = []
    current = "A.1"
    for index in range(first, last + 1):
        page = reader.pages[index].extract_text() or ""
        if "Annex B" in page:
            page = page.split("Annex B", 1)[0]
        for line in page.splitlines():
            if heading.match(line):
                current = line.split()[0]
            match = name_pattern.search(line)
            if match:
                name = re.sub(r"\d+(?:,\d+)*$", "", match.group(1))
                pdf_sources[name].add(edition)
                pdf_sections[name].add(f"{edition}:{current}")
        sections.append(page)
    text = "\n".join(sections)
    matches = list(name_pattern.finditer(text))
    if len(matches) != text.count("::="):
        raise ValueError(f"unrecognized Annex A production left-hand side in {filename}")
    names = {re.sub(r"\d+(?:,\d+)*$", "", match.group(1)) for match in matches}
    for index, match in enumerate(matches):
        parent = re.sub(r"\d+(?:,\d+)*$", "", match.group(1))
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        body = text[match.end() : end]
        for child in set(re.findall(r"\b[a-z][a-z_0-9]*\b", body)) & names:
            if child != parent:
                parents[child].add(parent)

core_rows = annex.table_rows(ledger.split("#### SYN-038 audited evidence map", 1)[0], "| SYN038-CORE-")
core = annex.direct_owners([row for row in core_rows if row[5] == "PASS" and row[0] != "SYN038-CORE-PI-03"], 2)
boundary = annex.direct_owners([row for row in core_rows if row[5] != "PASS" or row[0] == "SYN038-CORE-PI-03"], 2)
excluded = annex.direct_owners(annex.table_rows(ledger, "| SYN038-EX-"), 2)
names = addendum_names | set(pdf_sources)
pdf_name_count = len(pdf_sources)

exclusive_excluded = {"B.5", "B.11", "B.12", "B.13", "B.14", "B.15", "B.16", "B.20", "B.25", "B.26", "B.27", "B.29", "B.30"}
excluded_patterns = [
    (r"^(?:class_|ps_class_|constraint_|randomize_|randcase_|randsequence_|rs_|covergroup_|cover_point|cover_cross|coverage_|bins_|bin_identifier|cross_|select_bins_|select_condition|select_expression|weight_specification|solve_before_|sequence_|property_|assert_|assume_|cover_property|cover_sequence|restrict_property|checker_|clocking_|clockvar|cycle_delay|dpi_|vcd_|sdf_|specify_|specparam_|timing_check|system_timing_check|.*_timing_check|.*_path_.*|path_delay|pulse_control|pulsestyle|showcancelled|notifier|timestamp_condition|timecheck_condition|delayed_reference|delayed_data|start_edge_offset|end_edge_offset|event_based_flag|remain_active_flag|reject_limit_value|error_limit_value)$", "SYN-034 verification/timing/foreign profile exclusion"),
    (r"^(?:dynamic_array_|associative_dimension|queue_dimension|empty_queue|unsized_dimension|list_of_virtual_interface_decl|virtual_interface_declaration)$", "SYN-034 dynamic/native profile exclusion"),
    (r"^(?:overload_|dist_|expression_or_dist|method_|built_in_method_call|array_manipulation_call|array_method_call|array_method_name)$", "SYN-034 overload/distribution or unbounded method profile exclusion"),
    (r"^(?:event_trigger|event_declaration|event_identifier|list_of_event_identifiers|par_block|join_keyword|final_construct|procedural_continuous_assignment|procedural_continuous_assignments)$", "SYN-034 finite RTL process/profile exclusion"),
    (r"^(?:bind_|config_|library_|liblist_|cell_clause|cell_identifier|use_clause|design_statement|inst_clause|inst_name|topmodule_identifier)$", "SYN-032/SYN-033 selected Extended design assembly; outside Core"),
    (r"^(?:mos_|cmos_|pass_|pass_en_|pass_enable_|charge_strength|pull_gate_instance)$", "SYN-034 switch/charge profile exclusion"),
    (r"^(?:system_|file_|display_|strobe_|monitor_|dump|dumpports|vcdclose|pla_|finish_|stop_|simulation_|severity_|fatal_|nonfatal_|timeformat_|printtimescale_|random_function|dist_functions|elaboration_system_task)$", "SYN-035/host/diagnostic companion outside Core RTL"),
    (r"^(?:tagged_union_expression|cond_pattern|expression_or_cond_pattern|case_pattern_item|pattern)$", "SYN-021–025 selected Extended tagged/pattern profile; outside Core"),
]

forced_alias = {
    "assignment_operator": "operator_assignment",
    "block_item_declaration": "seq_block",
    "default_nettype_value": "default_nettype_compiler_directive",
    "endkeywords_directive": "keywords_directive",
    "formal_identifier": "identifier",
    "ifdef_directive": "conditional_compilation_directive",
    "identifier_list": "identifier",
    "ifndef_directive": "conditional_compilation_directive",
    "inout_port_identifier": "identifier",
    "input_port_identifier": "identifier",
    "interface_or_generate_item": "interface_item",
    "item_name": "hierarchical_identifier",
    "list_of_actual_arguments": "actual_argument",
    "list_of_interface_identifiers": "interface_identifier",
    "list_of_tf_variable_identifiers": "tf_port_declaration",
    "memory_identifier": "identifier",
    "module_common_item": "module_item",
    "module_or_assertion": "module_declaration",
    "module_or_variable": "module_declaration",
    "output_port_identifier": "identifier",
    "port": "list_of_ports",
    "port_value": "ordered_port_connection",
    "ps_identifier": "package_scope",
    "ps_parameter_identifier": "package_scope",
    "ps_type_identifier": "package_scope",
    "pragma_keyword": "pragma",
    "pragma_name": "pragma",
    "pragma_value": "pragma_expression",
    "real_numbera": "real_number",
    "scalar_constant": "integral_number",
    "source_text": "module_declaration",
    "strength_component": "drive_strength",
    "task_port_type": "tf_input_declaration",
    "text_macro_identifier": "identifier",
    "text_macro_name": "text_macro_definition",
    "tf_port_direction": "task_port_item",
    "undefine_compiler_directive": "conditional_compilation_directive",
    "var_type": "variable_type",
}
forced_excluded = {
    "procedural_continuous_assignment": "SYN038-EX-04: ADV-001 unsupported by design; every procedural assign/deassign stops the simulator with the ADV-032 diagnostic",
    "procedural_continuous_assignment(s)": "SYN038-EX-04: ADV-001 unsupported by design; every procedural assign/deassign stops the simulator with the ADV-032 diagnostic",
    "procedural_continuous_assignments": "SYN038-EX-04: ADV-001 unsupported by design; every procedural assign/deassign stops the simulator with the ADV-032 diagnostic",
    "assertion_variable_declaration": "SYN-034 assertion verification profile",
    "block_event_expression": "SYN-034 covergroup event profile",
    "class_constraint": "SYN-034 class/constraint verification profile",
    "constant_mintypmax_expression": "SYN-034 min/typ/max timing profile exclusion",
    "const_or_range_expression": "SYN-034 assertion range profile",
    "cycle_delay_const_range_expression": "SYN-034 assertion/clocking cycle-delay profile",
    "cycle_delay_range": "SYN-034 assertion/clocking cycle-delay profile",
    "data_source_expression": "SYN-034 specify path profile",
    "default_clause": "SYN-032 Extended library configuration profile",
    "deferred_immediate_assertion_item": "SYN-034 assertion verification profile",
    "delay": "Simulator timing companion; dynamic delays outside finite Core RTL target",
    "delay_control": "Simulator timing companion outside Core RTL; event controls retain Core PR-02",
    "delay_or_event_control": "Simulator intra-assignment timing companion outside Core RTL",
    "delay_value": "Simulator timing companion outside Core RTL",
    "dist_item": "SYN-034 randomization/distribution profile",
    "dist_list": "SYN-034 randomization/distribution profile",
    "dist_weight": "SYN-034 randomization/distribution profile",
    "final_simulation_time": "VCD file-format/host simulation companion outside Core RTL",
    "expect_property_statement": "SYN-034 assertion verification profile",
    "finish_number": "Host simulation-control companion outside Core RTL",
    "import_export": "SYN-034 foreign import/export profile",
    "input_identifier": "SYN-034 specify input-terminal descriptor profile",
    "include_statement": "SYN-032 Extended library-map include profile",
    "limit_value": "SYN-034 specify/timing-check profile",
    "load_memory_tasks": "SYN-029 Extended memory-image initialization profile",
    "list_of_real_identifiers": "SYN-034 runtime real storage target selection",
    "list_of_specparam_assignments": "SYN-034 specify/timing-path profile exclusion",
    "modport_tf_port": "SYN-034 interface task/function modport target selection",
    "modport_tf_ports_declaration": "SYN-034 interface task/function modport target selection",
    "mintypmax_expression": "SYN-034 min/typ/max timing profile exclusion",
    "notify_reg": "SYN-034 specify/timing-check profile",
    "output_identifier": "SYN-034 specify output-terminal descriptor profile",
    "overload_proto_formals": "SYN-034 operator-overload profile",
    "production": "SYN-034 random-sequence verification profile",
    "production_identifier": "SYN-034 random-sequence verification profile",
    "production_item": "SYN-034 random-sequence verification profile",
    "ps_class_identifier": "SYN-034 class verification profile",
    "randcase_item": "SYN-034 randomization verification profile",
    "range_list": "SYN-034 covergroup range profile",
    "real_identifier": "SYN-034 runtime real storage target selection; constant real values retain Core LX-03",
    "real_declaration": "SYN-034 runtime real storage target selection; constant real parameters retain Core LX-03",
    "realtime_declaration": "SYN-034 runtime real-time storage target selection",
    "realtime_function": "Host simulation-time reporting companion outside Core RTL",
    "repeat_range": "SYN-034 assertion sequence verification profile",
    "sdf_annotate_task": "SDF back-annotation outside Core RTL",
    "signal_identifier": "SYN-034 random-sequence verification profile",
    "simple_immediate_cover_statement": "SYN-034 assertion coverage verification profile",
    "simulation_time": "Host/VCD simulation-time reporting companion outside Core RTL",
    "terminal_identifier": "SYN-034 specify delayed-terminal profile",
    "time_declaration": "Simulator time-storage companion outside Core RTL",
    "stime_function": "Host simulation-time reporting companion outside Core RTL",
    "time_function": "Host simulation-time reporting companion outside Core RTL",
    "typename_function": "SYN-034 type-name reflection target selection; fixed dimension queries retain Core TY-12",
    "upward_name_reference": "SYN-034 upward module-name resolution target selection; ordinary hierarchy retains Core HY-06",
    "variable_identifier_list": "SYN-034 randomization verification profile",
    "writemem_tasks": "SYN-030 Extended memory-image output profile",
    "zero_or_one": "SYN-034 timing-check profile boundary (Annex A.7.5.3); not a UDP table production",
    "z_or_x": "SYN-034 specify timing-condition profile",
}

extended_evidence = {}


def assign_extended(productions, task, fixture, owner, scope):
    for production in productions:
        if production not in names:
            raise ValueError(f"unknown Extended production {production}")
        extended_evidence[production] = {
            "task": task,
            "fixture": fixture,
            "owner": owner,
            "scope": scope,
        }


assign_extended(
    {
        "combinational_body", "combinational_entry", "input_value",
        "level_input_list", "level_symbol", "list_of_udp_port_identifiers",
        "name_of_udp_instance", "output_symbol", "output_value",
        "udp_body", "udp_declaration", "udp_identifier",
        "udp_input_declaration", "udp_instance", "udp_instance_identifier",
        "udp_instantiation", "udp_nonansi_declaration",
        "udp_output_declaration", "udp_port_declaration", "udp_port_list",
    },
    "SYN-031",
    "tests/fixtures/sim/syn031_combinational_udp/syn_031_combinational_udp.v",
    "tests/sim_udp.rs::syn_031_combinational_udp_matrix_both_editions",
    "scalar combinational UDP table and instance",
)
assign_extended(
    {"udp_ansi_declaration", "udp_declaration_port_list"},
    "SYN-031",
    "tests/fixtures/sim/review_bundle/r12_udp_ansi.sv",
    "tests/sim_syn038_ledger.rs::ansi_combinational_udp_uses_declared_scalar_ports",
    "SystemVerilog ANSI scalar combinational UDP",
)
assign_extended(
    {
        "library_declaration", "library_description", "library_descriptions",
        "library_identifier", "library_text",
    },
    "SYN-032",
    "tests/fixtures/sim/syn032_library_configs/incdir.map",
    "tests/sim_syn032_library_configs.rs::library_map_incdirs_select_scoped_headers_in_both_editions",
    "bounded library map with scoped ordered -incdir",
)
assign_extended(
    {
        "cell_clause", "cell_identifier", "config_declaration", "default_clause",
        "config_identifier", "config_rule_statement", "design_statement",
        "inst_clause", "inst_name", "liblist_clause", "library_cell",
        "topmodule_identifier", "use_clause",
    },
    "SYN-032",
    "tests/fixtures/sim/syn032_library_configs/choose_gate.map",
    "tests/sim_syn032_library_configs.rs::filesystem_map_binding_changes_selected_composition",
    "bounded configuration cell and instance selection",
)
assign_extended(
    {"include_statement"},
    "SYN-032",
    "tests/fixtures/sim/syn032_library_configs/root.map",
    "tests/sim_syn032_library_configs.rs::configured_libraries_execute_in_both_editions_and_optimizer_modes",
    "bounded library-map include",
)
assign_extended(
    {
        "bind_directive", "bind_instantiation", "bind_target_instance",
        "bind_target_instance(_list)", "bind_target_instance_list",
        "bind_target_scope",
    },
    "SYN-033",
    "tests/fixtures/sim/syn033_structural_bind/syn_033_structural_bind.sv",
    "tests/sim_syn033_structural_bind.rs::module_and_instance_bind_execute_in_both_optimizer_modes",
    "module and selected instance structural bind",
)
assign_extended(
    {"tagged_union_expression", "cond_pattern", "expression_or_cond_pattern", "pattern"},
    "SYN-024",
    "tests/fixtures/sim/syn024_tagged_patterns/tagged_runtime.sv",
    "tests/sim_syn024_tagged_patterns.rs::tagged_bindings_nested_payloads_and_conditional_arms",
    "finite packed tagged construction and conditional pattern",
)
assign_extended(
    {"case_pattern_item"},
    "SYN-025",
    "tests/fixtures/sim/syn025_pattern_cases/runtime_modes.sv",
    "tests/sim_syn025_pattern_cases.rs::runtime_selectors_and_pattern_side_wildcards_follow_case_mode",
    "finite fixed-value pattern case",
)
assign_extended(
    {
        "array_manipulation_call", "array_method_call",
        "array_method_name", "built_in_method_call", "method_call",
        "method_call_body", "method_call_root",
    },
    "SYN-026",
    "tests/fixtures/sim/syn026_iterator_indices/indices.sv",
    "tests/sim_syn026_iterator_indices.rs::fixed_array_reduction_and_ordering_indices_follow_declared_coordinates",
    "fixed-array reduction and ordering methods; locator results remain excluded",
)
assign_extended(
    {"load_memory_tasks"},
    "SYN-029",
    "tests/fixtures/sim/memory_editions/default_order.sv",
    "tests/sim_memory_editions.rs::omitted_memory_range_keeps_the_selected_edition_order",
    "bounded memory-image load",
)
assign_extended(
    {"writemem_tasks"},
    "SYN-030",
    "tests/fixtures/sim/memory_views/writer_round_trip.sv",
    "tests/sim_memory_views.rs::reversed_negative_ranges_and_writer_round_trip_keep_row_order",
    "bounded memory-image output",
)

outside_core_implemented = {}


def assign_outside(productions, owner, docs_heading, profile, fixture=None):
    for production in productions:
        if production not in names:
            raise ValueError(f"unknown outside-Core production {production}")
        evidence = {
            "owner": owner,
            "docs_heading": docs_heading,
            "profile": profile,
        }
        if fixture is not None:
            evidence["fixture"] = fixture
        else:
            evidence["in_memory_source"] = True
        outside_core_implemented[production] = evidence


assign_outside(
    {"event_declaration", "event_identifier", "list_of_event_identifiers", "event_trigger"},
    "tests/sim_events.rs::sim_events_handshake",
    "**Named-event operations**",
    "outside Core testbench synchronization; old group 31",
)
assign_outside(
    {"hierarchical_event_identifier"},
    "tests/sim_events.rs::sim_events_hierarchical_reference",
    "**Named-event operations**",
    "outside Core named-event hierarchy; old group 31",
)
assign_outside(
    {"virtual_interface_declaration", "list_of_virtual_interface_decl"},
    "tests/sim_virtual_interfaces.rs::virtual_interface_rebinding_and_views_match_across_optimizer_modes",
    "**Virtual interfaces**",
    "outside Core runtime-rebound virtual handles; old group 72",
    "tests/fixtures/sim/virtual_interfaces/rebind_class.sv",
)
assign_outside(
    {"class_declaration"},
    "tests/sim_classes.rs::class_objects_constructors_methods_and_aliases_match_across_optimizer_modes",
    "**Classes**",
    "SYN-034(1) excludes static/elaboration class uses from Core; bounded runtime classes are simulator companions",
    "tests/fixtures/sim/classes/basic.sv",
)
assign_outside(
    {"program_declaration"},
    "tests/sim_program.rs::program_reactive_nba_and_zero_delay_ordering_match_module_active",
    "**Programs**",
    "outside Core reactive testbench container",
    "tests/fixtures/sim/program_blocks/program_basic.sv",
)
assign_outside(
    {"clocking_declaration"},
    "tests/sim_partial_features/clocking.rs::clocking_input_skews_sample_preponed_observed_and_history_values",
    "**Clocking**",
    "outside Core sampled testbench timing",
    "tests/fixtures/sim/partial_features/clocking_h13.sv",
)
assign_outside(
    {"assertion_item"},
    "tests/sim_concurrent_assertions.rs::concurrent_assertions_sample_before_nba_updates",
    "**Concurrent assertions**",
    "SYN-034(6) excludes finite assertion synthesis; bounded simulation checking is implemented",
    "tests/fixtures/sim/concurrent_assertions/sampling_nba.sv",
)
assign_outside(
    {"dynamic_array_variable_identifier", "dynamic_array_new"},
    "tests/sim_data_types_next.rs::dynamic_array_allocate_copy_resize_and_delete",
    "**Dynamic arrays, associative arrays and queues**",
    "outside Core resizable/native storage",
    "tests/fixtures/sim/data_types_next/dynamic_array.sv",
)
assign_outside(
    {"associative_dimension"},
    "tests/sim_data_types_next.rs::associative_array_insert_traverse_and_delete",
    "**Dynamic arrays, associative arrays and queues**",
    "outside Core key-indexed native storage",
    "tests/fixtures/sim/data_types_next/associative_array.sv",
)
assign_outside(
    {"queue_dimension"},
    "tests/sim_virtual_interfaces.rs::queue_virtual_interface_array_rebinds_elements",
    "**Dynamic arrays, associative arrays and queues**",
    "outside Core resizable queue storage",
    "tests/fixtures/sim/virtual_interfaces/queue.sv",
)

unsupported_udp = {
    "current_state", "edge_indicator", "edge_input_list", "edge_symbol",
    "init_val", "next_state", "seq_input_list", "sequential_body",
    "sequential_entry", "udp_initial_statement", "udp_reg_declaration",
}

def exclusion(name):
    if name in unsupported_udp:
        return "SYN-031 sequential/edge/state-initialization UDP explicitly rejected; outside Core and the selected Extended combinational subset"
    if name in forced_excluded:
        return forced_excluded[name]
    if name.startswith(("bind_",)):
        return "SYN-033 selected Extended structural-bind profile; outside Core"
    if name.startswith(("config_", "library_", "liblist_", "cell_", "inst_clause", "inst_name")):
        return "SYN-032 selected Extended library/configuration profile; outside Core"
    if name.startswith(("mos_", "cmos_", "pass_", "ncontrol_", "pcontrol_")) or name == "inout_terminal":
        return "SYN-034 switch/device profile exclusion"
    if name.startswith(("specparam_", "specify_", "module_path_", "path_delay", "pulse_control", "pulsestyle_", "showcancelled_", "timing_check", "scalar_timing_check", "controlled_timing_check", "state_dependent_path", "edge_sensitive_path", "parallel_path", "full_path", "t_path_delay", "trise_path", "tfall_path", "tz_path", "tx", "t01_path", "t10_path")):
        return "SYN-034 specify/timing-path profile exclusion"
    if name.startswith(("ps_covergroup_", "ps_class_", "ps_or_hierarchical_property_", "ps_or_hierarchical_sequence_")):
        return "SYN-034 verification container profile exclusion"
    if name in {"action_block", "hierarchical_event_identifier", "modport_clocking_declaration", "function_prototype", "task_prototype", "dynamic_array_new"}:
        return "SYN-034 verification, native container or prototype target exclusion"
    if name.startswith(("display_", "dump", "file_", "monitor_", "printtimescale_", "severity_", "fatal_", "nonfatal_", "simulation_control_", "stop_", "string_output_", "strobe_", "system_", "timeformat_", "finish_")):
        return "Host diagnostics, file/VCD I/O or simulation-control companion outside Core RTL"
    if name.startswith(("assertion_", "deferred_immediate_", "simple_immediate_", "immediate_assertion_", "procedural_assertion_", "class_", "clocking_", "coverage_", "covergroup_", "constraint_", "dpi_", "vcd_")):
        return "SYN-034 verification/foreign profile exclusion"
    if name in excluded:
        return ", ".join(sorted(excluded[name])) + " selected-profile exclusion"
    for expression, reason in excluded_patterns:
        if re.search(expression, name):
            return reason
    sections = family.get(name, set())
    if sections and sections <= exclusive_excluded:
        if "B.25" in sections:
            return "SYN-031 Extended UDP profile; outside Core"
        if "B.27" in sections:
            return "SYN-032 Extended configuration profile; outside Core"
        return "SYN-034 verification, timing, foreign or non-RTL profile exclusion"
    return None


assignments = {}
for name in sorted(names):
    entry = {
        "production": name,
        "editions": sorted(pdf_sources.get(name, ())),
        "annex_sections": sorted(pdf_sections.get(name, ())),
        "reference_families": sorted(family.get(name, ()), key=lambda x: int(x.split(".")[1])),
        "sources": sorted([f"{edition}_PDF" for edition in pdf_sources.get(name, ())] + (["ADDENDUM"] if name in addendum_names else [])),
    }
    if name in core:
        entry.update(disposition="CORE", ledger_rows=sorted(core[name]))
    elif name in extended_evidence:
        entry.update(disposition="EXTENDED", evidence=extended_evidence[name])
    elif name in outside_core_implemented:
        entry.update(disposition="OUTSIDE_CORE_IMPLEMENTED", evidence=outside_core_implemented[name])
    elif name in unsupported_udp:
        entry.update(disposition="EXCLUDED", reason=exclusion(name))
    elif name in boundary:
        entry.update(disposition="BOUNDARY", ledger_rows=sorted(boundary[name]))
    elif reason := exclusion(name):
        entry.update(disposition="EXCLUDED", reason=reason)
    else:
        entry.update(disposition="PENDING")
    assignments[name] = entry

    if not entry["editions"]:
        sv_only = bool(family.get(name, set()) & {"B.3", "B.4", "B.5", "B.11", "B.12", "B.13", "B.14", "B.15", "B.16", "B.20", "B.21", "B.22"})
        sv_only |= name.startswith(("bind_", "endkeywords_", "fatal_", "severity_", "nonfatal_", "typename_"))
        entry["editions"] = ["SV2009"] if sv_only else ["V2001", "SV2009"]
        entry["edition_basis"] = "reference-family inference; absent as a PDF left-hand side"

selected_anchors = {name for name, entry in assignments.items() if entry["disposition"] == "CORE"}

def path_to_core(name):
    queue = deque((parent, [parent]) for parent in sorted(parents[name]))
    seen = {name}
    while queue:
        parent, path = queue.popleft()
        if parent in seen or parent not in assignments:
            continue
        seen.add(parent)
        if parent in selected_anchors:
            return path
        if assignments[parent]["disposition"] == "EXCLUDED":
            continue
        queue.extend((next_parent, path + [next_parent]) for next_parent in sorted(parents[parent]))
    return None


for name in sorted(names):
    entry = assignments[name]
    if entry["disposition"] != "PENDING":
        continue
    if name in forced_alias:
        entry.update(disposition="ALIAS/HELPER", parents=[forced_alias[name]])
        continue
    path = path_to_core(name)
    if path:
        entry.update(disposition="ALIAS/HELPER", parents=[path[0]])
    elif not pdf_sources.get(name):
        root_name = re.sub(r"\([^)]*\)$", "", name)
        if root_name in assignments and root_name != name:
            entry.update(disposition="ALIAS/HELPER", parents=[root_name])
    if entry["disposition"] == "PENDING":
        entry.update(disposition="OPEN", child="SYN-038-N13", reason="Annex production has no verified Core witness or profile exclusion")

source_names = {
    source: sorted(name for name, entry in assignments.items() if source in entry["sources"])
    for source in ("V2001_PDF", "SV2009_PDF", "ADDENDUM")
}
output = {
    "schema": "syn038-annex-assignments/v2",
    "extraction": {
        "pdf_distinct": pdf_name_count,
        "addendum_distinct": len(addendum_names),
        "pdf_only": len(set(pdf_sources) - addendum_names),
        "addendum_only": len(addendum_names - set(pdf_sources)),
        "union_distinct": len(names),
    },
    "source_names": source_names,
    "assignments": list(assignments.values()),
}
lines = ["{", f'  "schema": {json.dumps(output["schema"])},', f'  "extraction": {json.dumps(output["extraction"], sort_keys=True)},', '  "source_names": {']
for index, (source, source_list) in enumerate(source_names.items()):
    comma = "," if index < len(source_names) - 1 else ""
    lines.append(f'    {json.dumps(source)}: {json.dumps(source_list)}{comma}')
lines += ['  },', '  "assignments": [']
for index, entry in enumerate(output["assignments"]):
    comma = "," if index < len(output["assignments"]) - 1 else ""
    lines.append(f"    {json.dumps(entry, sort_keys=True)}{comma}")
lines += ["  ]", "}"]
(ROOT / "tests/syn038_annex_assignments.json").write_text("\n".join(lines) + "\n", encoding="utf-8")
from collections import Counter
print(len(names), Counter(x["disposition"] for x in assignments.values()))
print("OPEN", [x for x in sorted(names) if assignments[x]["disposition"] == "OPEN"])
