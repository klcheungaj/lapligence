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
4. **Negative cases have an empty `.out`.** For AA-D7, S14-D5 and S23-D2, the
   expected result is a compile error, and no simulation should run.

### In llg

[`tests/sim_lrm_decisions.rs`](../tests/sim_lrm_decisions.rs) runs every case
through the public `llg` command line:

- on the legacy, compact/portable and compact/GMP value backends;
- in both optimizer modes;
- requiring the exact `.out` stdout and identical stderr across backends.

For B8, the test builds the `.c` companion with the host C compiler and passes
it with `--dpi-lib`; this case runs on Unix hosts only. AA-D7 must fail to
compile with "`$past` of a real expression is illegal", S14-D5 with
"nonblocking assignment to class property `x`", and S23-D2 with "is an
unpacked structure or union, which is not a singular variable". The scripted run is in
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

## Repeated waits, intra-assignment timing and event triggers (SIM-014)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S14-D1 | **Real repeat counts round.** A real count of a repeat loop or a repeated event control (standalone, intra-assignment or `->>`) converts to an integer by rounding, ties away from zero; a result of zero or less waits for nothing. | 9.4.5 L12140-12141; 12.7.2 L18159-18160; 6.12.2 L5531-5533 | `S14-D1_real_repeat_count` | llg choice (the repeat clauses do not mention real counts; llg applies the 6.12.2 conversion) |
| S14-D2 | **Nonpositive and X/Z counts do not wait.** A repeated intra-assignment or `->>` control whose count is zero, negative (signed) or contains X/Z waits for no event: a blocking assignment completes at once; an NBA or nonblocking trigger takes effect in the current time step. | 9.4.5 L12140-12149; 12.7.2 L18159-18160 | `S14-D2_nonpositive_repeat_immediate` | llg choice ("as if there is no repeat construct" is read with the `repeat (-3)` example as "no event is waited for"; the repeat-loop X/Z rule is applied to repeat event controls) |
| S14-D3 | **`->>` names its event at issue.** A timed nonblocking trigger triggers the event its operand names when the statement executes; rebinding or nulling that event variable while the trigger is pending does not redirect it. | 15.5.1 L20708-20711; 10.4.2 L13070-13071 | `S14-D3_nb_trigger_target_at_issue` | llg choice (15.5.1 does not say when "the referenced event" is resolved; llg treats it like an NBA target) |
| S14-D4 | **The first `wait_order` event may already be triggered.** An event triggered earlier in the same time step satisfies the first position of `wait_order`; the later events need new triggers. | 15.5.4 L20826; 15.5.3 L20735-20736 | `S14-D4_wait_order_first_triggered` | LRM text (llg's reading of "can wait for the persistent triggered property") |
| S14-D5 | **No NBA to class properties.** A nonblocking assignment to a class property, with or without timing control, is a compile error with the property name and location. | 6.21 L7007-7008 | `S14-D5_class_property_nba` (negative: compile error, empty `.out`) | LRM text (1800-2009; 1800-2012 relaxed the rule) |
| S14-D6 | **A failing `wait_order` without `else` continues.** The run-time error is reported like `$error` (an error message on stderr) and the process continues with the next statement. | 15.5.4 L20828-20830 | `S14-D6_wait_order_error_continues` | llg choice (the text does not say whether the error ends the simulation) |

## Force and release targets (SIM-023)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S23-D1 | **Release re-establishes a continuous driver.** Releasing a variable driven by a continuous assignment (including a constant one, a real one and an output port connection) reruns that driver as an ordinary Active-region event after the release; the releasing process itself still reads the forced value until it suspends. | 10.6.2 L13393-13395 | `S23-D1_release_reestablishes_continuous` | LRM text (the case reads one time unit later, so the order inside the release time step is not observed) |
| S23-D2 | **Non-singular force targets are illegal.** Forcing or releasing a whole unpacked structure, union or array variable is a compile error with the target name and location. | 10.6.2 L13373-13375; 6.4 L4606-4607 | `S23-D2_force_non_singular` (negative: compile error, empty `.out`) | LRM text |
| S23-D3 | **Packed member and element selects of nets are part-selects.** A constant member select of a packed-structure net, a constant element select of a packed-array net and a constant indexed part-select of a net are constant part-selects of a vector net: they are forced and released bit-exactly while the other bits follow the drivers. | 10.6.2 L13373-13374; 7.2.1 L7696-7700; 11.5.1 L15779-15782 | `S23-D3_net_member_select` | llg choice for member and element selects (10.6.2 names only bit- and part-selects); LRM text for indexed part-selects |
| S23-D4 | **Forces and releases combine bit by bit.** Each net bit belongs to the latest force that covered it, so a later overlapping force replaces only the overlapped bits; a release of a part of a forced range releases exactly those bits and the rest stay forced. | 10.6.2 L13373-13374, L13397-13399; 4.9.2 L3519 | `S23-D4_partial_release` | llg choice (the text does not say how overlapping forces and partial releases combine) |

## Bit-stream operations (SIM-020)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S20-1 | **Strings stream as bytes.** A string streams as its bytes, index 0 leftmost. A stream assigned or unpacked into a string fills it left to right, one character per 8 bits, and drops zero bytes. | 6.24.3 L7514-7515, L7528; 6.16 L5641-5642; 11.4.14 L15472-15474 | `S20-1_string_stream_bytes` | LRM text |
| S20-2 | **Greedy dynamic targets.** In one unpack, the first unselected dynamic array or queue takes every bit that the fixed targets after it leave. Each later unselected one becomes empty. | 11.4.14.4 L15600-15602; 6.24.3 L7522-7526 | `S20-2_greedy_dynamic_targets` | LRM text |
| S20-3 | **Dynamic targets are zero-filled.** A stream assigned to a dynamic array or queue gets as many elements as hold the whole stream. The last element is zero-filled on the right. | 11.4.14 L15472-15474 | `S20-3_dynamic_target_fill` | LRM text |
| S20-4 | **`with` ranges grow but never shrink.** A `with` range on a dynamic unpack target grows the array when the range reaches past its end; new elements read as zero. It never shrinks the array, and elements outside the range keep their values. | 11.4.14.4 L15629-15630, L15643-15645 | `S20-4_with_range_resize` | llg choice (the text says "resized to accommodate" without saying whether a larger array shrinks) |
| S20-5 | **`<<` unpack consumes, then reverses.** A `<<` unpack first takes from the left of the source the bits its targets need, then reverses those blocks. `with` extents are resolved before the reordering. | 11.4.14.3 L15561-15566 | `S20-5_reverse_unpack_consumed` | llg choice (the text defines the consumed bits but not how `<<` orders a partial source) |
| S20-6 | **Casts to dynamic types need whole elements.** An explicit bit-stream cast to a dynamic array or queue type needs a source that is a whole number of elements; any other size is a run-time error. An uncast stream assignment still zero-fills (S20-3). | 6.24.3 L7534-7537 | `S20-6_dynamic_cast_whole_elements` (negative at run time: the `.out` holds the output before the error) | llg choice |

## Postponed output (SIM-025)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S25-D1 | **The monitor flag outlives the display list.** `$monitoroff` clears one flag; a `$monitor` issued while it is clear registers a list that stays silent until `$monitoron`, which then prints once even if no value changed. | 21.2.3 L36536-36541 | `S25-D1_monitor_flag_persists` | llg choice (the text calls it "a monitor flag" set only by `$monitoron`/`$monitoroff`; it does not say that a new `$monitor` re-enables it) |
| S25-D2 | **Any number of `$fmonitor` lists are active together.** They are independent of `$monitor` and of each other (also on one file); `$monitoron/$monitoroff` do not affect them; `$fclose` cancels a list for the channel it closes, and a multichannel list keeps its other channels. | 21.3.2 L36676-36679; 21.3.1 L36635-36636 | `S25-D2_multiple_fmonitor` | LRM text for independence and cancellation; llg choice for the partial close of a multichannel list |
| S25-D3 | **A value that returns within the slot does not report.** The settled values at the end of the slot are compared with the printed ones, per argument expression: a variable that changes and returns, or an expression whose operands change but whose value does not, prints nothing. | 21.2.3 L36516-36519, L36530-36531 | `S25-D3_value_returns_within_slot` | llg choice (the text says "changes value" and "end of the time step" without addressing intermediate values) |
| S25-D4 | **Reports in static subroutines are legal.** `$strobe`/`$monitor`/`$fstrobe` in static tasks and functions may name module variables and static formals and locals; only automatic variables are barred. | 13.3.2 L18646-18652 | `S25-D4_static_subroutine_reports` | LRM text |
| S25-D5 | **Pending reports at `$finish` are dropped.** The Postponed region of the slot that executes `$finish` does not run, so its `$strobe` and `$monitor` lines do not print. | 20.2 L34119; 21.2.2 L36482-36484 | `S25-D5_pending_report_at_finish` (the missing lines are policy) | llg policy |
| S25-D6 | **Order of reports in a slot.** Strobes print in call order, then monitors (`$monitor` and `$fmonitor`) in registration order; a replaced `$monitor` takes the position of its replacement. | 4.4.2.9 L3210 | `S25-D6_report_order_in_slot` (the whole trace is policy) | llg policy |

## Programs and program bind (SIM-032)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S32-D1 | **The implicit `$finish` of programs is immediate.** When the last thread originating in a program initial ends, llg calls `$finish` at that point, as if that thread had called it: events still pending in the same time slot (a Re-NBA update, the design processes it would wake) do not run, and final procedures see the last committed values. | 24.3 L43229-43232; 9.2.3 L11258-11261 | `S32-D1_implicit_finish_immediate` | llg choice (the text says "immediately after" the threads end without saying whether pending same-slot events run first) |
| S32-D2 | **Programs can be bound into interfaces.** A `bind` whose target scope is an interface may instantiate a program; the program runs once per interface instance. | 23.11 L42888-42893, L42897-42901; A.1.4 L40954-40957 | `S32-D2_bind_program_into_interface` | llg choice (the prose names only interfaces as code bound into interfaces; the grammar allows a program instantiation as an interface item and an interface as a bind target) |

## Synchronous drives and clocking (SIM-034)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S34-D1 | **A cycle-delayed drive does not block.** `cb.v <= ##n e` evaluates `e` and `n` when it executes and matures `n` events of the target's own clocking block later; issued away from an event it counts from the next event (`##0` has no effect). Drives that mature in the same Re-NBA region commit in issue order, so the last issued value is driven. | 14.16 L20103-20108, L20144-20147; 14.16.2 L20194-20195, L20244-20260; 14.11 L19824-19828 | `S34-D1_cycle_drive_maturation` | LRM text (the clause's own example; llg blocked the process for the cycle count before) |
| S34-D2 | **A clocking output on a net has its own driver.** The driver starts at `'z`, has strong strength and resolves with the net's other drivers. | 14.16 L20069-20074 | `S34-D2_clocking_net_driver` | LRM text (llg wrote into the net's existing driver before) |
| S34-D3 | **A drive through a virtual interface is bound when it executes.** It drives the clockvar of the instance the handle names at that moment, with that instance's clocking event and skew; rebinding the handle before the drive matures does not redirect it. | 25.9 L44891-44893; 14.16 L20144-20147 | `S34-D3_vif_drive_bound_at_issue` | llg choice (the text does not say when the handle of a pending drive is resolved; llg treats it like an NBA target) |
| S34-D4 | **Design processes woken in Observed run before the reactive set.** A module process waiting for a clocking block event resumes in the Active region of the same pass, before program processes woken by the same event run in Reactive; its drives commit in the following Re-NBA region. | 14.13 L19921-19923; 4.5 L3386-3392; 24.3.1 L43248-43249 | `S34-D4_observed_wakeup_before_reactive` | LRM text (llg ran the reactive set first before) |

