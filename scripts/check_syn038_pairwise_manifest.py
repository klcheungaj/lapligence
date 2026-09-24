from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from itertools import combinations, product
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "tests/sim_syn038_pairwise_manifest.json"
SCHEMA = "lapligence.syn038.selected-pairwise/v4"

FACTORS = {
    "TY": ("integral_bit_logic", "enum", "packed_struct", "untagged_packed_union", "fixed_array_integral", "fixed_array_record", "unpacked_record", "tagged_extended"),
    "OP": ("direct_projection", "conditional", "equality_inside", "cast_stream", "assignment_pattern"),
    "CO": ("assignment_rhs", "port_actual", "call_argument", "function_return_statement", "declaration_initializer", "constant_elaboration", "event_expression"),
    "LV": ("none", "whole_object", "field", "element", "row_slice", "concatenation", "positional_pattern"),
    "SL": ("module_package", "static_local", "automatic_local", "formal", "return_slot", "interface_member"),
    "FM": ("none", "input", "output", "inout", "ref", "const_ref"),
    "HC": ("module", "subroutine", "generate", "interface"),
    "HR": ("local", "child_port", "hierarchical_identifier", "interface_member"),
    "CP": ("none", "function", "fixed_array_reduction"),
    "CT": ("none", "function", "task"),
    "IN": ("none", "constant_declaration", "automatic_local", "static_local", "runtime_declaration", "memory_image"),
    "WK": ("none", "continuous_net", "continuous_variable", "procedural_blocking", "procedural_nba"),
    "PC": ("none", "initial", "always", "always_comb", "always_latch", "always_ff"),
}

SEARCH_ORDER = ("CO", "IN", "HC", "WK", "PC", "LV", "SL", "FM", "CT", "CP", "TY", "OP", "HR")

FACTOR_MEANINGS = {
    "TY": "Outer declared type of the focal typed slot before member or element projection; leaf state and width are qualifiers.",
    "OP": "Selected syntax operation on a focal source or direct address projection of a focal target; an unselected arithmetic RHS does not change a target's direct_projection address. Operation result type can differ from source TY, as equality/inside yields one bit.",
    "CO": "Selected Core use site consuming the focal value or its operation result, possibly after a proven write to that same outer slot. call_argument is a user-defined task/function actual; system-task oracle arguments such as $display are outside that lane. CO never identifies an expression operator or write kind.",
    "LV": "Address form when the focal slot itself is a write target, including an earlier write on a selected source/use path; none means no selected write to the focal slot.",
    "SL": "Storage of the focal source or target slot, including leaves of a composite pattern target; interface_member means a mutable interface net or variable, not a parameter; independent of a receiving formal.",
    "FM": "Direction of the formal receiving a port actual or subroutine argument; independent of SL, the actual's storage.",
    "HC": "Lexical scope of the focal CO consumer when CO is present; otherwise the focal write site. Independent of a different writer's scope and the referenced target route.",
    "HR": "Selected route from HC, in priority order: an explicitly qualified module or procedural hierarchy identifier or callee, including $root, takes hierarchical_identifier; otherwise a concrete interface data member route such as bus.member or nested.member takes interface_member; otherwise an instantiated child port link takes child_port; remaining local and package-scope :: access takes local.",
    "CP": "Source-side function result or fixed-array reduction on the focal array receiver; reduction does not write that receiver.",
    "CT": "Receiving user-defined subroutine kind for a Core call argument; system-task oracles are outside this lane, and tasks have no expression return.",
    "IN": "Kind of initializer at the consumer site: constant_declaration is a parameter/localparam, runtime_declaration is a module/interface variable declaration assignment, and memory_image is a separate file-backed Extended lane.",
    "WK": "Explicit assignment form writing the focal slot, possibly before its selected CO consumer; continuous_net and continuous_variable mean static continuous assignment statements outside a procedure, while none covers paths without a selected explicit write and implicit port/call copy-out.",
    "PC": "With CO present, process executing the focal consumer use site, including synchronous zero-time helper calls; none for static continuous, port-link, static declaration-initialization and compile-time use sites. Without CO, process executing the focal write. A same-slot writer may execute in a different process.",
}

PATH_BASIS = {
    "assignment_rhs": "SYN-038 context matrix destination/driver rows; SYN-002–010, SYN-012, SYN-014 and SYN-015.",
    "port_actual": "SYN-038 input/output link and hierarchy rows; SYN-007–010 and SYN-018.",
    "call_argument": "SYN-038 selected user-defined task/function call/storage rows; SYN-011, SYN-013 and hierarchical task/function row HY-06. System-task oracle arguments are outside this synthesizable Core lane.",
    "function_return_statement": "SYN-038 expression/call/storage rows; SYN-001, SYN-012 and SYN-013.",
    "declaration_initializer": "SYN-038 initializer and storage rows; SYN-012, SYN-016 and SYN-019.",
    "constant_elaboration": "SYN-038 constant-expression and hierarchy rows; SYN-016 and SYN-019.",
    "event_expression": "SYN-038 values/effects and process rows; SYN-011 and SYN-014.",
}

OUTSIDE_BASIS = {
    "SYN038-OOS-EXT-TAGGED": "Tagged fixed values are the SYN-021 Extended lane, outside the selected Core TY levels (plan Section 6 and SYN-021).",
    "SYN038-OOS-EXT-MEMORY-IMAGE": "File-backed $readmemh/$readmemb loading belongs to SYN-029/SYN-030 Extended-initialization, not a Core declaration initializer.",
}

