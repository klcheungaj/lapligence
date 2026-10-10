# LRM decision register

This register lists the semantic decisions llg has made where the IEEE text needed
interpretation, was silent, or where llg deliberately changed existing
behaviour. Each decision has a portable test case, so its behaviour can be compared
with another simulator.

**A trusted simulator wins.** The cases are meant to be run on a trusted
commercial simulator. If that simulator's behaviour differs from a case's
expected output, llg adopts the simulator's behaviour. The same applies where the
text leaves the result open, unless the case is listed as llg policy and the
simulator's output is simply a different permitted outcome (see
[On a mismatch](#on-a-mismatch)).

Clause line numbers refer to `pdftotext -layout` extractions of the
standards: `SystemVerilog-1800-2009.txt` (SV) and `Verilog-1364-2001.txt` (V2001).
These are not part of the repository; the clause numbers are enough to find the
text in the published PDFs.

## Running a case

The cases are in
[`tests/fixtures/sim/lrm_decisions/`](../tests/fixtures/sim/lrm_decisions/).
Each `<ID>_<name>.sv` file is:

- standalone, plain IEEE 1800-2009 SystemVerilog;
- deterministic, with top module `tb`;
- run without tool-specific options, printing its results with `$display`
  and ending with `$finish`;
- headed by a comment that quotes the clause and states the decision.

`<ID>_<name>.out` holds the exact expected stdout.

### On another simulator

1. Compile and run the `.sv` file alone, with `tb` as the top module.
2. If there is a `.c` companion of the same name (only B8), build it as a
   shared DPI-C library and load it with the simulator's DPI library option.
3. Compare stdout with the `.out` file. Ignore the simulator's own banners and
   its `$finish` message.
4. **Negative cases have an empty `.out`.** For AA-D7, the expected result is
   a compile error, and no simulation should run.

### In llg

[`tests/sim_lrm_decisions.rs`](../tests/sim_lrm_decisions.rs) runs every case
through the public `llg` command line:

- on the legacy, compact/portable and compact/GMP value backends;
- in both optimizer modes;
- requiring the exact `.out` stdout and identical stderr across backends.

For B8, the test builds the `.c` companion with the host C compiler and passes
it with `--dpi-lib`; this case runs on Unix hosts only. AA-D7 must fail to
compile with "`$past` of a real expression is illegal". The scripted run is in
[tests/readme.md](../tests/readme.md):

```sh
scripts/run-tests.sh -E 'test(/^sim_lrm_decisions::/)'
```

## Kinds of decision

| Kind | Meaning |
| --- | --- |
| LRM text | The quoted text determines the result; llg was changed to follow it. |
| llg choice | The text is silent or ambiguous; llg picked the behaviour shown. |
| llg extension | llg applies a rule beyond what the text requires. |
| llg policy | The text explicitly leaves the result unspecified. Only lines that the case labels `llg` show llg's choice; the other lines are what every conforming simulator prints. |

## Concurrent assertions and sampled values (audit A)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| AA-D1 | **One result per attempt.** An implication whose antecedent matches several times is one evaluation attempt with one result. It fails as soon as one consequent fails; its pass action does not run for a failed attempt. | 16.13.6 L24614-24619; 16.15.3 L26601-26606 | `AA-D1_implication_one_result` | LRM text |
| AA-N1 | **When a pass is reported.** An attempt reports its pass once every consequent has passed and the antecedent can no longer match. An antecedent that can always match again (`x[->1:$] \|-> ...`) therefore leaves the attempt pending unless a consequent fails. | 16.13.6 L24617-24619 (silent on the reporting time) | `AA-N1_implication_result_time` | llg choice |
| AA-D2 | **`iff` on the leading clock.** An assertion that inherits `default clocking @(posedge clk iff en)`, or has an explicit `iff` on its leading clock, counts only the edges where the `iff` holds as clock ticks, including later ticks of `##n`. An `iff` on a nested (non-leading) clock is rejected. | 16.17 a) L27646-27648; 9.4.2.3 L11987-11988 | `AA-D2_default_clocking_iff` | LRM text; the nested-clock rejection is an llg limit |
| AA-D3 | **`cover sequence` counts every match.** `cover sequence` runs its pass statement once per match, with multiplicity. `cover property` succeeds at most once per attempt. | 16.15.3 L26590-26592, L26625-26627 | `AA-D3_cover_sequence_all_matches` | LRM text |
| AA-D4 | **Leading `##[0:n] r` keeps the empty match.** It behaves like `##0 r` for the zero delay. | F.3.4.2.2 L70580-70581; 16.7 L21795; 16.9.2 L22497-22498 | `AA-D4_leading_range_empty` | LRM text |
| AA-D5 | **Matches are counted with multiplicity.** A sequence has one match per distinct way of matching. This covers `or`, `and` pairs, `first_match` and match items, which run once per match. Ways that differ only in how the empty word is matched count once. When match items or local variables require it, llg enumerates paths up to a thread budget and reports an error if the budget is exceeded. | 16.9.7 L23414-23416; 16.9.5 L23159-23163; 16.9.8 L23475-23476; 16.11 L24156-24157; 16.15.3 L26625-26627 | `AA-D5_match_multiplicity` | LRM text. Counting empty-word ways once is an llg choice (the text is silent; read literally, `r[*0:$]` over an empty-admitting `r` would have infinitely many matches). |
| AA-D6 | **Event-clock gates are read when the event occurs.** The `$past` gate of an event-list or named-event clock is evaluated at that instant, like an `iff`. | 16.9.3 L22767-22770; 9.4.2.3 L11987-11988 | `AA-D6_event_clock_gate_instant` | LRM text |
| AA-D7 | **Real operands are rejected.** A `real`, `shortreal` or `realtime` operand of `$past`, `$rose`, `$fell`, `$stable` or `$changed` (and of the global-clocking forms) is a compile error in a concurrent assertion. llg rejects the same operands in procedural calls; `$sampled` still accepts them. | 16.6 L21543-21544; 16.6.1 L21574-21576 | `AA-D7_real_sampled_illegal` (negative: compile error, empty `.out`) | LRM text in assertions; llg extension for procedural calls |
| AA-I1 | **Nested programs and modules.** A portless program or module nested in a module, and not instantiated explicitly, runs as one implicit instance named after its declaration, and shares the enclosing scope's variables. | 24.3 L43207-43210; 23.4 L41758-41760 | `AA-I1_nested_program` | LRM text |
| — | **`expect` starts one attempt.** An `initial` block that holds only `expect` runs, and an armed `expect` starts exactly one attempt rather than one per clock. | 16.18 L27950-27951 | covered by the assertion suites, no separate case | LRM text |