## File input (SIM-026)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S26-D1 | **Destination selectors are evaluated when the call starts.** Index and select expressions of every `$fscanf`/`$sscanf`/`$fgets`/`$fread` destination are evaluated once, before any conversion, so `$fscanf(fd, "%d %d", i, a[i])` writes the element chosen by the old `i`. | 21.3.4.3 L36880-36881; 13.5.1 | `S26-D1_scan_selector_snapshot` | llg choice (the text does not order destination evaluation against earlier conversions) |
| S26-D2 | **Input from an invalid descriptor fails like ended input.** On a closed, unopened, multichannel, X or write-only descriptor `$fgetc`, `$ungetc` and `$fscanf` return EOF, `$fgets` and `$fread` return 0, `$feof` returns -1, and destinations are unchanged. | 21.3.4.3 L37024-37025; 21.3.8 L37231-37232 | `S26-D2_invalid_descriptor_input` | llg choice (the text gives no result for a descriptor that is not open) |
| S26-D3 | **`%u`/`%z` use 32-bit vecval words.** Words are least significant first, little-endian bytes, `%z` an aval word then a bval word; a read that meets end of file after some bytes assigns nothing and counts 0, with no bytes left it returns EOF. | Table 21-8 L36944-36946, L36958-36959, L36988-36990 | `S26-D3_unformatted_words` | llg choice ("native endian format"; short data is not covered) |
| S26-D4 | **`%t` rounds, then scales.** The value read is rounded to the `$timeformat` precision in its unit, then scaled into the calling scope's time unit. | Table 21-8 L36933-36938 | `S26-D4_scan_time_scaling` | LRM text (the clause's own example) |
| S26-D5 | **`%v` assigns the strength's value.** A mnemonic or two digits followed by 0/1/X/Z/L/H, or `HiZ`; L and H read as X; the value goes to bit 0 with the other bits cleared. | Table 21-8 L36927-36931 | `S26-D5_scan_strength` | llg choice (the four-value equivalent of L/H and the placement are not stated) |
| S26-D6 | **Integer conversions into real destinations convert the value.** `%d`/`%h`/`%o`/`%b` into a `real` assign the integer as a real, as an assignment would. | 21.3.4.3 L37001-37002 | `S26-D6_integer_scan_into_real` | llg extension (the text names integral destinations) |
| S26-D7 | **A `$fread` word cut short fills its high bytes.** Bytes read fill from the most significant byte down; the remaining low bytes keep their value; the count is the bytes read. | 21.3.4.4 L37075-37078 | `S26-D7_fread_short_last_word` | llg choice (end of file inside a word is not covered) |
| S26-D8 | **`$fgets` into a vector reads at most its whole bytes, right-justified.** The last character read is the least significant byte; unused high bytes are 0. | 21.3.4.2 L36830-36832 | `S26-D8_fgets_packed_capacity` | llg choice (the placement of a short line is not stated) |
| S26-D9 | **`%s` into a byte array is right-justified.** The array reads as a vector with its left-most element most significant: the last character goes to the right-most element, unused elements become 0, a longer field keeps its last characters. | 21.3.4.3 L37005-37006 | `S26-D9_scan_string_into_byte_array` | llg choice (element order is not stated) |
| S26-D10 | **`%*u` and `%*z` end the call.** Their field size comes from the destination, which a suppressed conversion lacks, so they are a matching failure and read nothing. | Table 21-8 L36879-36881, L36944-36945 | `S26-D10_suppressed_binary_conversion` | llg choice |
| S26-D11 | **A successful reposition clears the error status.** `$fseek` and `$rewind` clear `$ferror` and the end-of-file indicator; other successful operations keep the last error. | 21.3.7 L37203-37205; 21.3.5 L37143 | `S26-D11_seek_clears_error` | llg choice ("most recent operation" is read as the most recent failing or repositioning operation) |
| S26-D12 | **An integral `$sscanf` source drops leading zero bytes.** It reads as the characters of a string literal stored in a vector; a zero byte inside is white space. | 21.3.4.3 L36858, L36873-36874 | `S26-D12_packed_scan_source` | LRM text for null bytes; llg choice for leading padding |
| S26-D13 | **`%m` counts as an assigned item.** It assigns the scope name without reading input and is counted in the result. | Table 21-8 L36995-36996; 21.3.4.3 L37023 | `S26-D13_scope_conversion_counts` | llg choice |
| S26-D14 | **`$ungetc` returns 0 on success.** | 21.3.4.1 L36816-36818; V2001 17.2.4.1 L18994-18995 | `S26-D14_ungetc_returns_zero` | LRM text (llg returned the character before) |
| S26-D15 | **`$fwrite("%z")` writes vecval bits.** X is aval 1, bval 1 and Z is aval 0, bval 1. | Table 21-8 L36988-36990; 38.15 L62602 | `S26-D15_fwrite_z_encoding` | LRM text (llg wrote Z as X before) |