REQUIRED_SELECTED = (
    ("TY", "fixed_array_integral", "CO", "call_argument"),
    ("TY", "fixed_array_record", "CO", "port_actual"),
    ("TY", "unpacked_record", "CO", "port_actual"),
    ("TY", "packed_struct", "LV", "field"),
    ("TY", "packed_struct", "CO", "port_actual"),
    ("TY", "packed_struct", "SL", "interface_member"),
    ("TY", "packed_struct", "FM", "output"),
    ("TY", "packed_struct", "HR", "child_port"),
    ("LV", "field", "PC", "initial"),
    ("LV", "row_slice", "WK", "procedural_blocking"),
    ("CT", "function", "PC", "initial"),
    ("CT", "task", "PC", "initial"),
    ("CO", "event_expression", "CP", "none"),
    ("SL", "interface_member", "HC", "module"),
    ("TY", "fixed_array_integral", "SL", "automatic_local"),
    ("TY", "fixed_array_integral", "IN", "automatic_local"),
    ("TY", "fixed_array_record", "CP", "fixed_array_reduction"),
    ("SL", "formal", "WK", "continuous_variable"),
    ("SL", "static_local", "WK", "continuous_variable"),
    ("SL", "return_slot", "WK", "continuous_variable"),
    ("FM", "ref", "HR", "child_port"),
    ("CO", "event_expression", "LV", "whole_object"),
    ("CO", "event_expression", "WK", "procedural_blocking"),
    ("CO", "event_expression", "LV", "row_slice"),
    ("LV", "row_slice", "CP", "fixed_array_reduction"),
    ("CP", "fixed_array_reduction", "WK", "continuous_net"),
    ("CP", "fixed_array_reduction", "WK", "procedural_nba"),
    ("CO", "event_expression", "WK", "procedural_nba"),
    ("CO", "event_expression", "WK", "continuous_net"),
    ("WK", "continuous_variable", "PC", "always_latch"),
    ("CO", "call_argument", "WK", "procedural_nba"),
    ("CT", "task", "WK", "continuous_net"),
    ("CO", "call_argument", "WK", "procedural_blocking"),
    ("CT", "function", "WK", "procedural_blocking"),
    ("CT", "task", "WK", "procedural_blocking"),
    ("CO", "function_return_statement", "WK", "procedural_blocking"),
    ("IN", "constant_declaration", "WK", "procedural_blocking"),
    ("WK", "procedural_blocking", "PC", "none"),
    ("CO", "port_actual", "WK", "procedural_nba"),
    ("HR", "child_port", "WK", "continuous_variable"),
    ("CT", "function", "WK", "procedural_nba"),
    ("LV", "field", "FM", "const_ref"),
    ("FM", "const_ref", "WK", "procedural_blocking"),
    ("CO", "function_return_statement", "WK", "continuous_net"),
    ("CO", "function_return_statement", "WK", "procedural_nba"),
    ("LV", "concatenation", "FM", "input"),
    ("LV", "concatenation", "FM", "const_ref"),
    ("LV", "concatenation", "FM", "ref"),
    ("LV", "positional_pattern", "FM", "ref"),
    ("FM", "ref", "WK", "procedural_blocking"),
    ("FM", "ref", "WK", "procedural_nba"),
    ("FM", "inout", "WK", "procedural_blocking"),
    ("FM", "inout", "WK", "procedural_nba"),
    ("FM", "const_ref", "WK", "continuous_variable"),
    ("FM", "ref", "WK", "continuous_variable"),
    ("LV", "whole_object", "FM", "const_ref"),
    ("LV", "element", "FM", "const_ref"),
    ("LV", "row_slice", "FM", "const_ref"),
    ("LV", "positional_pattern", "FM", "const_ref"),
    ("FM", "const_ref", "WK", "procedural_nba"),
    ("LV", "whole_object", "FM", "input"),
    ("LV", "field", "FM", "input"),
    ("LV", "element", "FM", "input"),
    ("LV", "row_slice", "FM", "input"),
    ("LV", "positional_pattern", "FM", "input"),
    ("LV", "whole_object", "CP", "fixed_array_reduction"),
    ("LV", "field", "CP", "fixed_array_reduction"),
    ("LV", "element", "CP", "fixed_array_reduction"),
    ("LV", "concatenation", "CP", "fixed_array_reduction"),
    ("LV", "positional_pattern", "CP", "fixed_array_reduction"),
    ("CP", "fixed_array_reduction", "WK", "continuous_variable"),
    ("CP", "fixed_array_reduction", "WK", "procedural_blocking"),
)

REQUIRED_IMPOSSIBLE = (
    ("CO", "declaration_initializer", "IN", "none"),
    ("SL", "automatic_local", "WK", "procedural_nba"),
    ("TY", "integral_bit_logic", "LV", "field"),
    ("TY", "enum", "LV", "field"),
    ("TY", "fixed_array_integral", "LV", "field"),
    ("CO", "port_actual", "IN", "memory_image"),
    ("CO", "constant_elaboration", "SL", "interface_member"),
    ("SL", "interface_member", "IN", "constant_declaration"),
    ("HR", "interface_member", "IN", "constant_declaration"),
    ("CO", "constant_elaboration", "HR", "hierarchical_identifier"),
    ("HR", "hierarchical_identifier", "IN", "constant_declaration"),
    ("CO", "call_argument", "HR", "child_port"),
    ("CO", "event_expression", "HR", "child_port"),
    ("CT", "task", "PC", "none"),
    ("IN", "constant_declaration", "PC", "always_ff"),
    ("IN", "runtime_declaration", "PC", "always_comb"),
    ("FM", "const_ref", "HR", "child_port"),
    ("FM", "output", "CP", "function"),
    ("FM", "inout", "CP", "function"),
    ("FM", "ref", "CP", "function"),
    ("FM", "const_ref", "CP", "function"),
    ("FM", "const_ref", "CP", "fixed_array_reduction"),
    ("HC", "subroutine", "IN", "runtime_declaration"),
    ("SL", "static_local", "HR", "child_port"),
    ("FM", "inout", "WK", "continuous_variable"),
)

REQUIRED_OUTSIDE = (
    ("TY", "tagged_extended", "CO", "assignment_rhs"),
    ("TY", "fixed_array_integral", "IN", "memory_image"),
)

