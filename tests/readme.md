# Tests

## Scope and layout

- Shared Slang frontend, owned semantic database, simulator and LSP integration.
- [Shared harnesses](support/readme.md): temporary directories, process cleanup and timeouts.
- [LSP fixtures](fixtures/lsp/): framed stdio tests, manifests and declared source headers.
- [Simulation feature status](../docs/sim_features.md): the sole support checklist.

## Simulator testing methodology

- Target IEEE 1364-2001 and IEEE 1800-2009 using the clause references and production labels recorded in the tracked feature and ledger documents. The workspace-local [specification reference pack](../docs/specification/) provides optional review sources; its PDFs and maps are untracked and are not required to run the checker or reproduce the coverage audit.
- Keep end-to-end designs in checked-in `.v` / `.sv` files; pass them directly to `llg`.
- Run each conformance fixture with default optimization and `--no-opt`.
- Keep independent expected results in Rust: explicit truth tables, bit strings and width/signedness arithmetic.
- Compare exact stdout, expected diagnostics and exit status; reject unexpected lowering warnings.
- Test invalid syntax/unsupported contexts separately from successful execution.
- Isolate child working directories; serialize and restore any parent CWD changes.
- Use focused in-memory sources for frontend, database and IR unit tests.
- Run generated C under GCC ASan/UBSan; sanitizer coverage does not instrument the vendored Slang archive.

### Review continuation: concat, tagged reads, helper flow and ordering

- `cargo test --locked --test sim_review_next4 -- --test-threads=1` runs the
  checked-in `review_bundle/n01_*`, `n02_*`, `n06_*` and `n07_*` cases selected by
  that suite. It uses both optimizer modes, with a paired-edition legacy concat
  control, strict positive output, and specific eligibility/const-ref errors.
- Unit filters `singleton_concat`, `return_flow::tests`, and
  `arguments::tests::frozen_activation_receivers` cover typed rewrites, lexical
  exits and frozen receiver paths without treating source recognition as execution.
- `owned::tests::tagged_signed` includes a model built by the actual emitter.
  Run it with the established generated-C sanitizer flags to check ownership;
  handwritten native-runtime probes are not a substitute for that model or CLI
  execution. See the maintained feature checklist for qualification status.

### Vendor patch preparation

`vendor_patches.rs` covers clean and already-applied checkouts, source trees
without Git metadata, archives nested in an outer Git checkout, exact active
and retired file manifests, authenticated rendered output, LF digest
normalization with CRLF preservation, and rejection of symlink, Windows
reparse-point, hard-link, stale, and untracked source inputs. Its deterministic
handle-relative regressions cover ancestor and parent replacement plus a
temporary hard-link insertion after the staging identity check. The focused
command is:

```sh
cargo test --locked --test vendor_patches -- --test-threads=1
```

### Fixture integrity before a native build

Run `python3 scripts/check_sim_fixture_integrity.py --tracked` after staging every
new fixture. The gate checks the known static harness shapes, not arbitrary Rust
expressions. Its unit tests run with
`python3 -m unittest discover -s scripts -p test_sim_fixture_integrity.py`.
CI runs both before the native build. Missing files and files absent from Git's
index are errors; do not silently skip their tests.

The Group 1 repair supplies 51 newly authored replacements for missing HDL
inputs in the delivered topic suites. They preserve those suites' independent
value/diagnostic expectations, except where this repair deliberately converts
R14's legal packed-input/ref rejection cases into positive tests. The original
missing contents and six advertised-but-absent suites were not recovered.
Their historical pass counts are not part of the current acceptance record.

### SYN-038 grammar/context ledger and pairwise CLI witnesses — 2026-09-23