## Operator overloading (SIM-021)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S21-D1 | **Overloading an already-legal operator is an error.** A `bind` declaration whose operator is already legal for its formal types, and whose built-in result is assignable to its result type (`int + int` returning `int`, a same-type record `=` or `==`, `real` to `int` by `=`, `string < string`), is rejected at the declaration. | 11.11 L16473-16476, L16562-16563 | `S21-D1_builtin_legal_overload` | llg reading (the text says such operators "cannot be overloaded" but not whether the declaration is an error or has no effect; llg admitted it without effect before) |
| S21-D2 | **A prototype with a result the built-in cannot produce is admitted.** Formals that are legal for the built-in operator with an unpacked-record result (the clause's own `fcopyi(int)` for unary `+`) are admitted; legal uses keep their built-in meaning. | 11.11 L16473-16476, L16488-16489 | `S21-D2_record_result_overload_admitted` | llg reading |

## Random streams and legacy distributions (SIM-028)

The generator behind `$urandom`, process and object streams and `shuffle` is
implementation dependent, so those cases check relations only. `$random` and
`$dist_*` are fixed by Annex N, so their cases print numbers (computed by the
independent transcription in
[sim_028](../tests/fixtures/sim/feature_completion/sim_028/readme.md)).

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S28-D1 | **`$random` without a seed starts from 0.** The implicit seed of `$random` starts at 0, so its first values are the Annex N values for seed 0 (303379748, -1064739199, ...). | 20.15.1 L35315-35316; N.1 Table N.1 L76887; N.2 L77212-77213 | `S28-D1_implicit_random_seed` | llg choice (the text does not give the implicit seed's initial value) |
| S28-D2 | **Seed variables of other widths.** A seed variable is read as a 32-bit integer (truncated, or extended by its own signedness), and the signed 32-bit result seed is assigned back like an `integer` (sign-extended or truncated). | 20.15.1 L35315; 20.15.2 L35355-35356; Annex N.2 | `S28-D2_seed_writeback_width` | llg choice (the text does not say how a seed of another width converts) |
| S28-D3 | **X/Z seed bits read as 0.** An X/Z bit of a `$random` or `$dist_*` seed reads as 0, as in a 4-state to 2-state conversion. | 20.15.2 L35355-35356; 6.11.2 L5489-5490 | `S28-D3_unknown_legacy_seed` | llg choice (the text does not say how an unknown seed is read) |
| S28-D4 | **`new` takes one value from the creating thread.** Creating an object seeds its stream with the creating thread's next value, the value its next `$urandom` would have returned. An object created by a static declaration initializer takes the value from a stream of its instance's initialization RNG. | 18.14.1 L31161-31166; 18.13.3 L31089 | `S28-D4_object_creation_draw` | LRM text (that the value is the one `$urandom` would return is llg's reading) |
| S28-D5 | **`shuffle` draws from the calling thread.** `shuffle()` uses the stream of the thread that calls it, also inside a class method; the object's own stream is not used. | 7.12.2 L9366; 18.14.1 L31152-31153; 18.15 L31270 | `S28-D5_shuffle_thread_stream` | llg choice (the text does not name the stream shuffle uses) |
| S28-D6 | **A shallow copy copies the RNG state.** `new h` copies `h`'s stream state and takes no value from the creating thread. | 8.11 L9990-9998; 18.14.1 L31162-31163 | `S28-D6_shallow_copy_random_state` | LRM text for the copied state; taking no thread value is an llg choice (8.11 lists no seeding step) |
| S28-D7 | **Each instance has its own initialization stream.** A static process is seeded from the initialization stream of the instance that declares it, in that instance's creation order. Each instance's stream derives from the default seed and the instance's hierarchical name, so identical instances draw different values, and processes added to one instance never move another instance's seeds. | 18.14.1 L31148-31156 | `S28-D7_instance_streams` | llg choice (the default seed is implementation dependent; the text says every initialization RNG starts from "the default seed") |
| S28-D8 | **X/Z in 2-state random arguments read as 0.** `srandom`, `$urandom` seeds and `$urandom_range` bounds are 2-state `int`/`int unsigned` formals; X/Z bits read as 0. Real seeds round. | 18.13.1 L31018; 18.13.2 L31044-31045; 18.13.3 L31086; 6.11.2 L5489-5490 | `S28-D8_two_state_random_arguments` | LRM text |
| S28-D9 | **A foreign state string is a run-time error.** `set_randstate` with a string not produced by llg's `get_randstate` prints `invalid randstate string`, leaves the stream unchanged, lets the run continue and makes it exit with status 1. | 18.13.5 L31117-31119 | no portable case: the text says the result is undefined; covered by the [sim_028](../tests/fixtures/sim/feature_completion/sim_028/readme.md) `neg_randstate_invalid` fixture | llg choice |

## Deferred immediate assertions (SIM-036)

| ID | Decision | Clause (SV) | Case | Kind |
| --- | --- | --- | --- | --- |
| S36-D1 | **One report per execution.** Every execution of a deferred assertion queues its own report with its own issue-time arguments; only a flush point clears the process's queue. A loop that fails three times reports three times. | 16.4.1 L21243-21250; 16.4.2 L21276-21281 | `S36-D1_report_per_execution` | LRM text (llg formerly kept only the last result of one assertion per process and time step) |
| S36-D2 | **Flush points.** Resuming after `wait (...)`, `@(...)`, `wait fork` or `wait_order` flushes the whole queue of the process, as does re-triggering an `always_comb`/`always_latch`. A delay control (`#0`, `#n`), a fork-join and blocking built-in methods (semaphore, mailbox, process `await`) are not flush points. | 16.4.2 L21276-21281 | `S36-D2_flush_point_kinds` | LRM text; the exclusions are llg's reading of "event control or wait statement" |
| S36-D3 | **Method-call receiver at issue time.** A method-call action runs on the object the handle referred to when the assertion executed. | 16.4 L21220-21226; 16.4.1 L21244-21246 | `S36-D3_receiver_at_issue` | llg choice (the text covers arguments only) |
| S36-D4 | **Final procedures.** A report queued by a final procedure executes, with its issue-time arguments, when that final procedure returns. | 9.2.3 L11256-11259; 16.4.1 L21262-21265 | `S36-D4_final_procedure_reports` | llg choice (no Observed region follows the finals; the text is silent) |
| S36-D5 | **`$assertkill` spares matured reports.** A kill flushes only reports that have not matured. When a Reactive action calls `$assertkill`, the other matured reports of the step still execute. | 20.11 L35054-35057; 16.4.1 L21263-21264 | `S36-D5_kill_spares_matured` | LRM text (llg's reading of "currently executing") |
| S36-D6 | **Reports pending at `$finish` execute.** Reports queued in the step that calls `$finish` (or issued before an action's `$finish`/`$fatal`) execute in issue order before the final procedures. | 20.2 L34119 (silent) | `S36-D6_pending_reports_at_finish` | llg policy (existing behaviour, now recorded; contrast S25-D5) |
| S36-D7 | **Module-item sensitivity.** A deferred assertion used as a module item behaves as an `always_comb`: it re-executes when its condition or its action's arguments change, and the re-execution flushes the earlier report. | 16.4.3 L21341-21342; 9.2.2.2.1 L11184-11186 | `S36-D7_module_item_sensitivity` | LRM text (llg formerly watched only the condition) |
| S36-D8 | **Dynamic ref actuals.** An element of a dynamic array, queue or associative array (like a class property) passed to a `ref`/`const ref` action formal is a compile error. | 16.4 L21226-21227; 6.21 L7007-7008 | `S36-D8_dynamic_ref_actual` (negative: compile error, empty `.out`) | LRM text |

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