RULE_BASIS = {
    "SYN038-R-RUNTIME-DECL-SCOPE": "A runtime_declaration initializer is lexically a module/interface variable declaration, so its same focal use site cannot have HC subroutine. A function call supplying the initializer does not move that declaration into its callee; a subroutine-local variable initializer instead has IN automatic_local or static_local. IEEE 1800-2009 6.8/6.21 and Section 6 initializer/scope rows; W82 O379 and SV2009 module-call versus function-local near-miss probes pass in both CLI modes.",
    "SYN038-R-INITIALIZER-SITE": "One focal use site is a declaration initializer exactly when its IN kind is a Core declaration-initializer kind; plan Section 6 initializer row and SYN-016/SYN-019.",
    "SYN038-R-CALL-TARGET": "A receiving function/task target is present exactly for a call argument; plan Section 6 call row and SYN-013.",
    "SYN038-R-TASK-PROCESS": "A task call is a procedural statement and executes under an initial/always-family process, including zero-time helper calls; IEEE 1800-2009 9.2/13.3 and SYN-013/Section 6 call and process rows.",
    "SYN038-R-FORMAL-SITE": "Receiving formal direction exists only for a port actual or call argument; plan Section 6 storage/formal and input/output-link rows.",
    "SYN038-R-PORT-DIRECTION": "An instance port actual may receive input, output, inout, or ref direction; a ref module port connects an equivalent variable type, while const-ref is a subroutine formal mode rather than a module port direction. IEEE 1800-2009 23.3.3.2 and SYN-007–010 port-link contexts.",
    "SYN038-R-PORT-ROUTE": "A port actual is bound at elaboration outside a subroutine or active process (consumer PC none). An input actual can read the same focal slot previously written by a separate continuous or procedural driver; HR selects its access/link route. IEEE 1800-2009 23.3.3; SYN-007–010 and SV2009 source-bound port-driver probes.",
    "SYN038-R-PORT-LINK-ROUTE": "Every instance port actual has a structural child link unless a qualified identifier or concrete interface data member is the higher-priority route; HR local alone cannot represent its port-link path. IEEE 1800-2009 23.3.3 and Section 6 hierarchy/input-output link rows; O13–O17/O78/O79/O100/O131 source controls.",
    "SYN038-R-PORT-AUTOMATIC-STORAGE": "A structural instance port actual cannot read or target a procedural automatic local; a returned automatic value has a distinct focal return slot. IEEE 1800-2009 6.21/23.3.3 and Section 6 storage/input-output link rows.",
    "SYN038-R-CHILD-PORT-LINK": "HR child_port names an instantiated child port link at a port-actual use site; child-body port reads are local and dotted child-output reads are hierarchical identifiers. IEEE 1800-2009 23.3.3/23.6; SYN-007–010 and Section 6 hierarchy/input-output link rows.",
    "SYN038-R-STATIC-LOCAL-CHILD-PORT": "An unqualified procedural named-block static local is outside the enclosing module-instantiation port-actual scope: static lifetime does not widen lexical visibility. The SV2009 static_unqualified_block_port_actual_rejected.sv negative owner reports undeclared static_value at the actual in both CLI modes; W100's qualified tb.primary_process.static_value positive instead has HR hierarchical_identifier. IEEE 1800-2009 6.21/23.3.3 and Section 6 storage/hierarchy rows.",
    "SYN038-R-INTERFACE-MEMBER-STORAGE": "HR interface_member is a concrete mutable interface data member access, so the same focal slot has SL interface_member. Qualified interface task/function callees use HR hierarchical_identifier; IEEE 1800-2009 23.6/25 and Section 6 storage/hierarchy rows, with O20/O83/O85/O100 source controls.",
    "SYN038-R-ACTUAL-ADDRESS": "Input/const-ref actuals read focal source values; a prior explicit write may select part of that same outer variable, so LV describes the writer. Inout/ref actuals can likewise read a variable's prior blocking/NBA value before alias or copy-in; when WK is none, LV instead describes the actual lvalue. Output actuals only supply an lvalue. Ref/const-ref reject a net actual under IEEE 1800-2009 13.5.2. A read-only ref task can alias a continuously driven variable without assigning it, while inout has mandatory copy-out and a mutating ref assigns it procedurally, conflicting with its continuous driver under 10.3.2 (printed p.179/PDF p.217). SYN-007–010/SYN-013 and SV2009 written-source actual probes.",
    "SYN038-R-READ-SITE": "Return and event consumers can read a focal source slot previously written elsewhere; their consumer use sites are not themselves writes. Constant-elaboration reads retain the direct read-only restriction. Section 6 expression and process rows; W104/W105 and SV2009 source-bound event/return probes.",
    "SYN038-R-RETURN-SLOT": "A focal whole target of a return statement is a function return slot when no separate explicit write is selected. With a selected same-slot write, LV/SL may describe the written source subsequently read by return source; SYN-013 and W104 return-source controls.",
    "SYN038-R-RETURN-LEXICAL": "A function return statement is lexically inside a subroutine even when a process invokes it; SYN-013.",
    "SYN038-R-CONSTANT-TIME": "Constant elaboration has no executing process; SYN-016 and Section 6 pipeline row.",
    "SYN038-R-EVENT-PROCESS": "An explicit/evaluated event control runs in initial/always/always_ff, not the implicit always_comb/always_latch bodies; SYN-011/SYN-014 and their negative controls.",
    "SYN038-R-DECLARATION-TARGET": "A focal declaration-initializer target is a whole declared object. When an initializer instead reads a separately blocking-written focal source, LV describes that source write rather than the receiving declaration; SYN-012/SYN-016 and SV2009 local-source initializer probes.",
    "SYN038-R-DECLARATION-STORAGE": "Automatic/static/runtime/constant initializer kinds constrain storage only when the focal whole object is the declaration target. A blocking-written source consumed by an initializer retains its own SL; SYN-012/SYN-016/SYN-019 and pure-function/local-source initializer probes.",
    "SYN038-R-DECLARATION-PHASE": "Constant localparam/parameter initializers are elaborated and module/interface declaration initializers run in their declaration initialization phase, not an initial/always-family body; IEEE 1800-2009 6.20/6.8 and Section 6 constant/runtime and process rows.",
    "SYN038-R-STATIC-INIT-PHASE": "Static variable declaration initializers, including static block and subroutine locals, execute once before initial/always procedures start, not in the lexically enclosing process; IEEE 1800-2009 6.21/10.5 and Section 6 storage/initialization/process rows.",
    "SYN038-R-FOCAL-WRITE": "WK names an explicit write to the same outer focal slot addressed by LV, possibly before its CO consumer in another process. CO assignment_rhs combines only when that slot is consumed on the assignment RHS; input/const-ref/inout/ref actuals, return/declaration sources and event controls require anchored same-slot source writes. A later const-ref, inout or ref actual can read a prior blocking/NBA variable write; output-only actuals cannot claim that source read. Target-only observations omit CO. Section 6 expression-consumer and destination/driver rows; W61/W73/W99/W104/W105 and SV2009 source-identity controls.",
    "SYN038-R-CONTINUOUS-STORAGE": "Continuous assignment writes static module/interface storage, an output formal, or a hierarchically referenced static local or static function result variable; these function variables are not nets and automatic variables remain excluded. PC names a later assignment-RHS/event/call/return consumer process where present, not the static driver. IEEE 1800-2009 6.21/10.3.2/13.4.1/13.4.2; SYN-006/SYN-007 and same-slot continuous-source probes.",
    "SYN038-R-STATIC-CONTINUOUS-SCOPE": "WK continuous names a static driver outside a procedure. HC may name a subroutine consumer for an event, call or return source path; it does not turn that driver into procedural assign/deassign. IEEE 1800-2009 10.3 versus 10.6 and Section 6 driver/process rows.",
    "SYN038-R-PROCEDURAL-PROCESS": "WK blocking/NBA names a procedural source write. PC names the consumer when CO is present, so a static port link, pure-function declaration initializer, or pure constant helper's return source can have PC none while the separately anchored write executes procedurally. The latter is restricted to a same-slot automatic local written and returned in the subroutine; SYN-014 and Section 6 driver/process rows; SV2009 port and initializer controls.",
    "SYN038-R-AUTOMATIC-NBA": "An NBA cannot target automatic storage in the selected Core form; SYN-013 negative control and Section 6 legality qualifier.",
    "SYN038-R-OUTPUT-EXPRESSION": "Output/inout/ref actuals require an assignable direct projection; SYN-007–010/SYN-013 selected actuals.",
    "SYN038-R-REDUCTION-RECEIVER": "The retained Core fixed-array reduction receiver is a fixed integral or fixed-record array; R03 and SYN-011/SYN-012.",
    "SYN038-R-REDUCTION-READ": "A fixed-array reduction reads its focal array receiver. LV/WK may describe an earlier explicit write to that same outer array, including procedural blocking/NBA or static continuous drivers; they must agree on whether a selected write exists. The reduction does not write its receiver. R03, SYN-011, O21/O129 and same-array written-receiver SV2009 controls.",
    "SYN038-R-TYPE-FIELD": "A named field target requires an outer record, packed structure/union, or array of records; scalar integral/enum and fixed integral-array payloads have no named field. Section 6 TY/LV rows and SYN-002–005.",
    "SYN038-R-REF-ACTUAL-VARIABLE": "A ref actual is one variable, class property, unpacked member or array element; a concatenation or positional assignment-pattern expression cannot itself be a ref actual. When LV describes a prior blocking/NBA write to the same outer variable and the later actual is a direct variable, those earlier LV shapes remain legal. IEEE 1800-2009 13.5.2; SYN-013 and Section 6 formal/destination rows.",
    "SYN038-R-CONSTANT-INTERFACE-SIGNAL": "Constant elaboration cannot read a mutable interface net or variable; interface parameters are not the SL interface_member signal level. IEEE 1800-2009 11.2.1; SYN-016/SYN-019 and Section 6 constant/storage rows.",
    "SYN038-R-CONSTANT-DECL-INTERFACE-SIGNAL": "A parameter/localparam initializer cannot depend on a mutable interface net or variable, directly or through a constant function; interface parameters are outside SL interface_member. IEEE 1800-2009 11.2.1; SYN-016/SYN-019 and Section 6 constant/storage/initializer rows. Direct bus.payload and indirect constant-function probes are rejected in SV2009, while interface runtime initializers remain legal.",
    "SYN038-R-HIERARCHICAL-CONSTANT": "An instance/procedural hierarchical reference cannot supply a selected Core constant-elaboration expression or parameter/localparam initializer, directly or through a constant function. IEEE 1800-2009 6.20.2 and SYN-016/Section 6 constant-hierarchy rows; Slang documents hierarchical constants as a compatibility extension. SV2009 direct u.LIMIT, indirect read_limit() returning u.LIMIT, and $bits(u.payload) probes reject; package source_pkg::LIMIT and its constant function succeed as lexical package scope, not HR hierarchical_identifier. Runtime declaration initializer u.LIMIT remains selected (W78).",
    "SYN038-R-PRODUCER-FORMAL-ACTUAL": "The same focal function-result or fixed-array-reduction-result expression may be an input actual, but cannot be an output/inout procedural assignment target or a ref/const-ref variable actual. A result copied into a separate variable changes the focal slot and has CP none at the later actual. IEEE 1800-2009 13.5 and 13.5.2; Section 6 source/formal rows. Direct make_byte() output/inout/ref/const-ref and lanes.sum() const-ref actual probes reject under SV2009.",
    "SYN038-R-CONST-REF-SOURCE-OP": "A const-ref call actual must denote a variable, class property, unpacked-struct member or unpacked-array element. A conditional, equality, cast or assignment-pattern result on the focal source value is a temporary, not that variable. An operator only in a read-only actual's element index does not change the focal source OP from direct_projection. IEEE 1800-2009 13.5.2; tests/sim_syn038_operation_context_matrix.rs checks four separate SV2009 expression-actual rejections and direct-variable/selected-array-element positive controls in both CLI modes.",
}