`sim_syn038_ledger` checks the maintained [SYN-038 ledger](../docs/sim_features.md#syn-038-selected-core-grammar-by-context-ledger--2026-09-23)
without running HDL for its structural checks. It verifies selected rows have
unique stable IDs, explicit `V2001`/`SV2009` gates, `PASS` or `REJECT`
outcomes, existing fixture paths and required context axes. It also checks
selected-profile exclusions and that the historical 72-group inventory has
one disposition for every ID from 1 through 72.

The v4 [`sim_syn038_pairwise_manifest.json`](sim_syn038_pairwise_manifest.json)
defines the selected typed-value/source-path matrix with 13 factors: `TY`,
`OP`, `CO`, `LV`, `SL`, `FM`, `HC`, `HR`, `CP`, `CT`, `IN`, `WK`, and `PC`.
The frozen checker reports **2,247 raw / 1,849 selected Core / 1,849 covered /
0 legal gaps / 296 impossible / 102 outside-profile**. The manifest sets
`required_zero_legal_gaps=true`; `sim_syn038_ledger` and the checker fail if a
selected legal gap appears. A mutation that removes a closing witness was also
rejected. The checker SHA-256 is
`b1ccaad76c1488195b55923ce5391c748cfe94cb6879b61215064ace4d43ea6e`; the
manifest SHA-256 is
`941e2f1ab12064cfa820686e36ba2e29bbc8a418940c26e265f3d1edfe460757`.

W91–W96 add 54 selected pairs; W97 adds three; W98 adds 21; W99–W102 add nine;
W103 adds ten; W104 adds seven after one overlap. W105–W118 add reviewed
source-bound event, reduction, return, initializer, continuous-driver,
port, and task/function paths. W119 adds five selected-input paths; W120 adds
seven const-ref prior-source paths; W121 adds four blocking ref/inout paths;
W122 adds two NBA ref/inout paths; W123 adds one read-only ref actual to a
continuously driven variable after an explicit `#1` settle. Their exact CLI
oracles and DB owners are listed in the table below. The W123 warning is
expected and pinned at fixture line 15 in both optimizer modes.

The accepted legacy corrections move 23 `CP=function` labels from receiving
assignment targets to their actual function-result sources. The 18 W79/W103
address-only rows omit `CO=assignment_rhs`. The broader focal audit retains
that consumer only for 16 same-focal RHS reads and omits it on 176 target-only
or distinct-source rows; the earlier 18 sparse address rows remain sparse.
W99 O573–O575 and W75 O271–O275 are target writes whose RHS is a distinct
source, so they are not assignment-RHS consumer observations.

The source-bound applicability rules include the two narrow W96 rules for
`OP=conditional/equality_inside/cast_stream/assignment_pattern × FM=const_ref`
and `HC=subroutine × IN=runtime_declaration`. `SYN038-R-FOCAL-WRITE` and
`SYN038-R-READ-SITE` connect a write to the same focal object with a later
consumer at an event, reduction, return, port, task or function site;
`SYN038-R-REDUCTION-READ`, `SYN038-R-ACTUAL-ADDRESS`,
`SYN038-R-REF-ACTUAL-VARIABLE`, and continuous-storage rules constrain those
paths. All 32 newly selected pair transitions have explicit rule pins and
covering observations; the inout × continuous-variable pair is explicitly
impossible. The final denominator has zero legal gaps. SYN-038/SYN-039
selected-profile host acceptance passed the composed gates below.

`SYN038-R-HIERARCHICAL-CONSTANT` applies IEEE 1800-2009 §6.20.2: hierarchical
names are not legal sources in constant elaboration or parameter/localparam
constant declarations, including through constant functions and `$bits`.
Package-scoped references remain lexical package paths. W78 covers the separate
runtime declaration-initializer path from `u.LIMIT`.

`SYN038-R-PRODUCER-FORMAL-ACTUAL` applies IEEE 1800-2009 §13.5.2 to a producer
expression used directly as a subroutine actual. A function result cannot serve
as an `output` or `inout` assignment target or as a `ref`/`const ref` variable
actual; a fixed-array reduction result cannot serve as a `const ref` actual.
The rule moves `FM=output/inout/ref/const_ref × CP=function` and
`FM=const_ref × CP=fixed_array_reduction` to impossible. Input actuals still
accept expressions. Copying a result into a separate variable changes the
focal slot and permits a valid reference actual; a positive copied-variable
control is paired with direct SV2009 rejection probes in both optimizer modes.
The rule does not reclassify `SYN038-PW-LV-row_slice__FM-ref`: the audited
packed part-select module-`ref`-port case is legal and W90/O502 covers it.

`SYN038-R-CONST-REF-SOURCE-OP` applies only when the result of a conditional,
equality/inside, cast, or assignment-pattern operation is itself passed to a
subroutine `const ref` formal. Those four `OP × FM=const_ref` cells are
impossible because the actual expression is a temporary rather than an
eligible variable. An operator used only in a selected element's index leaves
the focal source operation as `direct_projection` and remains a legal
const-ref variable actual. The checked-in operation-context suite contains
the four direct-expression negative probes in
`fixtures/sim/syn038_pairwise/constref_conditional_rejected.sv`,
`fixtures/sim/syn038_pairwise/constref_equality_rejected.sv`,
`fixtures/sim/syn038_pairwise/constref_cast_rejected.sv`, and
`fixtures/sim/syn038_pairwise/constref_pattern_rejected.sv`, plus
selected-element positive controls. The owner
`sim_syn038_operation_context_matrix::typed_operation_contexts_keep_source_and_use_site_in_both_cli_modes`
runs both SV2009 optimizer modes and requires empty stdout, exit code 1, and
Slang's exact `invalid expression for pass by reference; only variables, class
properties, and members of unpacked structs and arrays are allowed` diagnostic.

`SYN038-R-RUNTIME-DECL-SCOPE` marks only
`HC=subroutine × IN=runtime_declaration` impossible. Runtime declaration
initializers belong to module/interface declaration sites; a local declaration
inside a subroutine is classified as `automatic_local` or `static_local`, even
when its initializer calls a function. IEEE 1800-2009 §§6.8 and 6.21 and
W82/O379's module-scope runtime initializer witness preserve that distinction.
The five resulting applicability IDs are
`SYN038-IAP-OP-conditional__FM-const_ref`,
`SYN038-IAP-OP-equality_inside__FM-const_ref`,
`SYN038-IAP-OP-cast_stream__FM-const_ref`,
`SYN038-IAP-OP-assignment_pattern__FM-const_ref`, and
`SYN038-IAP-HC-subroutine__IN-runtime_declaration`.

Dedicated public-CLI witnesses use fixtures in
[`fixtures/sim/syn038_pairwise/`](fixtures/sim/syn038_pairwise/). Their positive
cases explicitly select `--edition 2009`, run with and without optimization,
and compare exact stdout and stderr oracles. Most use the shared CLI harness;
W66 uses a dedicated exact CLI helper to preserve its path-derived Slang
warning, W69 uses an explicit public-CLI loop with source-anchor checks, and
W87 directly constructs `llg` invocations with `--top tb` and an optional
`--no-opt` to retain the exact source-located initializer warnings. Edition
and runtime rejection controls remain in their owning suites.

- Process, storage, and formal-mode cases: `sim_syn038_array_processes`,
  `sim_syn038_latch_record`, `sim_syn038_interface_processes`,
  `sim_syn038_interface_record_events`, `sim_syn038_interface_record_inout`,
  `sim_syn038_interface_record_constref`, `sim_syn038_formal_process_matrix`,
  `sim_syn038_typed_formal_matrix`, `sim_syn038_selected_activation_lvalues`,
  and `sim_syn038_static_return_continuous`.
- Enum, record, and function-return cases: `sim_syn038_enum_auto_function`,
  `sim_syn038_enum_contexts`, `sim_syn038_packed_pattern_return_comb`,
  `sim_syn038_packed_return_ff`, `sim_syn038_record_auto_ref`,
  `sim_syn038_record_reduction_init`, `sim_syn038_return_slot_formals`,
  `sim_syn038_static_local_continuous`, `sim_syn038_localparam_member_reads`,
  `sim_syn038_localparam_runtime_index`, `sim_syn038_typed_initializer_matrix`,
  and `sim_syn038_typed_lvalue_matrix`.
- Operation and consumer/address contexts: `sim_syn038_op_consumer_matrix` and
  `sim_syn038_op_lvalue_address_matrix`; `sim_syn038_operation_context_matrix`
  also owns the four direct const-ref expression rejections and selected-element
  positive controls.
- Cross-context actual and lvalue storage paths:
  `sim_syn038_lvalue_context_matrix`.
- Typed source, aggregate-target, generated-lvalue, initializer-scope,
  hierarchy/process/call, and typed-scope remainders: `sim_syn038_typed_context_remainders`,
  `sim_syn038_typed_aggregate_lvalue_matrix`,
  `sim_syn038_lvalue_generate_scope_remainders`,
  `sim_syn038_scope_initializer_remainders`,
  `sim_syn038_hierarchy_process_call_remainders`, and
  `sim_syn038_typed_scope_remainders`.
- Typed constant-function source storage through elaboration: `sim_syn038_typed_constant_event_matrix`.
- Continuous assignment contexts: `sim_syn038_typed_continuous_matrix` and
  `sim_syn038_continuous_pattern_lhs`.
- Generated hierarchy and port cases: `sim_syn038_generate_actuals`,
  `sim_syn038_generate_enum_predicate`, `sim_syn038_generate_output_field`,
  `sim_syn038_generate_port`, and `sim_syn038_ref_port`.
- Hierarchical value and event contexts: `sim_syn038_hierarchical_value_matrix`.
- Event, interface-port, and interface-function source witnesses:
  `sim_syn038_subroutine_event_signal_matrix`,
  `sim_syn038_interface_member_child_port`,
  `sim_syn038_interface_function_source`, and
  `sim_syn038_interface_record_events`. W105's O595–O596 event/write paths are
  mapped by the focal-write and read-site rules.
- Scope and hierarchy routes: `sim_syn038_scope_hierarchy_matrix`.
- Call actual storage and hierarchy routes: `sim_syn038_call_storage_matrix` and
  `sim_syn038_call_provenance_matrix`.
- Same-root assignment RHS witnesses:
  `sim_syn038_co_same_root_assignment_rhs` checks the fixture output in both
  optimizer modes, maps O597–O601 and proves RHS-to-destination identity in the
  owned database. It closes `CO-assignment_rhs__LV-field` and
  `CO-assignment_rhs__LV-positional_pattern`.
- Process/value and child-parameter declaration-initializer cases:
  `sim_syn038_process_value_matrix` and
  `sim_syn038_child_param_decl_init`.
- Initializer-source storage paths: `sim_syn038_initializer_source_matrix` and
  `sim_syn038_interface_runtime_initializer`.
- Scope/storage/process contexts and remaining NBA write paths:
  `sim_syn038_scope_storage_process_matrix` and
  `sim_syn038_process_write_remainders` and
  `sim_syn038_storage_write_remainders`.
- Static return-slot reference actuals: `sim_syn038_static_return_ref_actual`.
- Lvalue storage and formal paths: `sim_syn038_lvalue_storage_formal_matrix`.
- Interface-member storage and selected writes: `sim_syn038_interface_lvalues`.
- Runtime-source local initializer cases: `sim_syn038_runtime_source_local_initializers`.
- Packed-union cases: `sim_syn038_union_constant`,
  `sim_syn038_union_function_port`, `sim_syn038_union_hier_ref`, and
  `sim_syn038_union_interface`.

#### R12 typed-value, initializer, formal, and source-context witnesses

The source axes below follow the v4 manifest; omitted axes are explicitly
`none`. Each row's oracle is checked in the optimized and `--no-opt` CLI modes.

| Witness | Source route and axes | Fixture and owner | Invocation and both-mode oracle |
| --- | --- | --- | --- |
| W63 | `TY=enum/packed_struct/untagged_packed_union`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=element/row_slice/concatenation`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/IN=none`; `WK=procedural_blocking/procedural_nba`; `PC=initial/always_comb/always_latch/always_ff` (O154–O162). | `tests/fixtures/sim/syn038_pairwise/typed_lvalue_matrix.sv`; `tests/sim_syn038_typed_lvalue_matrix.rs::typed_selected_lvalue_matrix_matches_in_both_optimizer_modes` | `syn038_pairwise/typed_lvalue_matrix`; `--edition 2009`; stdout `enum=01/a0/a5 struct=0100/a000/b5c2 union=0100/a000/d3a4\n`; stderr `""` |
| W64 | `TY=packed_struct/untagged_packed_union/integral_bit_logic`; `OP=assignment_pattern/cast_stream/equality_inside`; `CO=declaration_initializer`; `LV=whole_object`; `SL=module_package/static_local/automatic_local`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/WK=none`; `IN=constant_declaration/runtime_declaration/static_local/automatic_local`; `PC=none/initial` (O163–O174). | `tests/fixtures/sim/syn038_pairwise/typed_initializer_matrix.sv`; `tests/sim_syn038_typed_initializer_matrix.rs::typed_initializers_keep_their_constant_and_runtime_values` | `syn038_pairwise/typed_initializer_matrix`; `--edition 2009`; stdout `const=1234,b5c6,1 runtime=2143,c5d6,1 static=3153,d5e6,1 auto=4163,e5f6,1\n`; stderr `""` |
| W65 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=port_actual`; `LV=whole_object`; `SL=module_package`; `FM=ref`; `HC=module`; `HR=child_port`; `CP/CT/IN/WK/PC=none` (O175). | `tests/fixtures/sim/syn038_pairwise/ref_port.sv`; `tests/sim_syn038_ref_port.rs::module_ref_port_tracks_parent_and_child_updates_in_both_modes` | `syn038_pairwise/ref_port`; `--edition 2009`; stdout `refport=c3\n`; stderr `""` |
| W66 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=whole_object`; `SL=return_slot`; `FM=none`; `HC=module`; `HR=hierarchical_identifier`; `CP/CT/IN/PC=none`; `WK=continuous_variable` (O176). | `tests/fixtures/sim/syn038_pairwise/static_return_continuous.sv`; `tests/sim_syn038_static_return_continuous.rs::static_function_result_accepts_hierarchical_continuous_variable_assignment` | `syn038_pairwise/static_return_continuous`; `--edition 2009`; helper chain `assert_exact_cli` → `invoke` → `fixture_path`; stdout `result=1\n`; stderr `Warning: {absolute_fixture_path}:10:27 non-void function 'f' does not return a value\n` (Slang `NoReturnStatement`) |
| W67 | `TY=packed_struct/untagged_packed_union`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=none`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/IN/WK=none`; `PC=initial` (O177–O179; no new pair IDs). | `tests/fixtures/sim/syn038_pairwise/localparam_member_reads.sv`; `tests/sim_syn038_localparam_member_reads.rs::packed_localparam_members_lower_as_constant_runtime_reads` | `syn038_pairwise/localparam_member_reads`; `--edition 2009`; stdout `struct=12\nunion=a5c3\noctet=a5\n`; stderr `""` |
| W68 | `TY=enum/packed_struct/untagged_packed_union/fixed_array_record/unpacked_record`; `OP=direct_projection`; `CO=call_argument`; `LV=whole_object/none`; `SL=module_package`; `FM=output/inout/ref/const_ref`; `HC=module`; `HR=local`; `CP/IN/WK=none`; `CT=task`; `PC=initial` (O180–O190; 13 pair closures). | `tests/fixtures/sim/syn038_pairwise/typed_formal_matrix.sv`; `tests/sim_syn038_typed_formal_matrix.rs::typed_actuals_keep_formal_directions_distinct_from_storage` | `syn038_pairwise/typed_formal_matrix`; `--edition 2009`; stdout `enum=01,01,01 pair=5aa5 union=5aa5 records=11/a1,22/b2 inout=c3/44 ref=31/c7\n`; stderr `""` |
| W69 | `TY=integral_bit_logic`; `OP=conditional/equality_inside/cast_stream/assignment_pattern`; `CO=call_argument/function_return_statement/event_expression/port_actual/constant_elaboration/declaration_initializer`; `LV=none`; `SL=module_package/formal`; `FM=input/none`; `HC=module/subroutine`; `HR=local/child_port`; `CP=none`; `CT=function/none`; `IN=none/runtime_declaration`; `WK=none`; `PC=initial/always/none` (O191–O203; 28 pair closures). | `tests/fixtures/sim/syn038_pairwise/op_consumer_matrix.sv`; `tests/sim_syn038_op_consumer_matrix.rs::expression_consumers_keep_distinct_contexts_in_both_cli_modes` | `syn038_pairwise/op_consumer_matrix`; `--edition 2009`; explicit public-CLI runs with and without optimization; stdout `calls=18,0,18,18 events=1,1,1 widths=5,7,2\n`; stderr `""` |
| W70 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs/declaration_initializer`; `LV=element/row_slice/concatenation/positional_pattern/whole_object`; `SL=interface_member`; `FM=none`; `HC=interface`; `HR=local`; `CP/CT=none`; `IN=none/runtime_declaration`; `WK=procedural_blocking/none`; `PC=initial/none` (O204–O208). | `tests/fixtures/sim/syn038_pairwise/interface_lvalues.sv`; `tests/sim_syn038_interface_lvalues.rs::interface_member_lvalues_and_initializer_keep_separate_readbacks` | `syn038_pairwise/interface_lvalues`; `--edition 2009`; stdout `interface=01,a0,b2,01 seeded=12 lanes=34,56\n`; stderr `""` |
| W71 | `TY=integral_bit_logic/enum/fixed_array_integral`; `OP=direct_projection`; `CO=declaration_initializer`; `LV=none`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/WK=none`; `IN=static_local/automatic_local`; `PC=none/initial` (O209–O214). | `tests/fixtures/sim/syn038_pairwise/runtime_source_local_initializers.sv`; `tests/sim_syn038_runtime_source_local_initializers.rs::runtime_sources_initialize_static_and_automatic_locals` | `syn038_pairwise/runtime_source_local_initializers`; `--edition 2009`; stdout `runtime-local-init=39,39 color=5c,5c lanes=39,4a:39,4a\n`; stderr `llg: simulation ended without $finish (no processes remain) at time 0\n` |
| W72 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=element/row_slice/concatenation`; `SL=formal/static_local/automatic_local/return_slot`; `FM=none`; `HC=module/subroutine`; `HR=local`; `CP/CT/IN=none`; `WK=procedural_blocking`; `PC=initial` (O215–O226; 13 pair closures). Immediate assertions check every intermediate write. | `tests/fixtures/sim/syn038_pairwise/selected_activation_lvalues.sv`; `tests/sim_syn038_selected_activation_lvalues.rs::selected_formal_local_and_return_lvalues_match_in_both_optimizer_modes` | `syn038_pairwise/selected_activation_lvalues`; `--edition 2009`; stdout `local=b6,b7 formal=b4 return=b5\n`; stderr `""` |
| W73 | `TY=integral_bit_logic/enum/packed_struct/untagged_packed_union/unpacked_record/fixed_array_record`; `OP=direct_projection/conditional/equality_inside/cast_stream/assignment_pattern`; `CO=assignment_rhs`; `LV=whole_object/field/concatenation/row_slice/element`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/IN/PC=none`; `WK=continuous_net/continuous_variable` (O227–O249; 27 pair closures). Whole-record and array fields/elements have independent assertions; self-source expressions read bits disjoint from continuous targets. | `tests/fixtures/sim/syn038_pairwise/typed_continuous_matrix.sv`; `tests/sim_syn038_typed_continuous_matrix.rs::typed_continuous_assignments_match_with_and_without_optimization` | `syn038_pairwise/typed_continuous_matrix`; `--edition 2009`; both optimization modes; stdout `operation=03,03,03,03,03,05,05\nlvalue=1234,1234,b2,b3,c6\ncont=01,01,01,1234,1234,a5c3,a5c3,56,78,11,44\n`; stderr `""` |
| W74 | `TY=packed_struct/untagged_packed_union`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=none`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/IN/WK=none`; `PC=initial` (O250–O260; adds zero pair IDs). Qualifiers cover ascending and descending packed ranges at both valid endpoints, signed and unsigned selectors, selected Z, unknown and out-of-bounds indices, and a packed-record neighboring-field sentinel. | `tests/fixtures/sim/syn038_pairwise/localparam_runtime_index.sv`; `tests/sim_syn038_localparam_runtime_index.rs::localparam_packed_member_runtime_indices_preserve_four_state_selection` | `syn038_pairwise/localparam_runtime_index`; `--edition 2009`; both optimization modes; stdout `valid=a5/c3 asc=a5/c3 signed=c3 unsigned=xx z=z3 unknown=xx out=xx/xx/xx\n`; stderr `""` |
| W75 | `TY=enum/packed_struct/fixed_array_integral/unpacked_record/fixed_array_record/integral_bit_logic`; `OP=direct_projection/conditional/equality_inside/cast_stream/assignment_pattern`; `CO=assignment_rhs/port_actual/function_return_statement/event_expression`; `LV=none/field/element/row_slice/concatenation/positional_pattern`; `SL=module_package`; `FM=input/none`; `HC=module/subroutine`; `HR=hierarchical_identifier`; `CP/CT/IN=none`; `WK=none/procedural_blocking`; `PC=initial/always/none` (O261–O275; historical 22 pair closures). `CO=assignment_rhs` is supported by source-consumer O261–O267; target-only writes O271–O275 are under the broad CO audit and are not asserted as RHS consumers. | `tests/fixtures/sim/syn038_pairwise/hierarchical_value_matrix.sv`; `tests/sim_syn038_hierarchical_value_matrix.rs::hierarchical_child_values_and_selected_writes_execute` | `syn038_pairwise/hierarchical_value_matrix`; `--edition 2009`; both optimization modes; stdout `hier=01,1,12,56,21,33,56,12,53 event=1\n`; stderr `""` |
| W76 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=call_argument`; `LV=none/whole_object`; `SL=static_local/automatic_local/formal/return_slot/interface_member/module_package`; `FM=input/output/inout/ref/const_ref`; `HC=subroutine/interface/module`; `HR=local/interface_member/hierarchical_identifier`; `CP/IN/WK=none`; `CT=function/task`; `PC=initial` (O276–O300; 35 pair closures). Immediate assertions independently check call results, copy-out, and aliases. `iface_actual` is static local at a qualified child task call; `hier_const_value` is module storage at a qualified child function call. Unqualified calls on concrete module interface members use the interface-member route. | `tests/fixtures/sim/syn038_pairwise/call_storage_matrix.sv`; `tests/sim_syn038_call_storage_matrix.rs::call_actual_storage_and_routes_match_in_both_optimizer_modes` | `syn038_pairwise/call_storage_matrix`; `--edition 2009`; both optimization modes; stdout `calls=12,43,13,44,25,2d,55 iface=11,41,13,2c,42,12 hier=33,6f\n`; stderr `""` |
| W77 | `TY=integral_bit_logic/enum/packed_struct/untagged_packed_union/fixed_array_integral/unpacked_record/fixed_array_record`; `OP=direct_projection/conditional/equality_inside/cast_stream/assignment_pattern`; `CO=assignment_rhs`; `LV=none/whole_object/field/element/row_slice/concatenation/positional_pattern`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/IN=none`; `WK=none/procedural_blocking/procedural_nba`; `PC=always/always_comb/always_latch/always_ff` (O301–O324; 26 selected pair gains). Assertions cover each process result, including NBA values after commit. | `tests/fixtures/sim/syn038_pairwise/process_value_matrix.sv`; `tests/sim_syn038_process_value_matrix.rs::process_values_match_across_optimizer_modes` | `syn038_pairwise/process_value_matrix`; `--edition 2009`; both optimization modes; stdout `process=12,abcd,55,a,b2,01 c3,01,12/34,11/22/33/44,a5,1,25 d4,01,01,55/66,1,25,a5 e,01,12/34,1\n`; stderr `""` |
| W78 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs/declaration_initializer`; `LV=none`; `SL=module_package`; `FM=none`; `HC=module`; `HR=hierarchical_identifier`; `CP/CT=none`; `IN=none/runtime_declaration`; `WK=none`; `PC=initial/none` (O325–O328; closes `SYN038-GAP-CO-declaration_initializer__HR-hierarchical_identifier` and `SYN038-GAP-HR-hierarchical_identifier__IN-runtime_declaration`). The three declaration observations cover a fixed child and two siblings with distinct overrides; O328 is a procedural control. | `tests/fixtures/sim/syn038_pairwise/child_param_decl_init.sv`; `tests/sim_syn038_child_param_decl_init.rs::child_parameter_initializers_keep_instance_identity_in_both_modes` | `syn038_pairwise/child_param_decl_init`; `--edition 2009`; both optimization modes; stdout `decl=05 siblings=05/0a procedural=05\n`; stderr `""` |
| W79 | `TY=fixed_array_integral` (omitted for O332/O336 under the sparse outer-slot rule); `OP=conditional/equality_inside/cast_stream/assignment_pattern`; `CO` omitted for all O329–O342 under the accepted sparse target-address basis; `LV=element/field/row_slice/concatenation/positional_pattern`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/IN=none`; `WK=procedural_blocking`; `PC=initial` (O329–O342; 18 distinct gains: 14 `OP × LV` plus four `TY × OP/LV`). O332/O336 target elements of an unpacked array of packed records, which has no matching outer `TY` level; the operation computes the array index and the RHS is a separate value source. | `tests/fixtures/sim/syn038_pairwise/op_lvalue_address_matrix.sv`; `tests/sim_syn038_op_lvalue_address_matrix.rs::operation_lvalues_use_the_selected_address_in_both_cli_modes` | `syn038_pairwise/op_lvalue_address_matrix`; `--edition 2009`; both optimization modes; stdout `conditional=a2,b6,0 equality=a5,b4,1 cast=b6,b4,1 pattern=10110110,d4,1\n`; stderr `""` |
| W80 | `TY=packed_struct/fixed_array_integral/integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs/port_actual/call_argument/declaration_initializer/function_return_statement`; `LV=field/element/row_slice/concatenation/positional_pattern/whole_object/none`; `SL=module_package/interface_member/static_local/automatic_local/return_slot`; `FM=none/output`; `HC=module/interface/subroutine`; `HR=local/interface_member/child_port`; `CP=none/function`; `CT=none/function/task`; `IN=none/runtime_declaration/constant_declaration/static_local/automatic_local`; `WK=none/procedural_blocking/procedural_nba`; `PC=always/always_comb/always_latch/always_ff/initial/none` (O343–O374; 31 selected pair gains). O370–O372 close `SL=static_local/interface_member/return_slot × IN=automatic_local`. | `tests/fixtures/sim/syn038_pairwise/lvalue_context_matrix.sv`; `tests/sim_syn038_lvalue_context_matrix.rs::selected_lvalue_contexts_keep_actual_storage_and_process_paths_distinct` | `syn038_pairwise/lvalue_context_matrix`; `--edition 2009`; both optimization modes; stdout `proc=11,22,3,45,01 comb=89,80 latch=cd,81 ff=5,01 if=31,4,56,01 call=c1,d2,6,7,e3,f4 child=9a init=23,34,45,56,12 extra=45,12,78,67,12\n`; stderr `""` |
| W81 | `TY=integral_bit_logic`; `OP=assignment_pattern`; `CO=assignment_rhs`; `LV=positional_pattern`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/IN=none`; `WK=continuous_net/continuous_variable`; `PC=none` (O375–O377; closes `LV=positional_pattern × WK=continuous_net`, `LV=positional_pattern × WK=continuous_variable`, and `LV=positional_pattern × PC=none`). A resolved-net conflict then known value, selected-wire and pure-variable checks verify the written bits; a counted-function RHS control verifies sensitivity. | `tests/fixtures/sim/syn038_pairwise/continuous_pattern_lhs.sv`; `tests/sim_syn038_continuous_pattern_lhs.rs::continuous_pattern_lvalues_preserve_targets_and_rhs_sensitivity` | `syn038_pairwise/continuous_pattern_lhs`; `--edition 2009`; both optimization modes; stdout `rhs_eval\nrhs_eval\nnet=01 selected_net=01 pure_variable=01 counted_variable=01\n`; stderr `""` |
| W82 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=declaration_initializer`; `LV=none`; `SL=automatic_local/formal/return_slot/static_local/interface_member`; `FM=none`; `HC=module`; `HR=local/interface_member`; `CP=function/none`; `CT=none`; `IN=constant_declaration/runtime_declaration/static_local/automatic_local`; `WK=none`; `PC=none/initial` (O378–O393; 18 distinct pair gains). Static declarations inside the initial block retain `PC=none`; automatic declarations execute in `initial`. | `tests/fixtures/sim/syn038_pairwise/initializer_source_matrix.sv`; `tests/sim_syn038_initializer_source_matrix.rs::declaration_initializers_read_each_source_storage_kind` | `syn038_pairwise/initializer_source_matrix`; `--edition 2009`; optimized and `--no-opt`; stdout `automatic=11,21,31,41 formal=52,62,72 return=84,94,a4,b4 static=01,01,02 interface=00,00,d5\n`; stderr `""` |
| W83 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs/call_argument/declaration_initializer/event_expression/function_return_statement`; `LV=none/whole_object`; `SL=automatic_local/formal/interface_member/module_package/return_slot/static_local`; `FM=input/none`; `HC=generate/interface/module/subroutine`; `HR=local`; `CP=none`; `CT=function/none`; `IN=automatic_local/none`; `WK=continuous_net/continuous_variable/none/procedural_blocking/procedural_nba`; `PC=always/always_comb/always_latch/always_ff/initial/none` (O394–O423; 34 net gains after one pair shared with W82). Interface and generated writes, automatic/static storage, function actual/return paths, constant-function evaluation, and event sources are checked in their owning scopes and processes. | `tests/fixtures/sim/syn038_pairwise/scope_storage_process_matrix.sv`; `tests/sim_syn038_scope_storage_process_matrix.rs::storage_and_process_contexts_keep_distinct_source_paths_observable` | `syn038_pairwise/scope_storage_process_matrix`; `--edition 2009`; optimized and `--no-opt`; stdout `scope=5b,5b,5b,5b,5b,5a,5b event=5a gen=1,1 if=1,1 const=5\n`; stderr `""` |
| W84 | `TY=integral_bit_logic`; `OP=assignment_pattern/cast_stream/direct_projection/equality_inside`; `CO=assignment_rhs/call_argument`; `LV=none/whole_object`; `SL=module_package`; `FM=const_ref/none`; `HC=module`; `HR=local`; `CP=none`; `CT=function/none`; `IN=none`; `WK=none/procedural_nba`; `PC=always_comb/always_latch/none` (O424–O429; six net gains after two pairs shared with W83). Equality, `inside`, cast, and pattern RHSs feed whole-object NBA destinations in `always_comb`; a latch NBA and separate continuous const-ref call complete the source controls. | `tests/fixtures/sim/syn038_pairwise/process_write_remainders.sv`; `tests/sim_syn038_process_write_remainders.rs::remaining_process_write_paths_match_in_both_cli_modes` | `syn038_pairwise/process_write_remainders`; `--edition 2009`; optimized and `--no-opt`; stdout `ops=0,0,21,21 comb=24 latch=25 const_ref=a1\n`; stderr `""` |
| W85 | `TY=enum/fixed_array_integral/fixed_array_record/integral_bit_logic/unpacked_record`; `OP=assignment_pattern/cast_stream/conditional/direct_projection/equality_inside`; `CO=assignment_rhs/declaration_initializer/event_expression`; `LV=none`; `SL=interface_member/module_package`; `FM=none`; `HC=generate/interface/module/subroutine`; `HR=hierarchical_identifier/interface_member`; `CP/CT/WK=none`; `IN=automatic_local/none/static_local`; `PC=always/always_comb/always_ff/always_latch/initial/none` (O430–O449; 27 distinct selected-pair gains). Interface members and qualified child values retain separate route identities across process families and declaration initialization; static interface defaults are read before later process writes. | `tests/fixtures/sim/syn038_pairwise/scope_hierarchy_matrix.sv`; `tests/sim_syn038_scope_hierarchy_matrix.rs::source_routes_keep_their_lexical_scope_and_storage_identity` | `syn038_pairwise/scope_hierarchy_matrix`; `--edition 2009`; optimized and `--no-opt`; stdout `scope=if:10,6b,00 gen:20,a7 module:10 types:5c,22,88,44 op:a7,0,05,e5 return:a7 init:1000a700 event=1 latch=a7,10 ff=10\n`; stderr `""` |
| W86 | `TY=fixed_array_integral/integral_bit_logic`; `OP=assignment_pattern/cast_stream/conditional/direct_projection/equality_inside`; `CO=assignment_rhs/call_argument/constant_elaboration/declaration_initializer/event_expression/function_return_statement/port_actual`; `LV=none`; `SL=interface_member/module_package/return_slot/static_local`; `FM=input/none`; `HC=module/subroutine`; `HR=child_port/hierarchical_identifier/interface_member/local`; `CP=fixed_array_reduction/function`; `CT=function/none/task`; `IN=constant_declaration/none/runtime_declaration/static_local`; `WK=none`; `PC=always/always_comb/always_ff/always_latch/initial/none` (O450–O475; 42 selected-pair gains). Function return slots and fixed-array reduction receivers are the focal producers; later destinations and receiving formals are separate checked slots. | `tests/fixtures/sim/syn038_pairwise/call_provenance_matrix.sv`; `tests/sim_syn038_call_provenance_matrix.rs::call_provenance_paths_match_in_both_optimizer_modes` | `syn038_pairwise/call_provenance_matrix`; `--edition 2009`; optimized and `--no-opt`; stdout `calls=13,1,19,21,18 task=13,12 routes=12,12,5a,12 reductions=6,1,6,12,05,05,0d,09 processes=12,05,05,1 parameter=0 overrides=51,84\n`; stderr `""` |
| W87 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=declaration_initializer`; `LV=none`; `SL=interface_member`; `FM=none`; `HC=module`; `HR=interface_member`; `CP/CT/WK=none`; `IN=runtime_declaration`; `PC=none` (O476; one selected-pair gain). A module initializer reads an uninitialized two-state interface member before a later process write; a same-scope zero-default control avoids declaration-order assumptions. | `tests/fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv`; `tests/sim_syn038_interface_runtime_initializer.rs::module_initializer_reads_bound_interface_storage_in_both_modes` | Fixture `syn038_pairwise/interface_runtime_initializer`; direct public CLI `llg --top tb [--no-opt] --edition 2009 {absolute_fixture_path}` in both modes | stdout `copy=00,source=5a,control=00\n`; stderr `Warning: {absolute_fixture_path}:9:35 initializer for static variable 'same_scope_copy' refers to 'local_seed' which will not have a value at initialization time\nWarning: {absolute_fixture_path}:10:24 initializer for static variable 'copy' refers to 'value' which will not have a value at initialization time\n` |
| W88 | `TY=enum/fixed_array_record/integral_bit_logic/packed_struct`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=field/whole_object`; `SL=formal/return_slot/static_local`; `FM=none`; `HC=module/subroutine`; `HR=hierarchical_identifier/local`; `CP/CT/IN=none`; `WK=continuous_net/procedural_blocking/procedural_nba`; `PC=initial/none` (O477–O484; eight selected-pair gains). Output formals, static locals, and return slots are kept distinct from caller actuals; assertions check whole and field writes and NBA commits. | `tests/fixtures/sim/syn038_pairwise/storage_write_remainders.sv`; `tests/sim_syn038_storage_write_remainders.rs::storage_write_remainder_cells_run_in_both_optimizer_modes` | `syn038_pairwise/storage_write_remainders`; `--edition 2009`; optimized and `--no-opt`; stdout `storage_write_remainders=passed\n`; stderr `Warning: {absolute_fixture_path}:101:33 non-void function 'hier_result' does not return a value\nllg: $finish at time 4000 at tb:153:9\n` |
| W89 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=call_argument`; `LV=whole_object`; `SL=return_slot`; `FM=ref`; `HC=module`; `HR=hierarchical_identifier`; `CP=none`; `CT=task`; `IN=none`; `WK=none`; `PC=initial` (O485; zero new pair gains). The actual is the hierarchical static function result variable, not a function-call expression; the task write and function readback observe the same alias. | `tests/fixtures/sim/syn038_pairwise/static_return_ref_actual.sv`; `tests/sim_syn038_static_return_ref_actual.rs::hierarchical_static_function_return_slot_binds_to_task_ref_formal` | `syn038_pairwise/static_return_ref_actual`; `--edition 2009`; optimized and `--no-opt`; stdout `static-return-ref=passed\n`; stderr `llg: $finish at time 0 at tb:18:9\n` |
| W90 | `TY=fixed_array_integral/integral_bit_logic/unpacked_record/untagged_packed_union`; `OP=direct_projection`; `CO=assignment_rhs/call_argument/port_actual`; `LV=concatenation/field/positional_pattern/row_slice`; `SL=automatic_local/formal/module_package/return_slot/static_local`; `FM=inout/none/output/ref`; `HC=module/subroutine`; `HR=child_port/local`; `CP=function/none`; `CT=function/none/task`; `IN=none`; `WK=none/procedural_blocking`; `PC=initial/none` (O486–O502; 23 selected-pair gains). Output/inout actual patterns, selected dynamic array addresses, distinct task/function formals and local/return slots, packed-union targets, and packed-slice module `ref` actuals have independent checks; O502 covers `LV=row_slice × FM=ref`. | `tests/fixtures/sim/syn038_pairwise/lvalue_storage_formal_matrix.sv`; `tests/sim_syn038_lvalue_storage_formal_matrix.rs::lvalue_storage_and_formal_paths_preserve_selected_values` | `syn038_pairwise/lvalue_storage_formal_matrix`; `--edition 2009`; optimized and `--no-opt`; stdout `union=1234,5678 port=10 task=10 function=10:01 dynamic=10/1,1 direct=10 concat=10 inout=11 concat_inout=11 concat_fn=4142 field=61,62 row=31,32 formal=11,12 static=21,22 automatic=23,24 return=10 task_local=92 ref=a0 ref_field=7a\n`; stderr `""` |
| W91 | `TY=enum/fixed_array_record/packed_struct/unpacked_record/untagged_packed_union`; `OP=assignment_pattern/cast_stream/conditional/equality_inside`; `CO=assignment_rhs/constant_elaboration/declaration_initializer/event_expression/function_return_statement`; `LV=none/whole_object`; `SL=module_package/return_slot/static_local`; `FM=none`; `HC=module/subroutine`; `HR=local`; `CP=none`; `CT=none`; `IN=constant_declaration/none/runtime_declaration/static_local`; `WK=none`; `PC=always/initial/none` (O503–O519; 17 selected-pair gains). Typed values cover operation sources, constants, declaration initialization, event comparisons and function returns with separate full-width readbacks. | `tests/fixtures/sim/syn038_pairwise/typed_context_remainders.sv`; `tests/sim_syn038_typed_context_remainders.rs::typed_context_remainders_keep_exact_values_in_both_optimizer_modes` | `syn038_pairwise/typed_context_remainders`; `--edition 2009`; stdout `const=2,5,6,4,2 op=1234,0,2143,11223344,556677,1 init=5a6b,21,11,55,77,64 target=20,60,81,92 events=1,1 row=3344 override=62,82,43 extra=1,0,1,c0de\n`; stderr `""` |
| W92 | `TY=enum/fixed_array_record/unpacked_record`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=concatenation/positional_pattern/row_slice`; `SL=module_package`; `FM=none`; `HC=module`; `HR=local`; `CP/CT/IN=none`; `WK=procedural_blocking`; `PC=initial` (O520–O525; six selected-pair gains). Record-array and unpacked-record selected targets and enum positional targets are checked immediately, including neighboring fields and elements. | `tests/fixtures/sim/syn038_pairwise/typed_aggregate_lvalue_matrix.sv`; `tests/sim_syn038_typed_aggregate_lvalue_matrix.rs::typed_aggregate_lvalue_matrix_matches_exact_oracle_in_both_optimizer_modes` | `syn038_pairwise/typed_aggregate_lvalue_matrix`; `--edition 2009`; stdout `records=20,21,22,23 payload=70,71,61 enum=1,2\n`; stderr `""` |
| W93 | `TY=fixed_array_integral/integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=concatenation/positional_pattern/row_slice`; `SL=module_package`; `FM=none`; `HC=generate`; `HR=local`; `CP/CT/IN=none`; `WK=procedural_blocking`; `PC=initial` (O526–O528; three selected-pair gains). Generated initial processes check row-slice, concatenation and positional-pattern destinations. | `tests/fixtures/sim/syn038_pairwise/lvalue_generate_scope_remainders.sv`; `tests/sim_syn038_lvalue_generate_scope_remainders.rs::generated_scope_lvalues_write_the_selected_objects` | `syn038_pairwise/lvalue_generate_scope_remainders`; `--edition 2009`; stdout `generated_row=31,32 generated_concat=4142 generated_pattern=01\n`; stderr `""` |
| W94 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=declaration_initializer`; `LV=whole_object`; `SL=automatic_local/module_package/static_local`; `FM=none`; `HC=generate/interface/subroutine`; `HR=local`; `CP/CT=none`; `IN=automatic_local/constant_declaration/runtime_declaration/static_local`; `WK=none`; `PC=initial/none` (O529–O536; 13 selected-pair gains). Subroutine, generate and interface initializer sites distinguish constant, runtime, automatic and static declaration phases. | `tests/fixtures/sim/syn038_pairwise/scope_initializer_remainders.sv`; `tests/sim_syn038_scope_initializer_remainders.rs::scope_initializer_remainders_match_in_both_cli_modes` | `syn038_pairwise/scope_initializer_remainders`; `--edition 2009`; stdout `subroutine=a1\ngenerate=11,22,33,44\ninterface=51,62,73\n`; stderr `""` |
| W95 | `TY=fixed_array_integral/integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs/constant_elaboration/port_actual`; `LV=none/whole_object`; `SL=formal/module_package/return_slot`; `FM=input/none`; `HC=generate/interface/subroutine`; `HR=child_port/local`; `CP=fixed_array_reduction/function/none`; `CT=none`; `IN=none`; `WK=none/procedural_blocking`; `PC=always_latch/initial/none` (O537–O543; 10 selected-pair gains). Generated reduction, interface function result, latch target, nested interface child-port actual and lexical package-constant widths each have distinct checks. | `tests/fixtures/sim/syn038_pairwise/hierarchy_process_call_remainders.sv`; `tests/sim_syn038_hierarchy_process_call_remainders.rs::hierarchy_process_call_remainders_match_in_both_cli_modes` | `syn038_pairwise/hierarchy_process_call_remainders`; `--edition 2009`; stdout `reduction=07 function=a6 latch=91 child_port=5c const=5,7,2a\n`; stderr `""` |
| W96 | `TY=enum/fixed_array_integral/packed_struct/unpacked_record/untagged_packed_union`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=none`; `SL=interface_member/module_package`; `FM=none`; `HC=generate/interface`; `HR=local`; `CP/CT/IN/WK=none`; `PC=always_comb` (O544–O548; five selected-pair gains). Complete typed sources are copied to same-type destinations in interface/generate scopes and checked after settling. | `tests/fixtures/sim/syn038_pairwise/typed_scope_remainders.sv`; `tests/sim_syn038_typed_scope_remainders.rs::typed_values_keep_their_outer_type_in_interface_and_generate_scopes` | `syn038_pairwise/typed_scope_remainders`; `--edition 2009`; stdout `tyhc=a,2b,c,5a,d\n`; stderr `""` |
| W97 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=constant_elaboration`; `LV=none`; `SL=automatic_local/formal/static_local`; `FM=none`; `HC=module`; `HR=local`; `CP=function`; `CT/IN/WK/PC=none` (O549–O551; three selected-pair gains). Automatic local, static local and formal source values flow through separate constant-function results into packed typedef dimensions. | `tests/fixtures/sim/syn038_pairwise/typed_constant_event_matrix.sv`; `tests/sim_syn038_typed_constant_event_matrix.rs::typed_constant_event_paths_keep_exact_values_in_both_optimizer_modes` | `syn038_pairwise/typed_constant_event_matrix`; `--edition 2009`; stdout `widths=3,5,5,4 constfunc=5,6,7 const=4/12,3/0d runtime=5/17,4/0b static=6/1a,2/09 return=5aa5 overrides=2/1a,7/03,4/12 nested=51,62,73,84,31,42 events=1,1\n`; stderr `""` |
| W98 | `TY=untagged_packed_union/enum/integral_bit_logic`; `OP=assignment_pattern/direct_projection/conditional/cast_stream/equality_inside`; `CO=assignment_rhs/call_argument/constant_elaboration/declaration_initializer/event_expression`; `LV=none`; `SL=module_package/static_local/automatic_local/formal/interface_member`; `FM=none/input`; `HC=module/subroutine/generate/interface`; `HR=local/hierarchical_identifier`; `CP=none`; `CT=none/function/task`; `IN=none/constant_declaration/automatic_local/static_local`; `WK=none`; `PC=initial/none/always_comb` (O552–O572; 21 selected-pair gains). Index-only operators in const-ref actuals keep the focal source `direct_projection`; the four direct expression actuals have separate rejection fixtures. | `tests/fixtures/sim/syn038_pairwise/operation_context_matrix.sv` plus `constref_{conditional,equality,cast,pattern}_rejected.sv`; `tests/sim_syn038_operation_context_matrix.rs::typed_operation_contexts_keep_source_and_use_site_in_both_cli_modes` | `syn038_pairwise/operation_context_matrix`; `--top tb --edition 2009`; optimized and `--no-opt`; stdout `types=1122,0,0,11223344,5162 task=25,0,25,2534 formal=2534 constref=34,34,34,34,5a locals=25,25,25,25,25 snapshot=00,00 constant=22,3 generate=25,25,2534 interface=25,25,2534\nunion-pattern=12,34\nfunction-types=2,56 event=1\n`; stderr `""`. Each direct-expression const-ref negative returns 1 with empty stdout and the exact §13.5.2 diagnostic in both modes. |
| W99 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=call_argument` on O576/O577 and omitted on target-only O573–O575; `LV=whole_object`; `SL=module_package/interface_member/automatic_local/formal`; `FM=none/ref`; `HC=module/subroutine`; `HR=hierarchical_identifier/interface_member`; `CP=none`; `CT=none/task`; `IN=none`; `WK=continuous_net/continuous_variable/none`; `PC=none/initial` (five accepted route/storage gains; O573–O575 contribute no `CO` gain). O573–O575 check separate target values across source phases; O576/O577 classify the qualified call-argument address path, with subsequent ref-alias readback as a separate oracle. | `tests/fixtures/sim/syn038_pairwise/storage_hier_route_remainders.sv`; `tests/sim_syn038_storage_hier_route_remainders.rs::storage_and_hierarchical_routes_run_in_both_optimizer_modes` | `syn038_pairwise/storage_hier_route_remainders`; `--edition 2009`; optimized and `--no-opt`; stdout `storage_hier_route_remainders=passed\n`; stderr `llg: $finish at time 2000 at tb:146:9\n` |
| W100 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=port_actual`; `LV=none`; `SL=static_local`; `FM=input`; `HC=module`; `HR=hierarchical_identifier`; `CP/CT/IN/WK/PC=none` (O578; one selected-pair gain). A block-local static source is named through an explicit hierarchy in a child input actual. | `tests/fixtures/sim/syn038_pairwise/static_hierarchical_block_port_actual.sv`; `tests/sim_syn038_static_hierarchical_block_port_actual.rs::static_block_local_hierarchical_port_actual_runs_in_both_optimizer_modes` | `syn038_pairwise/static_hierarchical_block_port_actual`; `--edition 2009`; optimized and `--no-opt`; stdout `static_hierarchical_block_port_actual=passed\n`; stderr `llg: $finish at time 1000 at tb:32:9\n` |
| W101 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=port_actual`; `LV=none`; `SL=interface_member`; `FM=input`; `HC=interface`; `HR=child_port`; `CP/CT/IN/WK/PC=none` (O579; one selected-pair gain). The nested child interface sees the parent's mutable member through the port link. | `tests/fixtures/sim/syn038_pairwise/interface_member_child_port.sv`; `tests/sim_syn038_interface_member_child_port.rs::mutable_interface_member_drives_nested_child_port_readback_in_both_modes` | `syn038_pairwise/interface_member_child_port`; `--edition 2009`; optimized and `--no-opt`; stdout `interface-child-port=a5,a5\n`; stderr `""` |
| W102 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=assignment_rhs`; `LV=none`; `SL=interface_member`; `FM=none`; `HC=module`; `HR=interface_member`; `CP=function`; `CT/IN/WK=none`; `PC=initial` (O580; two selected-pair gains). The function reads `bus.member`; distinct caller samples observe it and the sibling interface instance is a control. | `tests/fixtures/sim/syn038_pairwise/interface_function_source.sv`; `tests/sim_syn038_interface_function_source.rs::mutable_interface_member_source_flows_through_function_result_in_both_modes` | `syn038_pairwise/interface_function_source`; `--edition 2009`; optimized and `--no-opt`; stdout `member_through_function=31,6b sibling=92\n`; stderr `llg: $finish at time 0 at tb:35:9\n` |
| W103 | `TY=untagged_packed_union/fixed_array_record/integral_bit_logic`; `OP=conditional/equality_inside/cast_stream/assignment_pattern`; `CO=call_argument/assignment_rhs` on source-side rows, omitted for target-address O582–O585; `LV=none/field/element`; `SL=return_slot/module_package`; `FM=input/none`; `HC=module`; `HR=local`; `CP=function/none`; `CT=function/none`; `IN=none`; `WK=none/procedural_nba/continuous_net`; `PC=initial/always_ff/none` (O581–O587; ten accepted pair gains). The four address operators select LHS fields; the RHS literals are separate sources. | `tests/fixtures/sim/syn038_pairwise/legacy_op_replacements.sv`; `tests/sim_syn038_legacy_op_replacements.rs::selected_write_addresses_and_source_only_paths_have_independent_oracles` | `syn038_pairwise/legacy_op_replacements`; `--edition 2009`; optimized and `--no-opt`; stdout `conditional=a5,a5 equality=b6,b6 cast=c7,c7 pattern=d8,d8 net=05 union=5678,1234 source=11,22\n`; stderr `""` |
| W104 | `TY=packed_struct/fixed_array_integral/integral_bit_logic`; `OP=direct_projection/conditional`; `CO=assignment_rhs`; `LV=none/element/whole_object`; `SL=return_slot/module_package`; `FM=none`; `HC=module/subroutine`; `HR=local`; `CP=function`; `CT/IN=none`; `WK=none/procedural_blocking/continuous_net/continuous_variable/procedural_nba`; `PC=initial/none/always_comb/always_ff` (O588–O594; seven net selected-pair gains after overlap). Function result sources are focal at the return/source slot; caller/readback samples remain distinct. Focal review confirms O591/O592/O594 are same-focal RHS reads that retain `CO=assignment_rhs`. | `tests/fixtures/sim/syn038_pairwise/legacy_cp_replacements.sv`; `tests/sim_syn038_legacy_cp_replacements.rs::function_source_replacement_witnesses_match_in_both_optimizer_modes` | `syn038_pairwise/legacy_cp_replacements`; `--edition 2009`; optimized and `--no-opt`; stdout `initial=12/34 array=12,34,56 conditional=21,43 net=5a function=5a direct=5a var=7d function=7d direct=7d comb=6b/6b\nupdated=net=a5 function=a5 direct=a5 var=c6 function=c6 direct=c6 comb=7c/7c\nff=11->22 sample=11\nff=22->33 sample=22\n`; stderr `""` |
| W105 | `TY=integral_bit_logic`; `OP=direct_projection`; `CO=event_expression`; `LV=whole_object`; `SL=module_package/formal`; `FM=none`; `HC=subroutine`; `HR=local`; `CP/CT/IN=none`; `WK=procedural_blocking`; `PC=initial`. The fixture performs blocking writes to both observed sources; O595–O596 are source-bound write/event observations covered by the focal-write and read-site rules. | `tests/fixtures/sim/syn038_pairwise/subroutine_event_signal_matrix.sv`; `tests/sim_syn038_subroutine_event_signal_matrix.rs::subroutine_event_sources_observe_transitions_in_both_cli_modes` | `syn038_pairwise/subroutine_event_signal_matrix`; `--edition 2009`; optimized and `--no-opt`; stdout `module=1 formal=1\n`; stderr `""` |
| W106 | O597–O601: same-root whole, field, concatenation, positional-pattern, and NBA assignment RHSs; `CO=assignment_rhs` follows literal same-focal reads. | `tests/fixtures/sim/syn038_pairwise/co_same_root_assignment_rhs.sv`; `tests/sim_syn038_co_same_root_assignment_rhs.rs::same_root_assignment_rhs_witnesses_match_in_both_optimizer_modes`; DB: `slang_binds_each_assignment_rhs_to_its_own_destination_declaration` | `syn038_pairwise/co_same_root_assignment_rhs`; `--top tb --edition 2009`; stdout `same_root=11,21,69,01,31\n`; stderr `""`; DB proves RHS/LHS root identity. Pair keys: `CO-assignment_rhs__LV-field`, `CO-assignment_rhs__LV-positional_pattern`. |
| W107 | O602–O606: whole, element, row-slice, concatenation and pattern writes followed by reduction reads of the same fixed array. | `tests/fixtures/sim/syn038_pairwise/written_array_reduction_receivers.sv`; `tests/sim_syn038_written_array_reduction_receivers.rs::written_fixed_array_reductions_match_both_optimizer_modes` | `syn038_pairwise/written_array_reduction_receivers`; `--edition 2009`; stdout `written-reduction=0a,29,37,30,1e\n`; stderr `""` |
| W108 | O607–O609: reduction receivers after continuous-net, continuous-variable and NBA element writes. | `tests/fixtures/sim/syn038_pairwise/written_reduction_continuous_and_nba.sv`; `tests/sim_syn038_written_reduction_continuous_and_nba.rs::fixed_array_reductions_observe_continuous_and_nba_writes_in_both_cli_modes` | `syn038_pairwise/written_reduction_continuous_and_nba`; `--edition 2009`; stdout `net-reduction=6\nvar-reduction=6\nnba-reduction=5\n`; stderr `""` |
| W109 | O610–O615: scalar and selected event lvalues bind to their exact source declarations. | `tests/fixtures/sim/syn038_pairwise/event_lvalue_matrix.sv`; `tests/sim_syn038_event_lvalue_matrix.rs::event_expression_lvalues_run_in_both_cli_modes`; DB: `event_operands_and_lvalue_forms_share_each_owned_declaration_identity` | `syn038_pairwise/event_lvalue_matrix`; `--edition 2009`; stdout `events=111111 field=31,80 unpacked=35,80 element=70,42 row=51,52/73 concat=a6c2 pattern=5c\n`; stderr `""` |
| W110 | O616: event expression and NBA writer use the same focal slot; before/after checks distinguish issue from commit. | `tests/fixtures/sim/syn038_pairwise/event_expression_nba_same_slot.sv`; `tests/sim_syn038_event_expression_nba_same_slot.rs::event_expression_and_nba_write_observe_the_same_slot_in_both_cli_modes` | `syn038_pairwise/event_expression_nba_same_slot`; `--edition 2009`; stdout `before=11 old=00 event=00 seen=0\nafter=11 state=22 event=22 seen=1\n`; stderr `""` |
| W111 | O617–O628: continuous net/variable event sources and combinational/latch RHS readers across process sites. | `tests/fixtures/sim/syn038_pairwise/continuous_event_drivers.sv`; `tests/sim_syn038_continuous_event_drivers.rs::continuous_net_and_variable_events_match_exact_cli_oracle_in_both_modes`; DB: `owned_continuous_lhs_event_and_assignment_rhs_consumers_bind_to_outer_source` | `syn038_pairwise/continuous_event_drivers`; `--edition 2009`; stdout `wire=1,1,1,1,0,1 logic=1,1,1,1,0,1\n`; stderr `""` |
| W112 | O629–O631: NBA and continuous net/variable writes read as task input actuals. | `tests/fixtures/sim/syn038_pairwise/written_sources_call_arguments.sv`; `tests/sim_syn038_written_sources_call_arguments.rs::written_module_sources_reach_task_inputs_in_both_cli_modes` | `syn038_pairwise/written_sources_call_arguments`; `--edition 2009`; stdout `nba_call=5a/5a\ncontinuous_call=5a/5a\n`; stderr `""` |
| W113 | O632–O638: blocking-written source values observed through function returns and declaration initializers across storage phases. | `tests/fixtures/sim/syn038_pairwise/written_source_return_initializers.sv`; `tests/sim_syn038_written_source_return_initializers.rs::written_return_and_initializer_sources_match_in_both_cli_modes` | `syn038_pairwise/written_source_return_initializers`; `--edition 2009`; stdout `local_initializer=5a\ninitializer_source=5a\nstatic_initializer=5a\nlocal_return=5a\nmodule_initializer=5a\nconstant_initializer=5a\n`; stderr `""` |
| W114 | O639: continuous-net driver reaches a child input port. | `tests/fixtures/sim/syn038_pairwise/port_driven_wire_child_input.sv`; `tests/sim_syn038_written_child_inputs.rs::continuously_driven_wire_child_input_matches_both_cli_modes` | `syn038_pairwise/port_driven_wire_child_input`; `--edition 2009`; stdout `port_source=5a/5a\n`; stderr `""` |
| W115 | O640: continuous-variable driver reaches a child input port. | `tests/fixtures/sim/syn038_pairwise/port_driven_logic_child_input.sv`; `tests/sim_syn038_written_child_inputs.rs::continuously_driven_logic_child_input_matches_both_cli_modes` | `syn038_pairwise/port_driven_logic_child_input`; `--edition 2009`; stdout `port_logic=5a/5a\n`; stderr `""` |
| W116 | O641: NBA-written source reaches a child input port. | `tests/fixtures/sim/syn038_pairwise/nba_written_child_input.sv`; `tests/sim_syn038_written_consumer_inputs.rs::nba_written_child_input_matches_both_cli_modes` | `syn038_pairwise/nba_written_child_input`; `--edition 2009`; stdout `nba_port=5a/5a\n`; stderr `""` |
| W117 | O642: blocking-written source reaches a child input port. | `tests/fixtures/sim/syn038_pairwise/blocking_written_child_input.sv`; `tests/sim_syn038_written_consumer_inputs.rs::blocking_written_child_input_matches_both_cli_modes` | `syn038_pairwise/blocking_written_child_input`; `--edition 2009`; stdout `blocking_port=5a/5a\n`; stderr `""` |
| W118 | O643–O645: continuous-net, continuous-variable and NBA-written sources reach function input actuals. | `tests/fixtures/sim/syn038_pairwise/function_inputs_from_drivers.sv`; `tests/sim_syn038_written_consumer_inputs.rs::driven_sources_passed_to_function_inputs_match_both_cli_modes` | `syn038_pairwise/function_inputs_from_drivers`; `--edition 2009`; stdout `function_inputs=5a/5a/5a\n`; stderr `""` |
| W119 | O649–O653: five blocking-written selected sources reach task input actuals; +5 selected-input paths. | `tests/fixtures/sim/syn038_pairwise/written_selected_input_actuals.sv`; `tests/sim_syn038_written_selected_input_actuals.rs::written_selected_sources_reach_task_inputs_in_both_cli_modes` | `syn038_pairwise/written_selected_input_actuals`; `--edition 2009`; stdout `selected-inputs=31/80,42/17,51,52/73,a6/74,5c,7d/75\n`; stderr `""` |
| W120 | O668–O675: blocking, NBA and continuous-variable prior writes feed whole-variable const-ref actuals; +7 paths. | `tests/fixtures/sim/syn038_pairwise/constref_written_source_actuals.sv`; `tests/sim_syn038_constref_written_source_actuals.rs::prior_writes_reach_whole_const_ref_actuals_in_both_cli_modes` | `syn038_pairwise/constref_written_source_actuals`; `--edition 2009`; stdout `constref=31,42/81,53/16/75,51,62/76,a7,b8/77,8d,9e/78\nextra=86/97\n`; stderr `""` |
| W121 | O654–O665: blocking-written whole objects, fields, elements, slices, concatenations and patterns reach ref/inout tasks; +4 paths. | `tests/fixtures/sim/syn038_pairwise/written_ref_inout_actuals.sv`; `tests/sim_syn038_written_ref_inout_actuals.rs::blocking_written_whole_variables_reach_ref_and_inout_tasks_in_both_cli_modes` | `syn038_pairwise/written_ref_inout_actuals`; `--edition 2009`; stdout `prior-ref=10>ef,21>de,32>cd,54>ab,a6>59,5c>a3\nprior-inout=11>ee,22>dd,42>bd,64>9b,b6>49,6c>93\n`; stderr `""` |
| W122 | O666–O667: prior NBA values reach ref/inout task actuals; +2 paths. | `tests/fixtures/sim/syn038_pairwise/written_nba_ref_inout_actuals.sv`; `tests/sim_syn038_written_nba_ref_inout_actuals.rs::prior_nba_values_reach_whole_ref_and_inout_tasks_in_both_cli_modes` | `syn038_pairwise/written_nba_ref_inout_actuals`; `--edition 2009`; stdout `nba-ref-inout=5a>a5,5a>a5 final=a5/a5\n`; stderr `""` |
| W123 | O676: a `#1` settle precedes a read-only ref call to a continuously driven `logic` source; +1 pair. | `tests/fixtures/sim/syn038_pairwise/ref_read_continuous_variable.sv`; `tests/sim_syn038_ref_read_continuous_variable.rs::read_only_ref_of_continuously_driven_logic_runs_in_both_cli_modes`; DB: `owned_continuous_driver_and_ref_actual_share_source_identity` | `syn038_pairwise/ref_read_continuous_variable`; `--edition 2009`; stdout `ref-read=5a\n`; stderr `Warning: {absolute fixture path}:15:21 cannot mix continuous and procedural assignments to variable 'source'\n` |

W106 is present in the manifest as O597–O601 and has two selected-pair
closures: `CO-assignment_rhs__LV-field` and
`CO-assignment_rhs__LV-positional_pattern`. The five CLI cases check whole,
field, concatenation, positional-pattern and NBA same-root RHSs in both modes;
the DB owner binds each RHS reference to its own LHS root (fixture lines 22,
26, 30, 34 and 38). Exact stdout is `same_root=11,21,69,01,31\n`, with empty
stderr.
The W66 owner also checks duplicate-driver and mixed-driver rejections with
exact stderr oracles under `SYN038_DUPLICATE_DRIVER` and `SYN038_MIXED_DRIVER`.

The accepted 23-row legacy focal correction keeps `CP=function` on actual
function-result sources rather than on separate receiving assignment targets.
The corrected target rows are O03/O11/O12/O18/O29/O32/O38/O44/O45/O47/O48/
O50/O51/O52/O53/O65/O66/O103/O119/O424–O427; they use `CP=none` and
`OP=direct_projection` for the target-side focal value. Source-side replacements
are covered separately by W103/W104. The 18 earlier sparse `CO` rows are W79
O329–O342 and W103 O582–O585. The broader audit retains `CO=assignment_rhs`
only on 16 same-focal RHS rows; 176 target-adjacent rows omit `CO`, including
W99 O573–O575 and W75 O271–O275. The 18 W79/W103 address-only rows remain
sparse. O332/O336 also omit `TY` because their outer objects are unpacked arrays
of packed records with no matching outer type level. This same-focal source rule
is encoded in the checked-in manifest and checker.

```sh
python3 scripts/check_syn038_pairwise_manifest.py --gaps-by-block
cargo test --locked --test sim_syn038_ledger -- --test-threads=1
```

Run the dedicated pairwise CLI witnesses with:

```sh
cargo test --locked \
  --test sim_syn038_array_processes --test sim_syn038_enum_auto_function \
  --test sim_syn038_enum_contexts --test sim_syn038_formal_process_matrix \
  --test sim_syn038_child_param_decl_init \
  --test sim_syn038_generate_actuals \
  --test sim_syn038_generate_enum_predicate --test sim_syn038_generate_output_field \
  --test sim_syn038_generate_port --test sim_syn038_hierarchical_value_matrix \
  --test sim_syn038_call_storage_matrix \
  --test sim_syn038_interface_processes \
  --test sim_syn038_interface_lvalues --test sim_syn038_interface_record_constref \
  --test sim_syn038_interface_record_events \
  --test sim_syn038_interface_record_inout --test sim_syn038_latch_record \
  --test sim_syn038_localparam_member_reads --test sim_syn038_localparam_runtime_index \
  --test sim_syn038_op_consumer_matrix --test sim_syn038_op_lvalue_address_matrix \
  --test sim_syn038_operation_context_matrix \
  --test sim_syn038_lvalue_context_matrix \
  --test sim_syn038_process_value_matrix --test sim_syn038_initializer_source_matrix \
  --test sim_syn038_scope_storage_process_matrix \
  --test sim_syn038_process_write_remainders \
  --test sim_syn038_scope_hierarchy_matrix \
  --test sim_syn038_call_provenance_matrix \
  --test sim_syn038_interface_runtime_initializer \
  --test sim_syn038_storage_write_remainders \
  --test sim_syn038_static_return_ref_actual \
  --test sim_syn038_lvalue_storage_formal_matrix \
  --test sim_syn038_continuous_pattern_lhs \
  --test sim_syn038_packed_pattern_return_comb --test sim_syn038_packed_return_ff \
  --test sim_syn038_record_auto_ref --test sim_syn038_record_reduction_init \
  --test sim_syn038_ref_port --test sim_syn038_return_slot_formals \
  --test sim_syn038_runtime_source_local_initializers \
  --test sim_syn038_selected_activation_lvalues \
  --test sim_syn038_static_local_continuous --test sim_syn038_static_return_continuous \
  --test sim_syn038_typed_formal_matrix --test sim_syn038_typed_initializer_matrix \
  --test sim_syn038_typed_continuous_matrix --test sim_syn038_typed_lvalue_matrix \
  --test sim_syn038_union_constant --test sim_syn038_union_function_port \
  --test sim_syn038_union_hier_ref --test sim_syn038_union_interface \
  --test sim_syn038_typed_context_remainders \
  --test sim_syn038_typed_aggregate_lvalue_matrix \
  --test sim_syn038_lvalue_generate_scope_remainders \
  --test sim_syn038_scope_initializer_remainders \
  --test sim_syn038_hierarchy_process_call_remainders \
  --test sim_syn038_typed_scope_remainders \
  --test sim_syn038_typed_constant_event_matrix \
  --test sim_syn038_storage_hier_route_remainders \
  --test sim_syn038_static_hierarchical_block_port_actual \
  --test sim_syn038_interface_member_child_port \
  --test sim_syn038_interface_function_source \
  --test sim_syn038_legacy_op_replacements \
  --test sim_syn038_legacy_cp_replacements \
  --test sim_syn038_subroutine_event_signal_matrix \
  --test sim_syn038_co_same_root_assignment_rhs \
  --test sim_syn038_written_array_reduction_receivers \
  --test sim_syn038_written_reduction_continuous_and_nba \
  --test sim_syn038_event_lvalue_matrix \
  --test sim_syn038_event_expression_nba_same_slot \
  --test sim_syn038_continuous_event_drivers \
  --test sim_syn038_written_sources_call_arguments \
  --test sim_syn038_written_source_return_initializers \
  --test sim_syn038_written_child_inputs \
  --test sim_syn038_written_consumer_inputs \
  --test sim_syn038_written_selected_input_actuals \
  --test sim_syn038_constref_written_source_actuals \
  --test sim_syn038_written_ref_inout_actuals \
  --test sim_syn038_written_nba_ref_inout_actuals \
  --test sim_syn038_ref_read_continuous_variable \
  -- --test-threads=1
```

### SYN-039 selected-profile acceptance — Linux WSL2 host gate passed

The SYN-038 value-pair denominator is frozen at **2,247 raw / 1,849 selected /
1,849 covered / 0 legal gaps / 296 impossible / 102 outside-profile**.
`required_zero_legal_gaps=true` is enforced by the checker and ledger; the
checker rejects a selected legal gap, and a witness-removal mutation was
rejected. Checker SHA-256:
`b1ccaad76c1488195b55923ce5391c748cfe94cb6879b61215064ace4d43ea6e`.
Manifest SHA-256:
`941e2f1ab12064cfa820686e36ba2e29bbc8a418940c26e265f3d1edfe460757`.
The checker and ledger enforce the zero-gap requirement. The W123-specific
ledger and witness checks now pass after the invocation-contract correction.
This closes the audited denominator for the committed source snapshot below.
The composed serialized all-features run passed for the bounded selected profile
on the stated Linux WSL2 host.

The current committed source identity is HEAD
`179bd76fdd72683cd3b1b9c2056444848c867127`, tree
`d3195386ac77e61950bd2cb8d60890f0407063fc`. The scoped commit sequence is
`554a343` (frontend), `ec7032b` (simulator repairs and held-out fixtures),
`97ba0df` (SYN-038 witnesses), and `179bd76` (checker, manifest, and ledger).
The earlier working-tree digest
`c2824c38babd55beb5632af25147f03f85ff55fb1786dea2a2907080a61f7d4f` on base
`b4874776ede211437123f6c1d0505e67c3cb92cf` is historical pre-commit provenance,
not the current source identity. The serialized
`cargo test --locked --all-features --no-fail-fast -- --test-threads=1` gate
passed: **2,756 passed, zero failed, one ignored across 242 targets**. Its log
is `persistence/synth-review-full-suite-20260924T1515.log`. Validation ran on
`Linux 6.6.87.2-microsoft-standard-WSL2 x86_64` with rustc 1.98.0
(2026-08-18), Cargo 1.98.0 (2026-08-05), Debian cc/GCC 14.2.0 and CMake 3.31.6.
The earlier all-features attempt, before the W123 ledger-contract and
interface-storage fixes, reported 2,754 passed, 2 failed and 1 ignored across
242 targets. Both failures passed in the completed run on the committed source
identity above.

The following static checks passed on that host:

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check --locked --all-targets --all-features`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo build --locked --bin llg --bin llg_ls --bin elab_check`
- `cargo check --locked --lib --no-default-features`
- `python3 scripts/check_sim_fixture_integrity.py --tracked` — 902 references,
  851 paths, zero errors

At 15:30 UTC, the settled-source `llg` replay ran R01–R09 in
SystemVerilog-2009 in both optimizer modes; the R10 mux-policy case additionally
ran in Verilog-2001 in both modes. All 36 return-code/stdout/stderr outcomes
matched the earlier reviewed run after normalizing only temporary-path prefixes:
22 matched positive oracles, 14 retained their documented manual-review
policy/diagnostic statuses, and there were zero timeouts. The separate R11
generation-only 65,537-cell cap replay matched 2/2 optimizer-mode outcomes.

R11's selected fixed-array limit is 65,536 cells, while the supplied IEEE
editions require a minimum capacity of 16,777,216 elements. Its 65,537-cell
case is a generation-only rejection control and does not test a 16M-element
simulation. The documented implementation therefore does not claim the
standard minimum-capacity requirement above the project cap.

R13's `sim_review_bundle_composition` passed 2/2 tests. Each SystemVerilog-2009
fixture runs in both optimizer modes through the public CLI with exact oracles:

- `r13_recursive_pattern_function_port.sv` composes recursive type keys,
  function return, aggregate input port and `always_comb`; stdout is
  `recursive pattern function/port passed: 34\n`, stderr is empty.
- `r13_record_pattern_selected_nba.sv` composes record-pattern deconstruction,
  selected NBA scheduling and pre-commit selector/source changes; stdout is
  `selected record NBA scatter passed: 12/34\n`, stderr is empty.

Focused generated-model GCC ASan/UBSan evidence spans seven time-stamped
snapshots: 12:36 (18 tests/18 logs), 13:04 (4/4), 13:11 (2/2), 13:13 (2/2),
14:02 (10/10), 14:15 (10/10, including the post-fix W123 run), and 15:26 (2/2
`sim_syn038_union_interface` modes after the interface-storage fix). These
snapshots total 48 test executions and 48 inspected generated-model logs, with
suites repeated across snapshots; this is neither 48 distinct tests nor a full
sanitizer lane. The logs contain only
the known libaco coroutine stack-switch warning and no ASan/UBSan/LSan error or
summary. This lane instruments generated C models, not vendored Slang. Handwritten
native C probes are separate and are not counted here.

The tested editions and host limits are explicit: R01–R09 replay uses SV2009;
R10 additionally uses V2001; R11 is an SV2001 generation-only capacity
boundary; R13 is SV2009.
The evidence qualifies Linux x86_64 under this WSL2 host only. It does not
qualify native Windows/macOS, other Linux distributions/toolchains, or synthesis
tool output. The selected-profile acceptance claim is limited to this Linux
WSL2 host and the checked-in bounded scope.

The composed acceptance gate includes `sim_syn039_acceptance`,
`sim_rtl_composition`, `sim_rtl_completion`, `sim_sequential_predicates`,
`sim_udp`, `sim_syn032_library_configs`, `sim_net_resolution`,
`sim_memory_views`, `sim_syn036_capacity`, the SYN-038 ledger and pairwise CLI
owners, and the review-bundle suites. Its serialized all-features run passed:
2,756 passed, zero failed and one ignored across 242 targets.

### Synthesizable review bundle regressions

The [review bundle fixtures](fixtures/sim/review_bundle/readme.md) preserve the
held-out R01–R11 HDL cases and their file inputs. Additional checked-in cases
exercise the repaired behavior and compose features across lowering, generated
C, and the runtime. The public-CLI suites compare exact output in both optimizer
modes; runtime-error cases require the expected diagnostic after successful
frontend and lowering stages.

```sh
cargo test --locked --test sim_tagged_union_access --test sim_review_bundle \
  --test sim_review_bundle_patterns --test sim_fixed_ordering_review \
  --test sim_memory_editions --test sim_memory_views \
  --test sim_conditional_policy --test sim_review_bundle_composition \
  --test sim_syn036_capacity --test sim_syn038_ledger -- --test-threads=1
```

`sim_tagged_union_access` covers inactive tagged-member reads and writes,
receiver evaluation, and captured selected NBA addresses (R01–R02).
Its `n05_tagged_guard_stress` fixture repeats nested selected reads/writes and
reference forwarding with a receiver-call counter. Run this suite under the
existing generated-runtime sanitizer configuration to check N05: correct stdout
and handwritten value-helper probes alone cannot establish guard leak freedom.
The `owned::tests::tagged_guards` unit tests separately track both emitted guard
paths. Existing inactive read/write/ref/NBA cases remain diagnostic controls.
`sim_casez::case_matching_tables_and_selector_z_work_in_both_editions` runs
`review_bundle/n11_casez_selector_z.sv` under 2001 and 2009, optimized and
unoptimized. Runtime task formals cover all sixteen state pairs for ordinary
case/casez/casex; separate constant-function, generate-if, constant procedural
and 129-bit cases check the remaining routes. Literal tables in the core,
optimizer and standalone four-state tests supply independent oracles. The
property-test reference and original C vector expectations use the corrected
symmetric casez rule; wildcard equality and inside remain separate contracts.
`sim_review_bundle` covers static-return callback classification and
read-modify-write return access (R03). `sim_review_bundle_patterns` covers
recursive and duplicate type keys, constant index expressions, and typed
pattern-lvalue deconstruction (R04–R06). `sim_fixed_ordering_review` checks
whole-element reverse and mapped ordering of fixed record arrays (R07).
`sim_memory_editions` checks enum file words before narrowing (R08), while
`sim_memory_views` checks selected rows, slices, addresses, and selector capture
(R09). `sim_conditional_policy` pins the published ambiguous-selector Z/Z cell
through localparam, generate-case, and runtime evaluation (R10).
`sim_review_bundle_composition` carries repaired pattern keys through ports and
functions, and pattern deconstruction through selected NBA scheduling (R13).
`sim_syn036_capacity` runs the held-out 65,537-cell rejection control for the
disclosed R11 resource ceiling.
`sim_syn038_ledger` owns the structural and traceability checks plus focused
CLI cases for selected grammar rows, fixture ownership, edition gates, and
context combinations (R12). The dedicated pairwise CLI targets are listed in
the SYN-038 suite map above. The zero-gap denominator is frozen; the full
composed all-features gate passed on the stated Linux WSL2 host.

Focused generated-model sanitizer evidence and host scope are summarized in the
SYN-039 section above. The latest tracked fixture-integrity scan found 902
references across 851 paths with zero errors; this checks static path integrity,
not behavioral coverage.
### Coverage

- `sim_port_net_types`: R05 directional dissimilar inout collapse in both public
  optimizer modes. Fixtures cover both port orientations, hierarchical chains,
  warning-only external choices, implicit biases and drive strengths, selected
  and concatenated ports, true aliases, ascending ranges, fixed net-array cells,
  force/release and winning/absent propagation delays. Separate negatives retain
  mixed-alias, trireg and frontend uwire-inout rejection. Pure unit tests encode
  all 81 table cells; frontend/database tests cover the 49 executable resolved
  class pairs and composition after snapshot destruction. New Rust/HDL tests
  are unexecuted until run in the pinned environment. The native
  `port_net_collapse_probe.c` configures the existing runtime by hand; its two
  modes do not establish frontend/lowering correctness or generated-model
  coroutine/sanitizer acceptance.

  ```sh
  cargo test --locked --lib port_net_type
  cargo test --locked --test slang_semantics port_net_type -- --test-threads=1
  cargo test --locked --test sim_port_net_types -- --test-threads=1
  cargo test --locked --test sim_net_resolution --test sim_net_decl --test sim_inout -- --test-threads=1
  ```

- `sim_loops`: mixed packed/unpacked `foreach` tests cover exact coordinate
  order, bit reads/writes, skipped slots, lexical scope, source-loop jumps,
  signed endpoint boundaries, subprogram values, and formal-port bounds with
  combinational sensitivity. Fixed scalar, excess-iterator, and read-only
  negative cases remain separate. See [loop fixtures](fixtures/sim/loops/readme.md).
  `slang_semantics` tests owned slot import and missing/malformed metadata;
  projector and database unit tests cover dimension preservation and validation.

- `sim_group1_formal_repairs`: R09/R14 packed activation isolation, recursion,
  callbacks, member state conversion, immediate references, captured copy-out
  addresses and preserved const/NBA negatives. `sim_edition` exercises the shared
  execution/navigation edition policy, macro/directive context, standard timing
  checks and explicit registered extensions. The new tests are unexecuted until
  run in the pinned native/Rust environment.

- `sim_group1_repairs`: file-backed callback, instance-index, selected aggregate
  and fixed-streaming regressions for the first Group 1 review repair batch.
  Runs both optimizer modes and uses its own `group1_repairs` fixture directory. Added tests are not acceptance evidence
  until executed on the target toolchain.

- [Datatype/net matrices](fixtures/sim/type_conformance/readme.md): mixed operators, resolution truth tables, casts, two/four-state storage and X/Z-to-zero conversion.
- [Feature regressions](fixtures/sim/partial_features/readme.md): ports, events, timing, packed selections, real sensitivity/math, time formatting, immediate and deferred four-state assertions, gated host commands, and resumable `$stop` control.
- [Concurrent assertion regressions](fixtures/sim/concurrent_assertions/): Preponed sampling, asynchronous `disable iff`, overlapping attempts, Reactive actions, vacuity accounting, and end-of-simulation pending-attempt handling in both optimizer modes.
- [Process semantic regressions](fixtures/sim/process_semantics/readme.md):
  always-family sensitivity, time-zero execution, writer/timing contracts and
  legal latch/flip-flop controls.
- `sim_syn014_process_contexts.rs` runs the SYN-014 aggregate/process witness
  in both optimizer modes: fixed record and nested-array sensitivity through
  helper calls and input links, ordinary `@*` call-site behavior, written
  member exclusion, disjoint packed writers, legal latch/flip-flop controls,
  and single-fault writer/event rejection controls. See
  [SYN-014 fixtures](fixtures/sim/syn014_process_contexts/readme.md).
- `sim_process_control`: process-class identity/status observations,
  suspended waits, terminal awaits, recursive kill cleanup and independent
  delayed NBA ownership in both optimizer modes.
- `sim_semaphore`: zero-key construction and exact `try_get` results,
  differing-count FIFO contention, suspended wake deferral, task-handle
  arguments, and cancellation-safe blocked waiters in both optimizer modes.
- `sim_mailboxes`: typed/untyped bounded and unbounded mailbox FIFO order,
  peek/try APIs, packed/real/string/handle copy semantics, waiter handoff and
  process-kill cleanup in both optimizer modes.
- [Physical-time regressions](fixtures/sim/physical_time/readme.md): 1fs–100s
  scheduling, checked overflow, and femtosecond waveform timestamps.
- [Waveform regressions](fixtures/sim/waveform/readme.md): file-backed VCD/FST
  catalogs, `$dumpvars` depth/name filtering, aliases, declared array indices,
  value types and dump lifecycle controls.
- [File-I/O regressions](fixtures/sim/file_io/readme.md): owned descriptor
  masks, multichannel output, deferred file formatting, portable seek/rewind/
  flush status, descriptor-table boundaries, HDL-aware formatted/character/
  line input, and packed/ascending/descending binary reads.
- `sim_memory`: fixed packed-memory `$readmemh/$readmemb` parsing,
  `$writememh/$writememb` roundtrips, range/order/address handling, four-state
  conversion and file-size diagnostics in optimized and unoptimized models.
- `sim_memory_views`: SystemVerilog-2009 multidimensional and constant selected
  memory views, row-major file ordering, declaration-direction storage,
  address jumps, incomplete rows, packed-struct elements and unsupported-view
  diagnostics in optimized and unoptimized models.
- [Procedural assignment regressions](fixtures/sim/procedural_assign/): PCA priority, replacement, dependencies and force layering.
- `sim_reference_args`: typed `ref`/`const ref` aliasing, selected actuals, nested calls, recursion and suspension observation.
- [Datatype basics](fixtures/sim/data_types/), [wide values](fixtures/sim/data_types_extended/) and [edge cases](fixtures/sim/data_type_edges/): operator/state combinations, limb boundaries and capacity rejection.
- [Aggregates and containers](fixtures/sim/data_types_next/) and [completion cases](fixtures/sim/data_types_completion/): storage, methods and conversion boundaries.
- [Logical expression regressions](fixtures/sim/logical_ops/): ordinary `->`/`<->` four-state truth tables, precedence, side-effect evaluation, real operands and optimizer parity.
- `sim_stochastic`, `runtime_stochastic`: IEEE stochastic-analysis queue order, status codes, simulation-time statistics and scheduler-independent runtime boundaries.
- [Executable-node coverage](fixtures/sim/u01_coverage/): source-located fail-closed unsupported nodes, compile-time declarations and elaborated-away branches.
- `sim_edition`: checked-in 2009 time-literal rounding and 2001 edition/keyword CLI probes.
- `sim_syn018_module_declarations`: parameterized extern declarations and bodies,
  independently scoped nested modules, owned hierarchy identities, and single-fault
  frontend diagnostics in both compilation-unit policies and optimizer modes.
- `sim_syn033_structural_bind`: parameterized module-type and selected-instance binds,
  interface-to-interface binding, owned bound-instance paths after frontend drop, and
  single-fault unknown/primitive-target diagnostics in both optimizer modes.
- `sim_syn036_capacity`: the selected 65,536-cell fixed-array storage boundary,
  direct cell-wise reduction above packed payload capacity, and single-fault
  flattened-value diagnostics in both optimizer modes.
- `sim_syn039_acceptance`: final composed Core/Extended public-CLI witnesses
  and the neighboring sequential-UDP profile rejection in both optimizer modes.
- `sim_physical_time`: file-backed 1fs/10fs/100fs/1ps/1ns mixed scopes,
  10s/100s units, checked overflow rejection, and VCD femtosecond
  headers/timestamps.
- `runtime_values`, `runtime_boundaries`, `region_conformance`: standalone C value checks, resource bounds and scheduling order.
- [Dynamic packed storage](runtime_value_storage/readme.md): exact allocation
  sizes, deep copies, moves, replacement, width boundaries, injected allocation
  failure, and waveform snapshot transfer/cleanup. Runs directly through CMake
  without Cargo/Slang, or through `runtime_value_storage.rs`.
- `sim_random`, `runtime_random`: legacy `$random`/`$dist_*` Annex N vectors
  through generated models and standalone C runtime boundary checks, each at
  optimized and unoptimized levels.
- `sim_opt_differential`: optimized/unoptimized equivalence against regression traces.

### Limits of the tests

- Passing fixtures establish exercised behavior, not complete IEEE conformance.
- Wide probes cover representative operations at 65,536 and 1,048,575 bits; they do not exhaust every value or context.
- Capacity tests check rejection at 1,048,576 bits; fixed-size atoms retain their specified widths.
- SYN-036 also checks the selected 65,536-cell fixed-array storage boundary;
  this resource ceiling is separate from the packed value limit.
- Driver-boundary tests exercise registry growth past the retired per-net ceilings.
- Two-state net declarations are language errors (§1800-2009 6.7); two-state conversion tests apply to variables and expressions.
- Platform build configuration alone is not evidence of successful native execution.
- Feature restrictions and implementation limits are maintained in [sim_features.md](../docs/sim_features.md).
- Zero-time execution uses `LLG_ZERO_LOOP_LIMIT` for scheduler passes and
  `LLG_PROCESS_STEP_LIMIT` for generated process back-edges (with
  `LLG_NONCONVERGENCE_LIMIT` as an alias). Both default to 10,000,000, require
  a positive decimal `uint64_t`, and report invalid or overflowing values
  before execution; setting only the scheduler variable also applies that
  value to the process budget.

## Running tests

### Prerequisites

- Rust 1.98.0 toolchain and initialized vendored dependencies.
- `cargo-nextest` (`cargo install cargo-nextest --locked`).
- CMake and a C compiler; the file-based conformance suites require both.
- Run commands from the repository root.
- Generated-runtime archives are shared under `target/llg-runtime-cache` by
  default; set `LLG_RUNTIME_CACHE_DIR` to override the location. Relative
  override paths are resolved from the repository root.
- Nextest runs 8 tests concurrently by default; use
  `--profile max-threads` to opt in to 32 on a sufficiently large host.

### Focused simulator suites

```sh
cargo nextest run --locked --test sim_type_conformance --test sim_partial_features
cargo nextest run --locked --test sim_data_types --test sim_data_types_extended --test sim_data_type_edges
cargo nextest run --locked --test sim_data_types_next --test sim_data_types_completion --test sim_net_resolution --test sim_net_defaults --test runtime_values --test runtime_random
```

```sh
cargo nextest run --locked --test sim_physical_time
cargo nextest run --locked --test sim_mailboxes
```

### One readable fixture

```sh
cargo run --locked --bin llg -- --top tb tests/fixtures/sim/type_conformance/uwire.sv
cargo run --locked --bin llg -- --no-opt --top tb tests/fixtures/sim/type_conformance/uwire.sv
```

### Generated-runtime sanitizers (GCC)

```sh
LLG_CC=gcc \
LLG_CFLAGS='-DACO_USE_ASAN -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all' \
ASAN_OPTIONS='detect_leaks=1:strict_string_checks=1:log_path=/tmp/llg-asan-model' \
UBSAN_OPTIONS='print_stacktrace=1:halt_on_error=1' \
cargo nextest run --locked --test sim_partial_features --test sim_type_conformance --test sim_procedural_assign --test runtime_values --test runtime_random
```

Inspect every `/tmp/llg-asan-model.*` file after the run for sanitizer error
reports. Generated coroutine models can emit ASan's
`__asan_handle_no_return` stack-switch warning; routing sanitizer diagnostics
to files preserves the suites' exact program-stderr assertions.

### Repository gate

```sh
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo check --locked --lib --no-default-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo nextest run --locked --all-features
cargo test --locked --doc --all-features
```

- [CI workflow](../.github/workflows/ci.yml): full suite selection, sanitizer settings and release checks.

### Reproducible baseline and per-patch regression

The exact Rust toolchain is pinned in [`rust-toolchain.toml`](../rust-toolchain.toml),
and Cargo dependencies are resolved by [`Cargo.lock`](../Cargo.lock). The
vendored Slang and libaco gitlinks must be checked out at the upstream base
revisions recorded by the root commit; `build.rs` applies the reviewable patches
under `patches/` before native sources are consumed. The patch directories also
carry complete clean/applied file digests and a retired-file digest for the
portable source-archive path. No project-specific vendor commits are allowed.
When Git metadata is present, the preparer verifies the pinned `HEAD` and
rejects tracked or source-like untracked inputs outside the active patch set;
Git metadata and `safe.directory` configuration remain optional.
`scripts/run-regression.sh` verifies the gitlinks before
running the serialized gate and records provenance, command lines, phase status,
timings, and complete logs in ignored `persistence/` output.

Run the gate in separate directories before and after a feature patch, then
compare the stable phase summary:

```sh
scripts/run-regression.sh --label before --output-dir persistence/u05/before
scripts/run-regression.sh --label after --output-dir persistence/u05/after
diff -u persistence/u05/before/summary.tsv persistence/u05/after/summary.tsv
diff -u persistence/u05/before/test-inventory.log persistence/u05/after/test-inventory.log
```

The runner refuses tracked root or submodule modifications by default. A local
run may pass `--allow-dirty-root` or `--allow-dirty-submodules`, but its
`metadata.tsv` is marked `reproducible=no` and must not be used as clean
baseline evidence. Expected stdout and diagnostic assertions remain in the
Rust tests; this workflow never blesses changed expected output. The
`runtime_values` phase is a standalone C value-runtime check, while the full
suite and generated-runtime matrix are the evidence for simulator execution.
The inventory diff makes added, removed, or renamed tests visible alongside
the phase-status comparison; any expected-output change still requires a
feature/clause justification in the patch.

## Priority-review regression sources

- `sim_process_control::a_child_killing_its_ancestor_never_returns_to_released_locals`
  covers ancestor kill with live automatic locals and nested descendants.
- The `semaphore/cancel_{head,tree,named,fork}.sv` fixtures distinguish live
  FIFO reservice from mid-cancellation grants and cover the public cancellation
  entrypoints. No extra `put` is used to make the waiting request runnable.
- `sim_vpi` adds vector-ownership and callback-borrow-lifetime plugins. They
  exercise zeroed, format-only, poisoned and caller-buffer vector requests,
  multiword X/Z results, and stale call/argument/iterator handles across all
  three system-task callback kinds. Permanent model handles remain valid.
- `sim_dpi::dpi_string_results_are_snapshotted_before_aliased_copyout` covers
  an inout echo, swapped buffers, a void return, and shared output/return
  pointers. The C emitter has a separate ordering regression for string,
  integral and void returns.

These regression sources were added by static review; their addition is not
recorded test-execution evidence.


## Follow-up review regression sources

The supplied `test-to-be-added` sources now live under
`fixtures/sim/imported_probes/`. `sim_imported_probes.rs` actively checks five
additional acceptance witnesses in both optimizer modes. Eight other supplied
acceptance inputs already have active feature-suite equivalents; net alias
connectivity is retained as an ignored test. The review counterexamples are
preserved with a suite mapping in their README, including the C integration
fragments that cannot run as standalone designs.

`sim_review_batch2.rs` contains origin-specific program exit/completion cases,
postponed alias reads, delayed-alias event/level waiters, scalar mailbox mismatch
and FIFO cases, and independent assertion-failure/severity accounting. The
program fixtures use multiple instances of the same definition and multiple
initials, with a module-defined task invoked from both program and module
origins. Mailbox mismatch tests preserve the queued message and the target;
nominal enum/handle type equivalence is exercised separately by the batch-04 source regressions.

`runtime_rng.rs` checks exact parent seed consumption, state replay and the
independence of already-created children. Prior draws in a parent are allowed
(and required) to affect a subsequently created child's seed. Existing program
and mailbox expectations are updated to those specified contracts rather than
weakening stdout/stderr checks. These additions and changes were inspected
statically only; no build or test execution is claimed.


## Ten-finding review batch 03

`sim_review_batch3.rs` supplies focused cases for Observed clocking-block
publication, inheritance-layer construction, factory receiver binding,
non-packed property initializers, assertion off/kill and property truth,
numeric-prefix scanning, and the FD/MCD bit contract. The dedicated VPI suite
adds requested time-format/scaling coverage; `runtime_review_batch3.rs` uses
two different sized-function call descriptors so frontend argument coercions
cannot conceal shared return-width state. Existing I/O oracles now distinguish
FDs from MCDs instead of asserting implementation-assigned slot numbers.

These test sources were added by static inspection only. No new passing-test
result is recorded, and shared stdout/stderr comparisons are unchanged.


## Review batch 04

- `sim_review_batch4.rs` supplies unexecuted regressions for retained outdated
  queue refs (including aliases, self-assignment, suspension and cancellation),
  nominal and structurally equivalent mailbox types, ref mailbox destinations,
  empty sequence boundaries, guarded repetition, nested/tied `first_match`,
  coincident/noncoincident multiclock boundaries and inherited clock flow,
  implication-local snapshots, and disjoint/overlapping packed-prefix analysis.
- `runtime_containers.rs` adds direct retained-cell lifetime assertions,
  including sharing, queue destruction, reorder identity, and final release.
- The H25 multiclock fixture now distinguishes a same-time destination from a
  strictly later edge; its expected output follows that distinction. The batch-03
  CLI harness imports the shared simulation helper required by `sim_cli`.
- No compiler, simulator, runtime probe, sanitizer, or Cargo command was run for
  these changes. Shared stdout/stderr comparisons remain unchanged.

## Source organization

`lsp_stdio.rs` retains the framed JSON-RPC client and shared process helpers;
`lsp_stdio/` contains responsibility-named suites. The integration-test crate
uses explicit `lsp_stdio/...` module paths so these suites are not discovered
as independent Cargo test targets. Existing checked-in HDL fixtures and
independent expected results remain with their original suite owners. The stdio
and feature-test cases now have domain-qualified names; update exact-name
filters to include that domain. Integration-test binary names are unchanged.

Runtime source-assembly unit tests in `src/sim/rt/tests.rs` compare the C facade
include order with the embedded flat implementation; they do not compile C or
replace runtime execution tests. See [the source map](../docs/source_layout.md)
for other unit-test and implementation domains.


## Dynamic ownership validation

- Run `python3 tests/runtime_value_storage/validate.py --compiler gcc --sanitizers`
  for native Debug/Release components, strict flat C/ABI checks, independent
  oracles and exact live-allocation plateau measurements.
  - Add `--full` to require Rust checks, both active emitted-model tests,
    public HDL cases in both optimizer modes, and the repository suite.
  - New or empty output directories retain `report.json` and actual command logs;
    missing prerequisites and excluded platform components never count as passes.
- [The component guide](runtime_value_storage/readme.md) describes portable and
  native-fiber coverage, environment requirements, exit codes and memory metrics.
- [HDL fixtures](fixtures/sim/dynamic_ownership/readme.md) are positive acceptance
  tests, not claims that their migration-dependent generated paths already pass.
- `sim_syn038_nested_task_fork_events.rs` owns three SV2009 nested-task fork
  regressions in both optimizer modes. `nested_task_fork_static_event.sv` must
  print `static-event=1\n` and has the exact upward-hierarchy warning at
  `{absolute_fixture_path}:14:19`; `nested_task_fork_formal_event.sv` prints
  `formal-hier-event=1\n` and has warnings at `:11:17` and `:14:19`;
  `nested_task_fork_real_join.sv` prints `real=2.5\n` with empty stderr. The
  suite checks wakeup on persistent static task-local/formal storage and joined
  automatic real output propagation. Synchronous `fork ... join` borrows
  automatic numeric cells from the suspended parent frame; `join_any` and
  `join_none` keep snapshot behavior. These tests do not claim detached
  automatic alias propagation or new `ref`-formal capture behavior. The focused
  gate also reran `sim_syn038_operation_context_matrix`, `sim_dynamic_ownership`,
  `sim_process_semantics`, and `sim_fork`:

  ```sh
  cargo test --locked --test sim_syn038_nested_task_fork_events \
    --test sim_syn038_operation_context_matrix --test sim_dynamic_ownership \
    --test sim_process_semantics --test sim_fork -- --test-threads=1
  ```
- [Dynamic-owner CI](../.github/workflows/dynamic-owners.yml) configures scoped
  Linux, macOS and Windows component checks plus a manual Linux full-host gate.
  Reports and command logs are printed to workflow logs; it uploads no artifacts.
  CI configuration is not native-platform execution evidence.

The dynamic component inventory also builds the original runtime and waveform
self-tests with allocation accounting. Numeric vectors and waveform cleanup run
in the sanitizer-safe lane; complete scheduler/region/stop-resume/budget probes
use native coroutines. Callback-finish probes cover packed snapshot/result cleanup
and all-context adoption, including shared eval/condition frames. Exact-address
scope-index and event-array probes cover retained targets, index churn and invalid
handle waits without weakening existing assertions. The checked inventory must
include these tests when their scheduler/waveform prerequisites are enabled.

The positive HDL ownership fixtures include numeric inputs/defaults/inout copy-in,
owned captured forks, evaluated/filtered waits and indexed events. They run through
`llg` with both optimizer settings; source addition or a migration diagnostic is
not a passing result. Whole-model Rust emitter tests also verify shared-context
reference counts and ordered index destruction, rather than accepting detached
expression fragments as owners.

The standalone value, container and file-I/O Cargo probes share their C sources
with the component suite (`*_isolation_probe.c`), so component validation includes
the original assertions and per-vector ownership cleanup. Run these individually:

```sh
cargo test --locked --test runtime_values --test runtime_containers --test runtime_file_io
cargo test --locked --lib --no-default-features sim::emit_c::owned::tests
cargo test --locked --no-default-features --test sim_dynamic_ownership -- --test-threads=1
```

The active `owned/tests/nextest_regressions.rs` models exercise the real emitter's
cancellation-before-copyout, lexical activation exit, inertial, strobe and force
paths. `nextest_control_probe.c` checks similar runtime patterns without Rust;
its native pass does not substitute for those generated-model tests. The full
runner and main CI select the active tests without `--ignored`.

### Native ownership source-repair regressions

The native-value follow-up uses the existing public container/string/file/process
HDL suites without weakening their expected results. Nine structural tests in
`src/sim/emit_c/owned/tests/native_values.rs` check emitter ownership contracts.
The component guide documents `native_value_scopes` and `native_input_callbacks`;
these are runtime probes and must not be reported as Rust/HDL passes. Failure-target
manifests are delivery inventories, not the maintained language feature checklist.

## Fixed-array reduction regressions (R03)

`sim_fixed_array_reductions.rs` registers checked-in HDL sources for all five
fixed-array methods, narrow and widened maps, signed/enum/record elements, X/Z,
wide owners, nested maps, automatic captures, function receivers, slices, declared
indices and port sensitivity. The shared CLI harness runs each case with
optimization enabled and disabled and checks exact output. Negative cases reject
unmapped rows, nonintegral maps and iterator names without a `with` expression.
A resizable-container control preserves the separate existing callback path.

```sh
cargo test --locked --lib fixed_array_reduction
cargo test --locked --test slang_semantics fixed_array_reduction -- --test-threads=1
cargo test --locked --test sim_fixed_array_reductions -- --test-threads=1
```

The R03 implementation environment did not have Rust/Cargo or a built `llg`;
these commands and their new tests have not been executed there. The two native
`fixed_array_reduction_*` CTest probes are handwritten runtime counterparts,
not freshly generated HDL models. Feature acceptance belongs in
[`docs/sim_features.md`](../docs/sim_features.md), not in fixture counts.


## Fixed-array conditional assignment regressions (R04)

- `sim_array_conditional_assignments.rs` runs checked-in SystemVerilog-2009
  fixtures through the public CLI, with and without optimization. Exact output
  and self-checks cover direct `always_comb` assignments, selector/arm effects,
  overlapping slices/patterns, NBA source/address capture and ordering,
  `always_ff`, declared bounds, selected rows, two-state and nested defaults,
  129-bit elements, and neighboring cast/concatenation conversions.
- Rejection fixtures retain array rank and element-type compatibility; equal
  flattened bit counts do not make different array shapes assignment compatible.
- `slang_semantics::array_conditional_assignments` checks that the original
  module RHS is a direct typed conditional, not a function/cast workaround,
  and that the positive contexts generate C from an owned DB after snapshot
  destruction. Generation checks do not execute the emitted model.

```sh
cargo test --locked --test slang_semantics array_conditional_assignment -- --test-threads=1
cargo test --locked --test sim_array_conditional_assignments -- --test-threads=1
cargo test --locked --test sim_p30_fixed_arrays -- --test-threads=1
cargo test --locked --test sim_rtl_completion array_conditional -- --test-threads=1
```

These new Rust/HDL tests were not executed in the R04 implementation environment,
which lacked Rust/Cargo and a built `llg`. Reused runtime component tests and
fixture-integrity checks are separate evidence, not language acceptance.

## Sequential predicate regressions (R06)

`sim_sequential_predicates.rs` runs the checked-in SystemVerilog-2009 sources in
`fixtures/sim/sequential_predicates/` through independent optimized and unoptimized
CLI runs. Ten positive cases cover the exhaustive three-clause four-state truth
matrix, branch roles, short-circuit effects, constant prefixes, 65/129-bit and real
truth, real-result ambiguity, R01/R04 array results, clock/combinational sensitivity,
qualifiers and R03 lexical reduction contexts. Two negative cases require explicit
pattern rejection, even with a preceding false clause. `slang_semantics` tests keep
the owned DB after snapshot destruction, inspect clause/branch roles and exercise
both generation modes. Unit tests cover import/validation, lints, folding, all-
clause traversal, capacity, checked FFI tags and structured emission.

```sh
cargo test --locked --lib sequential_predicate
cargo test --locked --test slang_semantics sequential_predicate -- --test-threads=1
cargo test --locked --test sim_sequential_predicates -- --test-threads=1
cargo test --locked --test sim_logical_ops --test sim_unique_priority --test sim_array_conditional_assignments -- --test-threads=1
```

R06 was implemented without Rust/Cargo or a built `llg` in the working environment;
the Rust tests, native C++ bridge and public-CLI HDL path were not compiled/run.
The handwritten `sequential_predicate_*` native probes, runtime component suites
and fixture-integrity checks are separate evidence, not end-to-end acceptance.
Pattern matching/bindings remain outside the implemented Boolean-clause subset.

## Corrective regressions after R01-R06

Positive CLI fixtures that expect empty runtime stderr terminate explicitly with
`$finish(0)`. Do not suppress simulator end-of-process, deadlock, or verbose
`$finish` reports in the shared harness to make an unfinished fixture pass.

The focused frontend tests additionally cover iterator metadata during semantic
node-vector growth, bound nested-default array values (including repeated
operand identities and their distinct slot indices), and concatenated inout
actuals that have an expression but no single high-side declaration. CLI cases
`array_conditional_assignments/nested_defaults` and
`port_net_types/concat_actual` check declaration order and connectivity in both
optimizer modes. The original alias expected values and NBA fixture remain the
regression oracles. Generated-name and delay-group assertions inspect the
relevant emitted operations/resolvers, not lowerer-private temporary names or
unrelated input-wire groups.

These corrective Rust/frontend/HDL tests require the normal Rust and native shim
build. Native value-runtime component results alone do not validate the capture
or lowering changes.

The nested-default regressions distinguish untyped bit-filling patterns from
explicitly typed row patterns. `nested_defaults.sv` supplies a one-bit default
and checks `ff`/`00` bytes; `typed_defaults.sv` retains the `dd`/`ee` byte checks
using a row type as the default value's context, plus direct packed patterns,
packed element sizing and signed results. `deep_defaults.sv` checks shared
operands across multiple generated rows and 65-bit values. The semantic-slot
test checks the raw snapshot before import for extents 1, 2, 3 and 17, then
checks generation after snapshot destruction in both optimizer modes. These
tests do not relax positional-count validation or the CLI diagnostic checks.

- `sim_review_tasks08_11.rs` uses checked-in source fixtures in both optimizer
  modes for tagged pattern-case match modes and whole fixed-value pattern
  bindings, snapshot isolation, sequential suppression and scope rejection. Ordinary inactive-member errors
  remain covered by `sim_tagged_union_access`; mode-aware pattern matching does
  not weaken those access guards.