## Processes, waits and built-in classes (audit B)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| B1 | **String class properties.** `wait` and `@` on a string class property wake when the property changes. | 9.4.2 L11822-11824; 9.4.3 L12032-12033 | `B1_string_property_wait` | LRM text |
| B2 | **Reads inside called functions.** `wait (f(h) == ...)` and `@(f(h))` wake when `f` reads an object member through a class-handle formal. Reads llg cannot observe are rejected with a located error instead of hanging. | 9.4.2 L11837-11840 | `B2_function_member_wait` | LRM text |
| B3 | **`resume()` resensitizes event controls.** A process suspended while blocked on an event control does not see an occurrence that happens while it is suspended; after `resume()` it waits for a new one. A wait condition that became true, or a delay that expired, completes on resume. | 9.7 L12644-12648 | `B3_resume_resensitize` | LRM text |
| B4 | **Handle writes other than assignments.** `@(h)` wakes when a mailbox `get`/`peek`, an `output`/`ref` copy-out or a task-form `$cast` changes `h`. A wait selected through a handle property follows a rebinding made by a task output. | 9.4.2 L11830-11831, L11837-11840 | `B4_handle_write_events` | LRM text |
| B8 | **Context imports in `$monitor`/`$strobe`.** A context import in a `$monitor`/`$strobe` argument is accepted when the design declares no DPI exports. It is rejected only when exports exist, because only then could it write design state during Postponed. | 4.4.2.9 L3212-3214; 35.5.3 L55824-55826 | `B8_context_import_strobe` (`.sv` and `.c`) | LRM text (llg's reading of the two clauses together) |
| AB-N1 | **Suspended queue waiters.** A process suspended while blocked in semaphore `get` or mailbox `get`/`peek` is not a candidate for keys or messages; others take them. `resume()` re-queues it at the FIFO tail. | 9.7 L12618, L12644-12648 | `AB-N1_suspended_queue_waiter` | llg choice (the text does not address queue membership). Treating suspended mailbox `put` waiters the same way, so that they do not take freed space, is an llg extension. |
| AB-N2 | **`always_comb` and class references.** Class property paths and method calls add nothing to `always_comb` sensitivity, not even the handle variable `h` in `h.x`/`h.f()`. `always @*` still adds the identifier `h`. | 9.2.2.2.1 L11192-11194; 9.4.2.2 L11899-11900 | `AB-N2_always_comb_class_reference` | LRM text. A direct `h == null` read in `always_comb` still adds `h`, which is an llg extension. |
| AB-N3 | **Disabling a whole fork branch.** If the named block that forms a fork branch's entire body is disabled, `status()` of that process returns `KILLED`. A disabled block followed by more statements, or a disabled top-level `initial begin : blk`, still ends `FINISHED`. | 9.7 L12615-12619; 9.6.2 L12387-12388 | `AB-N3_disable_branch_status` | LRM text for KILLED; limiting it to whole fork branches is an llg choice |
| AB-O1 | **`with` evaluation order.** Each locator or reduction method evaluates its `with` expression exactly once per element, in index order (key order for associative arrays). | 7.12 L9255; 7.12.1 L9267 | `AB-O1_with_evaluation_order` (the whole trace is policy) | llg policy |
| AB-O2 | **`unique` order.** `unique()`/`unique_index()` keep the first occurrence of each value, in index order. | 7.12.1 L9295-9301 | `AB-O2_unique_order` (`llg` lines are policy, `sorted` lines are portable) | llg policy |
| AB-O3 | **Killing a woken receiver.** A mailbox message leaves the queue when `put` hands it to a waiting receiver. Killing that receiver before it runs returns the message to the head of the queue. | 9.7 L12630-12634 | `AB-O3_kill_woken_receiver` (`llg` lines are policy, the `conserved` line is portable) | llg policy |

## Bit-stream operations (SIM-020)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S20-1 | **Strings stream as bytes.** A string streams as its bytes, index 0 leftmost. A stream assigned or unpacked into a string fills it left to right, one character per 8 bits, and drops zero bytes. | 6.24.3 L7514-7515, L7528; 6.16 L5641-5642; 11.4.14 L15472-15474 | `S20-1_string_stream_bytes` | LRM text |
| S20-2 | **Greedy dynamic targets.** In one unpack, the first unselected dynamic array or queue takes every bit that the fixed targets after it leave. Each later unselected one becomes empty. | 11.4.14.4 L15600-15602; 6.24.3 L7522-7526 | `S20-2_greedy_dynamic_targets` | LRM text |
| S20-3 | **Dynamic targets are zero-filled.** A stream assigned to a dynamic array or queue gets as many elements as hold the whole stream. The last element is zero-filled on the right. | 11.4.14 L15472-15474 | `S20-3_dynamic_target_fill` | LRM text |
| S20-4 | **`with` ranges grow but never shrink.** A `with` range on a dynamic unpack target grows the array when the range reaches past its end; new elements read as zero. It never shrinks the array, and elements outside the range keep their values. | 11.4.14.4 L15629-15630, L15643-15645 | `S20-4_with_range_resize` | llg choice (the text says "resized to accommodate" without saying whether a larger array shrinks) |
| S20-5 | **`<<` unpack consumes, then reverses.** A `<<` unpack first takes from the left of the source the bits its targets need, then reverses those blocks. `with` extents are resolved before the reordering. | 11.4.14.3 L15561-15566 | `S20-5_reverse_unpack_consumed` | llg choice (the text defines the consumed bits but not how `<<` orders a partial source) |
| S20-6 | **Casts to dynamic types need whole elements.** An explicit bit-stream cast to a dynamic array or queue type needs a source that is a whole number of elements; any other size is a run-time error. An uncast stream assignment still zero-fills (S20-3). | 6.24.3 L7534-7537 | `S20-6_dynamic_cast_whole_elements` (negative at run time: the `.out` holds the output before the error) | llg choice |

## Earlier decisions

These predate the register. Their evidence lives in the feature fixtures named
below, which use llg's normal harness rather than the standalone `tb` format.

| ID | Decision | Clause | Fixtures | Kind |
| --- | --- | --- | --- | --- |
| E1 | **`iff` and `$past` gates read current values.** An `iff` qualifier and the `$past` gating expression are evaluated with current (not sampled) values when the clock fires. | SV 16.9.3 L22767-22770; 9.4.2.3 L11987-11988; 16.5 L21496-21497 | [sim_035](../tests/fixtures/sim/feature_completion/sim_035/readme.md) `procedural_history`, `complex_clocks`; see also AA-D6 | LRM text |
| E2 | **`%p` of singular values.** `%p` prints a singular value (other than enums, strings and handles) as an unformatted `$display` argument would. | SV 21.2.1.7 L36415 | [sim_024](../tests/fixtures/sim/feature_completion/sim_024/readme.md) `integral_sizes`, `aggregates` | LRM text |
| E3 | **Procedural continuous assignments.** Procedural `assign`/`deassign` are rejected in every form. Hierarchical continuous assignments and `force`/`release` are supported. | V2001 9.3 L8805 | [adv_032](../tests/fixtures/sim/feature_completion/adv_032/readme.md) `assign_forms`, `assign_hierarchical`, `hier_continuous_assign`, `hier_force_release` | llg choice (a supported subset) |
| E4 | **`%p` of a packed union.** A packed union prints its first declared member. This is still an open question. | SV 21.2.1.7 L36389 | [sim_024](../tests/fixtures/sim/feature_completion/sim_024/readme.md) `aggregates` | llg choice (the sentence is written for unions in general) |

## Known deviations not fixed

These are documented in [known issues](known_issues.md) and have no decision case yet.

- **B5:** a message handed back by a killed receiver returns to the head of a
  bounded mailbox, so `num()` can exceed the bound until the next `get`
  ([mailbox message forms](known_issues.md#mailbox-message-forms-without-a-nested-value)).
- **B6:** mailboxes, semaphores, named events and pinned process handle records
  stay allocated until the model closes
  ([collected objects](known_issues.md#collected-objects-keep-some-runtime-objects-and-defer-collection)).
- **B7:** `status()` inside a `wait` condition or an event expression is
  rejected ([rejected process-handle forms](known_issues.md#rejected-process-handle-forms)).

## On a mismatch

1. Record the case ID, the simulator and version, and the differing stdout
   (or the diagnostic, for a negative case).
2. For an llg policy case, check whether the difference is only in the lines
   labelled `llg`. If the portable lines match, the result conforms.
   Adopting the other simulator's order is still a valid request.
3. Otherwise llg changes to match the trusted simulator. The case's `.out` file
   and header comment, this register, and any affected rows in
   [sim_features.md](sim_features.md) or [known_issues.md](known_issues.md)
   are updated with it.