def rule_runtime_declaration_scope(a: dict[str, str]) -> bool:
    return a.get("IN") != "runtime_declaration" or a.get("HC") != "subroutine"


def rule_initializer_site(a: dict[str, str]) -> bool:
    co, kind = a.get("CO"), a.get("IN")
    return co is None or kind is None or ((co == "declaration_initializer") == (kind != "none"))


def rule_call_target(a: dict[str, str]) -> bool:
    co, target = a.get("CO"), a.get("CT")
    return co is None or target is None or ((co == "call_argument") == (target != "none"))


def rule_task_process(a: dict[str, str]) -> bool:
    return a.get("CT") != "task" or a.get("PC") != "none"


def rule_formal_site(a: dict[str, str]) -> bool:
    co, mode = a.get("CO"), a.get("FM")
    return co is None or mode is None or ((co in ("port_actual", "call_argument")) == (mode != "none"))


def rule_port_direction(a: dict[str, str]) -> bool:
    return a.get("CO") != "port_actual" or a.get("FM") in (None, "input", "output", "inout", "ref")


def rule_port_route(a: dict[str, str]) -> bool:
    if a.get("CO") != "port_actual":
        return True
    if a.get("WK") not in (None, "none"):
        return (a.get("PC") in (None, "none")
                and a.get("HC") != "subroutine"
                and a.get("FM") in (None, "input"))
    return (a.get("PC") in (None, "none")
            and a.get("WK") in (None, "none")
            and a.get("HC") != "subroutine")


def rule_port_link_route(a: dict[str, str]) -> bool:
    return a.get("CO") != "port_actual" or a.get("HR") != "local"


def rule_port_automatic_storage(a: dict[str, str]) -> bool:
    return a.get("CO") != "port_actual" or a.get("SL") != "automatic_local"


def rule_child_port_link(a: dict[str, str]) -> bool:
    return a.get("HR") != "child_port" or a.get("CO") in (None, "port_actual")


def rule_static_local_child_port(a: dict[str, str]) -> bool:
    return a.get("SL") != "static_local" or a.get("HR") != "child_port"


def rule_interface_member_storage(a: dict[str, str]) -> bool:
    return a.get("HR") != "interface_member" or a.get("SL") in (None, "interface_member")


def rule_actual_address(a: dict[str, str]) -> bool:
    co, mode, address = a.get("CO"), a.get("FM"), a.get("LV")
    if co not in ("port_actual", "call_argument") or mode is None or address is None:
        return True
    if mode in ("input", "const_ref"):
        write = a.get("WK")
        if write is None:
            return True
        if mode == "const_ref" and write == "continuous_net":
            return False
        return matching_focal_write(a) if write != "none" else address == "none"
    if mode in ("inout", "ref"):
        write = a.get("WK")
        if write is None:
            return True
        if write == "none":
            return address != "none"
        return (write in ("procedural_blocking", "procedural_nba")
                or mode == "ref" and write == "continuous_variable") and matching_focal_write(a)
    return address != "none"


def rule_read_site(a: dict[str, str]) -> bool:
    co, address, write = a.get("CO"), a.get("LV"), a.get("WK")
    if co == "event_expression" or (co == "function_return_statement" and write not in (None, "none")):
        return True
    if co in ("constant_elaboration", "event_expression") and address not in (None, "none"):
        return False
    if co == "function_return_statement" and address not in (None, "none", "whole_object"):
        return False
    return co not in ("function_return_statement", "constant_elaboration", "event_expression") or write in (None, "none")


def rule_return_slot(a: dict[str, str]) -> bool:
    if a.get("CO") == "function_return_statement" and a.get("WK") not in (None, "none"):
        return True
    return (a.get("CO") != "function_return_statement" or a.get("LV") != "whole_object"
            or a.get("SL") in (None, "return_slot"))


def rule_return_lexical(a: dict[str, str]) -> bool:
    return a.get("CO") != "function_return_statement" or a.get("HC") in (None, "subroutine")


def rule_constant_time(a: dict[str, str]) -> bool:
    return a.get("CO") != "constant_elaboration" or a.get("PC") in (None, "none")


def rule_event_process(a: dict[str, str]) -> bool:
    return a.get("CO") != "event_expression" or a.get("PC") not in ("none", "always_comb", "always_latch")


def rule_declaration_target(a: dict[str, str]) -> bool:
    co, address, write = a.get("CO"), a.get("LV"), a.get("WK")
    if co == "declaration_initializer" and write == "procedural_blocking":
        return True
    if co == "declaration_initializer" and address not in (None, "none", "whole_object"):
        return False
    return co != "declaration_initializer" or write in (None, "none")


def rule_declaration_storage(a: dict[str, str]) -> bool:
    if a.get("CO") == "declaration_initializer" and a.get("WK") == "procedural_blocking":
        return True
    if a.get("CO") != "declaration_initializer" or a.get("LV") != "whole_object":
        return True
    kind, storage = a.get("IN"), a.get("SL")
    allowed = {
        "constant_declaration": ("module_package", "static_local", "formal"),
        "automatic_local": ("automatic_local",),
        "static_local": ("static_local",),
        "runtime_declaration": ("module_package", "interface_member"),
    }.get(kind)
    return allowed is None or storage is None or storage in allowed


def rule_declaration_phase(a: dict[str, str]) -> bool:
    return a.get("IN") not in ("constant_declaration", "runtime_declaration") or a.get("PC") in (None, "none")


def rule_static_init_phase(a: dict[str, str]) -> bool:
    return a.get("IN") != "static_local" or a.get("PC") in (None, "none")


def matching_focal_write(a: dict[str, str]) -> bool:
    address, write = a.get("LV"), a.get("WK")
    return address is None or write is None or (address == "none") == (write == "none")


def rule_focal_write(a: dict[str, str]) -> bool:
    co, address, write = a.get("CO"), a.get("LV"), a.get("WK")
    if co == "assignment_rhs" and address is not None and write is not None:
        return matching_focal_write(a)
    if co == "event_expression":
        return matching_focal_write(a)
    if co in ("port_actual", "call_argument") and a.get("FM") in (None, "input", "const_ref", "inout", "ref") and write not in (None, "none"):
        return matching_focal_write(a)
    if co == "function_return_statement" and write not in (None, "none"):
        return matching_focal_write(a)
    if co == "declaration_initializer" and write == "procedural_blocking":
        return matching_focal_write(a)
    return co in (None, "assignment_rhs") or write in (None, "none")


def rule_continuous_storage(a: dict[str, str]) -> bool:
    write = a.get("WK")
    if write not in ("continuous_net", "continuous_variable"):
        return True
    allowed_storage = (None, "module_package", "interface_member", "formal")
    if write == "continuous_variable":
        allowed_storage += ("static_local", "return_slot")
        if a.get("SL") in ("static_local", "return_slot") and a.get("HR") not in (None, "hierarchical_identifier"):
            return False
    consumer_can_have_process = a.get("CO") in (
        None, "assignment_rhs", "event_expression", "call_argument", "function_return_statement"
    )
    return ((consumer_can_have_process or a.get("PC") in (None, "none"))
            and a.get("SL") in allowed_storage)


def rule_static_continuous_scope(a: dict[str, str]) -> bool:
    if a.get("CO") in (None, "event_expression", "call_argument", "function_return_statement"):
        return True
    return a.get("WK") not in ("continuous_net", "continuous_variable") or a.get("HC") != "subroutine"


def rule_procedural_process(a: dict[str, str]) -> bool:
    write = a.get("WK")
    if write not in ("procedural_blocking", "procedural_nba") or a.get("PC") != "none":
        return True
    co = a.get("CO")
    if co in (None, "port_actual"):
        return True
    if co == "declaration_initializer" and write == "procedural_blocking" and a.get("CP") in (None, "function"):
        return True
    return (co == "function_return_statement" and write == "procedural_blocking"
            and a.get("SL") == "automatic_local" and a.get("HC") == "subroutine"
            and a.get("CP") in (None, "none") and a.get("IN") in (None, "none"))


def rule_automatic_nba(a: dict[str, str]) -> bool:
    return a.get("WK") != "procedural_nba" or a.get("SL") != "automatic_local"


def rule_output_expression(a: dict[str, str]) -> bool:
    if a.get("CO") not in ("port_actual", "call_argument") or a.get("FM") not in ("output", "inout", "ref"):
        return True
    return a.get("OP") in (None, "direct_projection")


def rule_reduction_receiver(a: dict[str, str]) -> bool:
    return (a.get("CP") != "fixed_array_reduction" or
            a.get("TY") in (None, "fixed_array_integral", "fixed_array_record"))


def rule_reduction_read(a: dict[str, str]) -> bool:
    return a.get("CP") != "fixed_array_reduction" or matching_focal_write(a)


def rule_type_field(a: dict[str, str]) -> bool:
    return a.get("LV") != "field" or a.get("TY") not in ("integral_bit_logic", "enum", "fixed_array_integral")


def rule_ref_actual_variable(a: dict[str, str]) -> bool:
    return (a.get("FM") != "ref" or a.get("LV") not in ("concatenation", "positional_pattern")
            or a.get("WK") != "none")


def rule_constant_interface_signal(a: dict[str, str]) -> bool:
    return a.get("CO") != "constant_elaboration" or a.get("SL") != "interface_member"


def rule_constant_decl_interface_signal(a: dict[str, str]) -> bool:
    return a.get("IN") != "constant_declaration" or a.get("SL") != "interface_member"


def rule_hierarchical_constant(a: dict[str, str]) -> bool:
    return (a.get("HR") != "hierarchical_identifier" or
            a.get("CO") != "constant_elaboration" and a.get("IN") != "constant_declaration")


def rule_producer_formal_actual(a: dict[str, str]) -> bool:
    return a.get("CP") in (None, "none") or a.get("FM") not in ("output", "inout", "ref", "const_ref")


def rule_const_ref_source_operation(a: dict[str, str]) -> bool:
    return (a.get("CO") != "call_argument" or a.get("FM") != "const_ref"
            or a.get("OP") in (None, "direct_projection"))


RULES = (
    ("SYN038-R-RUNTIME-DECL-SCOPE", rule_runtime_declaration_scope),
    ("SYN038-R-INITIALIZER-SITE", rule_initializer_site),
    ("SYN038-R-CALL-TARGET", rule_call_target),
    ("SYN038-R-TASK-PROCESS", rule_task_process),
    ("SYN038-R-FORMAL-SITE", rule_formal_site),
    ("SYN038-R-PORT-DIRECTION", rule_port_direction),
    ("SYN038-R-PORT-ROUTE", rule_port_route),
    ("SYN038-R-PORT-LINK-ROUTE", rule_port_link_route),
    ("SYN038-R-PORT-AUTOMATIC-STORAGE", rule_port_automatic_storage),
    ("SYN038-R-CHILD-PORT-LINK", rule_child_port_link),
    ("SYN038-R-STATIC-LOCAL-CHILD-PORT", rule_static_local_child_port),
    ("SYN038-R-INTERFACE-MEMBER-STORAGE", rule_interface_member_storage),
    ("SYN038-R-ACTUAL-ADDRESS", rule_actual_address),
    ("SYN038-R-READ-SITE", rule_read_site),
    ("SYN038-R-RETURN-SLOT", rule_return_slot),
    ("SYN038-R-RETURN-LEXICAL", rule_return_lexical),
    ("SYN038-R-CONSTANT-TIME", rule_constant_time),
    ("SYN038-R-EVENT-PROCESS", rule_event_process),
    ("SYN038-R-DECLARATION-TARGET", rule_declaration_target),
    ("SYN038-R-DECLARATION-STORAGE", rule_declaration_storage),
    ("SYN038-R-DECLARATION-PHASE", rule_declaration_phase),
    ("SYN038-R-STATIC-INIT-PHASE", rule_static_init_phase),
    ("SYN038-R-FOCAL-WRITE", rule_focal_write),
    ("SYN038-R-CONTINUOUS-STORAGE", rule_continuous_storage),
    ("SYN038-R-STATIC-CONTINUOUS-SCOPE", rule_static_continuous_scope),
    ("SYN038-R-PROCEDURAL-PROCESS", rule_procedural_process),
    ("SYN038-R-AUTOMATIC-NBA", rule_automatic_nba),
    ("SYN038-R-OUTPUT-EXPRESSION", rule_output_expression),
    ("SYN038-R-REDUCTION-RECEIVER", rule_reduction_receiver),
    ("SYN038-R-REDUCTION-READ", rule_reduction_read),
    ("SYN038-R-TYPE-FIELD", rule_type_field),
    ("SYN038-R-REF-ACTUAL-VARIABLE", rule_ref_actual_variable),
    ("SYN038-R-CONSTANT-INTERFACE-SIGNAL", rule_constant_interface_signal),
    ("SYN038-R-CONSTANT-DECL-INTERFACE-SIGNAL", rule_constant_decl_interface_signal),
    ("SYN038-R-HIERARCHICAL-CONSTANT", rule_hierarchical_constant),
    ("SYN038-R-PRODUCER-FORMAL-ACTUAL", rule_producer_formal_actual),
    ("SYN038-R-CONST-REF-SOURCE-OP", rule_const_ref_source_operation),
)


def find_support(fixed: dict[str, str], core: bool, enabled: tuple[str, ...] | None = None) -> dict[str, str] | None:
    if core and (fixed.get("TY") == "tagged_extended" or fixed.get("IN") == "memory_image"):
        return None
    active = [(name, check) for name, check in RULES if enabled is None or name in enabled]
    assignment = dict(fixed)

    def search(index: int) -> dict[str, str] | None:
        if index == len(SEARCH_ORDER):
            return dict(assignment)
        key = SEARCH_ORDER[index]
        if key in assignment:
            return search(index + 1)
        for value in FACTORS[key]:
            if core and (key == "TY" and value == "tagged_extended" or key == "IN" and value == "memory_image"):
                continue
            assignment[key] = value
            if all(check(assignment) for _, check in active):
                found = search(index + 1)
                if found is not None:
                    del assignment[key]
                    return found
        del assignment[key]
        return None

    if not all(check(assignment) for _, check in active):
        return None
    return search(0)


def impossible_rules(fixed: dict[str, str]) -> list[str]:
    active = [name for name, _ in RULES]
    for name in tuple(active):
        smaller = tuple(item for item in active if item != name)
        if find_support(fixed, False, smaller) is None:
            active.remove(name)
    if not active:
        raise ValueError(f"no structural explanation for {fixed}")
    return active


def cell_id(prefix: str, left: str, left_value: str, right: str, right_value: str) -> str:
    return f"SYN038-{prefix}-{left}-{left_value}__{right}-{right_value}"


def raw_cells():
    for left, right in combinations(FACTORS, 2):
        for left_value, right_value in product(FACTORS[left], FACTORS[right]):
            yield left, left_value, right, right_value


def observation_pairs(manifest: dict) -> dict[str, list[str]]:
    evidence = {row["id"]: row for row in manifest["evidence"]}
    if len(evidence) != len(manifest["evidence"]):
        raise ValueError("duplicate evidence ID")
    seen = set()
    covered = defaultdict(list)
    for observation in manifest["observations"]:
        obs_id = observation["id"]
        if obs_id in seen:
            raise ValueError(f"duplicate observation ID: {obs_id}")
        seen.add(obs_id)
        witness = observation["evidence_id"]
        if witness not in evidence:
            raise ValueError(f"{obs_id}: unknown evidence {witness}")
        fixture_text = (ROOT / evidence[witness]["fixture"]).read_text(encoding="utf-8")
        fixture_code = "\n".join(line for line in fixture_text.splitlines() if not line.lstrip().startswith("//"))
        anchors = observation.get("source_anchors")
        if not isinstance(anchors, list) or not anchors or any(not isinstance(anchor, str) or not anchor or anchor not in fixture_code for anchor in anchors):
            raise ValueError(f"{obs_id}: source anchors missing from fixture")
        if not any(fixture_code.count(anchor) == 1 for anchor in anchors):
            raise ValueError(f"{obs_id}: no unique focal source anchor")
        levels = observation["levels"]
        if len(levels) < 2:
            raise ValueError(f"{obs_id}: fewer than two factor levels")
        missing = [factor for factor in FACTORS if factor not in levels]
        if missing:
            if observation.get("missing_factors") != missing or not observation.get("sparse_basis"):
                raise ValueError(f"{obs_id}: sparse factor vector lacks its exact missing-axis basis")
        elif "missing_factors" in observation or "sparse_basis" in observation:
            raise ValueError(f"{obs_id}: full factor vector has stale sparse-axis metadata")
        for factor, value in levels.items():
            if factor not in FACTORS or value not in FACTORS[factor]:
                raise ValueError(f"{obs_id}: unknown {factor}={value}")
        if find_support(levels, True) is None:
            raise ValueError(f"{obs_id}: assigned levels have no coherent Core path")
        qualifiers = observation.get("qualifiers", {})
        catalog = manifest["qualifier_catalog"]
        for qualifier, values in qualifiers.items():
            if qualifier not in catalog or not isinstance(values, list) or not values or not set(values) <= set(catalog[qualifier]):
                raise ValueError(f"{obs_id}: invalid qualifier {qualifier}={values}")
        if not qualifiers.get("edition") or not qualifiers.get("pipeline_mode") or not qualifiers.get("constant_runtime"):
            raise ValueError(f"{obs_id}: edition, pipeline and constant/runtime qualifiers are required")
        editions = {"V2001" if value == "2001" else "SV2009" for value in evidence[witness].get("editions", ["2009"])}
        if not set(qualifiers["edition"]) <= editions:
            raise ValueError(f"{obs_id}: edition qualifier exceeds {witness} run metadata")
        if not set(qualifiers["pipeline_mode"]) <= set(evidence[witness]["pipeline_mode"]):
            raise ValueError(f"{obs_id}: pipeline qualifier exceeds {witness} run metadata")
        for left, right in combinations(FACTORS, 2):
            if left in levels and right in levels:
                pair = {left: levels[left], right: levels[right]}
                if find_support(pair, True) is None:
                    raise ValueError(f"{obs_id}: impossible observed pair {pair}")
                covered[cell_id("PW", left, levels[left], right, levels[right])].append(obs_id)
    return {key: sorted(value) for key, value in covered.items()}


def generated_catalog(manifest: dict) -> list[dict]:
    covered = observation_pairs(manifest)
    result = []
    for left, left_value, right, right_value in raw_cells():
        fixed = {left: left_value, right: right_value}
        pair_id = cell_id("PW", left, left_value, right, right_value)
        row = {"axes": [left, right], "levels": fixed}
        support = find_support(fixed, True)
        if support is not None:
            witnesses = covered.get(pair_id, [])
            row.update({"id": pair_id, "applicability": "selected_core", "selection_rule_id": f"SYN038-CORE-PATH-{support['CO']}", "evidence_status": "covered" if witnesses else "planned_legal_gap"})
            if witnesses:
                row["observation_ids"] = witnesses
            else:
                row["gap_id"] = cell_id("GAP", left, left_value, right, right_value)
        elif find_support(fixed, False) is not None:
            reasons = []
            if "memory_image" in fixed.values():
                reasons.append("SYN038-OOS-EXT-MEMORY-IMAGE")
            if "tagged_extended" in fixed.values():
                reasons.append("SYN038-OOS-EXT-TAGGED")
            row.update({"id": cell_id("OOS", left, left_value, right, right_value), "applicability": "outside_profile", "reason_rule_ids": reasons})
        else:
            row.update({"id": cell_id("IAP", left, left_value, right, right_value), "applicability": "impossible", "reason_rule_ids": impossible_rules(fixed)})
        result.append(row)
    return result


def validate_manifest(manifest: dict, expected: list[dict]) -> Counter:
    if manifest.get("schema") != SCHEMA:
        raise ValueError(f"schema must be {SCHEMA}")
    if tuple(manifest.get("factors", {})) != tuple(FACTORS):
        raise ValueError("factor IDs/order differ from frozen denominator")
    for factor, values in FACTORS.items():
        row = manifest["factors"][factor]
        if row.get("meaning") != FACTOR_MEANINGS[factor] or tuple(row.get("levels", [])) != values:
            raise ValueError(f"factor {factor} meaning or levels differ from frozen denominator")
    if manifest.get("core_path_basis") != {f"SYN038-CORE-PATH-{key}": value for key, value in PATH_BASIS.items()}:
        raise ValueError("Core path provenance differs from checker")
    if manifest.get("structural_rule_basis") != RULE_BASIS:
        raise ValueError("structural rule provenance differs from checker")
    if manifest.get("outside_rule_basis") != OUTSIDE_BASIS:
        raise ValueError("outside-profile provenance differs from checker")
    if manifest.get("pair_catalog") != expected:
        by_id = {row.get("id"): row for row in manifest.get("pair_catalog", [])}
        first = next((row["id"] for row in expected if by_id.get(row["id"]) != row), "unknown")
        raise ValueError(f"pair catalog differs from rule-derived cells at {first}; run --refresh-cells after reviewing rules")
    counts = Counter(row["applicability"] for row in expected)
    counts.update({"covered": sum(row.get("evidence_status") == "covered" for row in expected), "planned_legal_gap": sum(row.get("evidence_status") == "planned_legal_gap" for row in expected)})
    if manifest.get("baseline_counts") != {"raw": len(expected), **dict(counts)}:
        raise ValueError("baseline counts differ from derived catalog")
    if manifest.get("required_zero_legal_gaps") is not True:
        raise ValueError("zero selected legal gaps are not required by the manifest")
    if counts["planned_legal_gap"] != 0:
        raise ValueError(f"{counts['planned_legal_gap']} selected legal gaps remain")
    if len(expected) != 2247 or len({row["id"] for row in expected}) != len(expected):
        raise ValueError("raw pair inventory is not the frozen 2,247-cell grid")
    blocks = Counter(tuple(row["axes"]) for row in expected)
    if len(blocks) != 78:
        raise ValueError("not every factor pair has a block")
    for left, right in combinations(FACTORS, 2):
        if blocks[(left, right)] != len(FACTORS[left]) * len(FACTORS[right]):
            raise ValueError(f"missing cells for {left}-{right}")
    selected = {row["id"] for row in expected if row["applicability"] == "selected_core"}
    required_ids = {cell_id("PW", *pair) for pair in REQUIRED_SELECTED}
    if {row["id"] for row in manifest["required_selected_pairs"]} != required_ids:
        raise ValueError("required selected-pair regression inventory differs from checker")
    observation_evidence = {row["id"]: row["evidence_id"] for row in manifest["observations"]}
    for row in manifest["required_selected_pairs"]:
        if row["id"] not in selected or not row.get("basis"):
            raise ValueError(f"required selected pair lost: {row['id']}")
        basis_witnesses = set(re.findall(r"W\d+", row["basis"]))
        cell = next(item for item in expected if item["id"] == row["id"])
        covering_witnesses = {observation_evidence[obs_id] for obs_id in cell.get("observation_ids", [])}
        if basis_witnesses and not basis_witnesses & covering_witnesses:
            raise ValueError(f"required selected pair has stale evidence basis: {row['id']}")
    for pair in REQUIRED_IMPOSSIBLE:
        if find_support(dict(zip((pair[0], pair[2]), (pair[1], pair[3]))), False) is not None:
            raise ValueError(f"required impossible pair became feasible: {pair}")
    for pair in REQUIRED_OUTSIDE:
        if cell_id("OOS", *pair) not in {row["id"] for row in expected if row["applicability"] == "outside_profile"}:
            raise ValueError(f"required outside-profile pair changed: {pair}")
    for factor, values in FACTORS.items():
        for value in values:
            if value in ("tagged_extended", "memory_image"):
                continue
            if not any(row["levels"].get(factor) == value for row in expected if row["applicability"] == "selected_core"):
                raise ValueError(f"Core level has no selected pair: {factor}={value}")
    required_chains = {"SYN038-3WAY-01", "SYN038-3WAY-02", "SYN038-3WAY-03", "SYN038-3WAY-04"}
    if {row["id"] for row in manifest["targeted_chains"]} != required_chains:
        raise ValueError("targeted three-way chain inventory differs from plan")
    evidence = {row["id"]: row for row in manifest["evidence"]}
    for chain in manifest["targeted_chains"]:
        if chain.get("evidence_id") not in evidence or len(chain.get("terms", [])) < 3:
            raise ValueError(f"{chain['id']}: missing terms or evidence")
    qualifier_catalog = manifest.get("qualifier_catalog", {})
    for qualifier in ("shape", "width", "state_and_effect", "driver_topology", "edition", "compilation_unit", "constant_runtime", "pipeline_mode", "snapshot_drop"):
        if not qualifier_catalog.get(qualifier):
            raise ValueError(f"missing qualifier catalog {qualifier}")
    for evidence_id, row in evidence.items():
        fixture = ROOT / row["fixture"]
        runner = ROOT / row["runner"]
        if not fixture.is_file() or not runner.is_file():
            raise ValueError(f"{evidence_id}: fixture or runner is missing")
        owner_path = ROOT / row["owner"].split("::")[0]
        owner_source = owner_path if owner_path.is_file() else runner
        source_text = owner_source.read_text(encoding="utf-8")
        owner_body = source_text
        owner_name = row["owner"].split("::")[-1]
        if "::conversion_cases!::" not in row["owner"] and "::operation_cases!::" not in row["owner"]:
            marker = f"fn {owner_name}("
            start = source_text.find(marker)
            if start < 0:
                raise ValueError(f"{evidence_id}: owner function is missing")
            end = source_text.find("\n#[test]", start + len(marker))
            owner_body = source_text[start:end if end >= 0 else None]
        helper_chain = row.get("cli_helper_chain", [])
        if not isinstance(helper_chain, list) or any(not isinstance(name, str) or not name for name in helper_chain):
            raise ValueError(f"{evidence_id}: malformed CLI helper chain")
        invocation_bodies = [owner_body]
        for helper in helper_chain:
            if f"{helper}(" not in invocation_bodies[-1]:
                raise ValueError(f"{evidence_id}: CLI helper chain does not follow an owner call")
            marker = f"fn {helper}("
            start = source_text.find(marker)
            if start < 0:
                raise ValueError(f"{evidence_id}: CLI helper {helper} is missing")
            ends = [position for position in (source_text.find("\nfn ", start + len(marker)), source_text.find("\n#[test]", start + len(marker))) if position >= 0]
            invocation_bodies.append(source_text[start:min(ends) if ends else None])
        invocation_source = "\n".join(invocation_bodies)
        if "::conversion_cases!::" not in row["owner"] and "::operation_cases!::" not in row["owner"]:
            if fixture.stem not in invocation_source:
                raise ValueError(f"{evidence_id}: owning test does not invoke its fixture")
        oracle_source = invocation_source
        module_constant = row.get("module_oracle_constant")
        if module_constant:
            if module_constant not in oracle_source or f"const {module_constant}:" not in source_text:
                raise ValueError(f"{evidence_id}: module oracle constant is not bound by owner")
            oracle_source = source_text
        if not row.get("tokens") or any(token not in source_text for token in row["tokens"]):
            raise ValueError(f"{evidence_id}: oracle/invocation tokens are missing from owner source")
        mode = row.get("oracle_mode")
        if mode not in ("literal_lines", "derived", "generated"):
            raise ValueError(f"{evidence_id}: oracle mode is missing")
        if mode == "literal_lines":
            if not row.get("stdout") or any(f"{line}\\n" not in oracle_source for line in row["stdout"].splitlines()):
                raise ValueError(f"{evidence_id}: exact stdout lines are absent from owner")
        else:
            anchors = row.get("oracle_anchors")
            if not isinstance(anchors, list) or not anchors or any(anchor not in oracle_source for anchor in anchors):
                raise ValueError(f"{evidence_id}: derived oracle anchors are absent from owner")
            if mode == "generated" and not row.get("stdout_oracle"):
                raise ValueError(f"{evidence_id}: generated oracle description is missing")
        if row.get("stderr_mode") == "derived":
            anchors = row.get("stderr_anchors")
            if not isinstance(anchors, list) or not anchors or any(anchor not in oracle_source for anchor in anchors):
                raise ValueError(f"{evidence_id}: derived stderr anchors are absent from owner")
        elif row.get("stderr") and any(f"{line}\\n" not in oracle_source for line in row["stderr"].splitlines()):
            raise ValueError(f"{evidence_id}: exact stderr lines are absent from owner")
        if not row.get("pipeline_mode") or not (row.get("editions") or row.get("edition_mode")):
            raise ValueError(f"{evidence_id}: missing edition or pipeline metadata")
        edition_args = row.get("cli_edition_args")
        if not isinstance(edition_args, list):
            raise ValueError(f"{evidence_id}: CLI edition arguments are missing")
        if edition_args:
            if not row.get("editions") or row.get("edition_mode"):
                raise ValueError(f"{evidence_id}: explicit edition metadata is inconsistent")
            for argument_list in edition_args:
                if len(argument_list) != 2 or argument_list[0] != "--edition" or argument_list[1] not in row["editions"]:
                    raise ValueError(f"{evidence_id}: malformed explicit CLI edition arguments")
                direct_args = all(f'"{argument}"' in invocation_source for argument in argument_list)
                shared_args = ("EDITION" in owner_body and
                               all(f'"{argument}"' in source_text for argument in argument_list) and
                               "const EDITION:" in source_text)
                if not direct_args and not shared_args:
                    raise ValueError(f"{evidence_id}: named owner does not pass its edition arguments")
        elif not row.get("edition_mode") or row.get("editions"):
            raise ValueError(f"{evidence_id}: default edition metadata is inconsistent")
    supplemental = manifest.get("supplemental_witnesses", [])
    if len(supplemental) != 1 or supplemental[0].get("id") != "W15" or supplemental[0].get("snapshot_drop") != "performed_before_owned_database_validation_and_lowering":
        raise ValueError("owned-DB native snapshot-drop witness is missing")
    if not (ROOT / supplemental[0]["fixture"]).is_file():
        raise ValueError("owned-DB snapshot-drop fixture is missing")
    return counts


def main() -> int:
    parser = argparse.ArgumentParser(description="Check the frozen SYN-038 Core value-pair denominator and evidence mapping")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--refresh-cells", action="store_true")
    parser.add_argument("--emit-cells", action="store_true")
    parser.add_argument("--gaps-by-block", action="store_true")
    args = parser.parse_args()
    try:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
        expected = generated_catalog(manifest)
        if args.refresh_cells:
            manifest["pair_catalog"] = expected
            counts = Counter(row["applicability"] for row in expected)
            counts.update({"covered": sum(row.get("evidence_status") == "covered" for row in expected), "planned_legal_gap": sum(row.get("evidence_status") == "planned_legal_gap" for row in expected)})
            manifest["baseline_counts"] = {"raw": len(expected), **dict(counts)}
            args.manifest.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        counts = validate_manifest(manifest, expected)
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        print(f"SYN-038 pairwise manifest: {exc}", file=sys.stderr)
        return 1
    if args.emit_cells:
        for row in expected:
            detail = row.get("gap_id") or ",".join(row.get("observation_ids", row.get("reason_rule_ids", [])))
            print(f"{row['id']}\t{row['applicability']}\t{row.get('evidence_status', '')}\t{detail}")
    elif args.gaps_by_block:
        groups = defaultdict(list)
        for row in expected:
            if row.get("evidence_status") == "planned_legal_gap":
                groups["-".join(row["axes"])].append(row["gap_id"])
        for block, gaps in groups.items():
            print(f"{block}\t{len(gaps)}")
    print(f"SYN-038 pairwise: {len(expected)} raw, {counts['selected_core']} selected Core, {counts['covered']} covered, {counts['planned_legal_gap']} legal gaps, {counts['impossible']} impossible, {counts['outside_profile']} outside profile")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
