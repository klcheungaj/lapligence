# SIM-047 waveform qualification fixtures

These designs qualify ordinary VCD/FST output after native values, the
collector, the storage pass and the generate naming changes. The Rust module
`tests/sim_feature_completion/sim_047.rs` runs each through the public CLI in
both optimizer modes, reads every VCD with the independent reader
`tests/support/vcd.rs` (IEEE 1800-2009 Syntax 21-20 and Table 21-9; it shares
no code with the writer) and every FST with GTKWave's reader built from the
bundled libfst. The expected traces below are derived by hand from the
stimulus; the `.out` files hold the `$strobe`/`$display` lines, which print
the same end-of-step values a VCD records, so a run on another simulator can
be compared without its VCD header.

Clause text is quoted from `docs/specification/SystemVerilog-1800-2009.txt`
(abbreviated `SV`) with line numbers; IEEE 1364-2001 §18.1-18.2 has the same
wording for the VCD tasks.

## Clauses

- SV L37757-37759 (21.7.1.1): "The filename is optional and defaults to the
  literal string dump.vcd if not specified."
- SV L37768-37769 (21.7.1.2): "The $dumpvars task can be invoked as often as
  desired throughout the model (for example, within various blocks), but the
  execution of all the $dumpvars tasks shall be at the same simulation time."
- SV L37786: "When invoked with no arguments, $dumpvars dumps all the
  variables in the model to the VCD file."
- SV L37788-37791: "the first argument indicates how many levels of the
  hierarchy below each specified module instance to dump to the VCD file.
  Subsequent arguments specify which scopes of the model to dump to the VCD
  file. These arguments can specify entire modules or individual variables
  within a module."
- SV L37794-37795: "The argument 0 applies only to subsequent arguments that
  specify module instances, and not to individual variables."
- SV L37832-37833 (21.7.1.3): "Executing the $dumpvars task causes the value
  change dumping to start at the end of the current simulation time unit."
- SV L37844-37846: "When the $dumpoff task is executed, a checkpoint is made in
  which every selected variable is dumped as an x value. When the $dumpon task
  is later executed, each variable is dumped with its value at that time. In
  the interval between $dumpoff and $dumpon, no value changes are dumped."
- SV L37878-37879 (21.7.1.4): "The $dumpall task creates a checkpoint in the
  VCD file that shows the current value of all selected variables."
- SV L37887-37888: "When dumping is enabled, the value change dumper records
  the values of the variables that change during each time increment. Values
  of variables that do not change during a time increment are not dumped."
- SV L37901-37903 (21.7.1.5): "When the size of the VCD file reaches this
  number of bytes, the dumping stops, and a comment is inserted in the VCD
  file indicating the dump limit was reached."
- SV L37907-37909 (21.7.1.6): "The $dumpflush task can be used to empty the
  VCD file buffer of the operating system to verify all the data in that
  buffer are stored in the VCD file. After executing a $dumpflush task,
  dumping is resumed as before so no value changes are lost."
- SV L37961 (21.7.1.6 Example 2): "$dumpon; //no effect the first time
  through".
- SV L38066: "Strength information and memories are not dumped."
- SV L38068: "A real number is dumped using a %.16g printf() format."
- SV L38073-38074: "The identifier code is a code composed of the printable
  characters, which are in the ASCII character set from ! to ~ (decimal 33 to
  126)."
- SV L38090-38091: "Vector values appear in the shortest form possible:
  redundant bit values that result from left-extending values to fill a
  particular vector size are eliminated."
- SV L41595-41597 (23.3.3.7): "It is permissible to merge the dominating and
  dominated nets into a single net, whose type shall be that of the
  dominating net. The resulting net is called the simulated net".
- SV L46318-46319 (27.4): "the generate block identifier for the loop is
  indexed by adding the “[genvar value]” to the end of the generate block
  identifier."
- SV L46612-46614 (27.6): "Each generate construct in a given scope is
  assigned a number. ... All unnamed generate blocks will be given the name
  “genblk<n>” where <n> is the number assigned to its enclosing generate
  construct."

llg decisions are S47-D1..D13 in `docs/lrm_decisions.md`, each with a
portable case in `tests/fixtures/sim/lrm_decisions/`.

## Fixtures and independent traces

Times below are VCD ticks: `trace_values.sv` has 1 ps precision (`#1` is
1000), the others 1 ns (`#1` is 1). The `$unit` class of `excluded.sv` does not
make the precision finer (decision F2-D2). A trace lists the
end-of-step value of each step with a record; the reader also checks that no
variable has two ordinary records in one step (S47-D2).

- `trace_values.sv` (A01): ports, nets, a real, a two-element array, an inout
  net shared by two stages and generate scopes. Paths: `tb.g[0].first.s` and
  `tb.g[1].second.s` (27.4; no extra `g` level, S47-D10) and `tb.genblk2.extra`
  (27.6: the `if (1)` construct is the second generate construct of `tb`).
  The inout port nets of both stages collapse into `tb.shared` (23.3.3.7), so
  those three names share one identifier code. The `clk` inputs of both stages
  and the `d` input of stage 0 are connected to whole variables of their own
  type, so they share the actual's storage and its code (decision A21-D1):
  `{tb.clk, both stage clk}` and `{tb.d, stage 0 d}`; every other path has its
  own.
  `$dumpvars` runs after `d = 0` and before `d = 3` at step 0, so the
  checkpoint shows `d = 0011` (S47-D1).
  - `clk`, `extra`, both stage `clk`: 0@0, 1@1000, 0@2000, 1@3000.
  - `d`, stage 0 `d`: 0011@0, 1x0z@2000; stage 1 `d` is `~d`: 1100@0,
    0x1x@2000 (`~z` is x).
  - `q0`/stage 0 `q`: xxxx@0, 0011@1000, 1x0z@3000 (the posedge NBA of `d`);
    `q1`/stage 1 `q`: xxxx@0, 1100@1000, 0x1x@3000.
  - `shared`: stage 0 drives `q[0]`, stage 1 drives z, so x@0, 1@1000,
    z@3000.
  - `glitch` is set to 1 and back to 0 within step 1000: 0@0, 0@1000 (one
    record of the final value, S47-D2).
  - `level`: 0.5@0, 0.33333333333333331@2000 (1.0/3.0 written with `%.17g`,
    S47-D12); `mem[0]` 00000000@0; `mem[1]` 11111111@0, 01011010@2000.
- `targets.sv` (step 2): three `$dumpvars` calls at step 0 add up (S47-D3):
  `(1, tb.u1)` selects `u1.i` and `u1.m` but not `u1.l`; `(0, tb.g[1].l)`
  selects that instance; `(0, tb.mem, tb.b)` selects both elements and `b`.
  The `$dumpvars(0, tb)` at step 1 is ignored with the warning
  `$dumpvars at #1 ignored: the dump started at #0`. Traces: `b`
  11@0, 10@1; `u1.i`/`u1.m` 00@0, 01@1; `g[1].l.i` (a+1) 01@0, 10@1;
  `g[1].l.r` (~i) 10@0, 01@1; `mem[0]` 00010000@0; `mem[1]` 00010001@0,
  00100010@1.
- `depth_only.sv` (step 2): `$dumpvars(1)` called in the child selects one
  level below the top instance: only `tb.t` (00@0, 11@1), not `tb.u.c`
  (S47-D4).
- `excluded.sv` (A03): `$dumpvars(0, tb)` dumps `count`, `ratio` and `total`
  only; the string, handle, dynamic/queue/associative arrays, event,
  automatic function variable and block variable are left out (S47-D9).
  Objects are linked and dropped every cycle under `LLG_GC_STRESS=1`.
  `total` = list sum + dyn size + queue size + entry count: 1+1+1+1 = 4,
  0+2+2+2 = 6 (the list is dropped at count 2), 3+3+3+3 = 12, 7+4+4+4 = 19;
  `ratio` = total/4: 1, 1.5, 3, 4.75.
- `reject_string.sv`, `reject_handle.sv`, `reject_dynamic.sv`,
  `reject_block_variable.sv` (A03 negatives): naming such a variable in
  `$dumpvars` fails compilation with "has no VCD/FST representation".
- `controls.sv` (step 3): `$dumpfile("ignored.vcd")` is replaced by
  `trace.vcd` before the dump starts; the later `$dumpfile("late.vcd")` warns
  (S47-D7). Checkpoints: `$dumpvars`@0 {v=0001, s=0, r=0}; `$dumpoff`@2
  {x, x, NaN} after the step's v=0100 record (S47-D6); no checkpoint for the
  repeated `$dumpoff` or the `$dumpall` at 3 while off (S47-D5); `$dumpon`@4
  {0111, 1, 2.5} then v=1000 at the end of the step; `$dumpall`@5
  {1000, 1, 2.5} then s=0. The `$dumpon` at 0 has no effect (L37961). The
  final procedure's v=9 is recorded at the end time 6.
- `pending.sv` (step 3): controls before the first `$dumpvars` do nothing;
  `$dumpvars` then `$dumpoff` at step 1 start the dump off: `$dumpvars`@1
  {0011} then `$dumpoff`@1 {xxxx}; `$dumpon`@2 {0100}.
- `no_dumpvars.sv` (step 3): no `$dumpvars`, so neither `trace.vcd` nor
  `dump.vcd` is created.
- `dump_extension.sv` (step 3): `$dumpfile("verilog.dump")` writes VCD.
- `open_error.sv` (step 3): the file cannot be created; one
  `llg: waveform: cannot open` report, the run completes and exits 1
  (S47-D11).
- `limit.sv` (step 3): a 16-byte limit below the header size: the header is
  complete, no value change follows, and one `dump limit reached` comment
  ends the file (S47-D8).
- `flush_read.sv` (steps 3-4): `$dumpflush` at step 2 and a `$system` copy
  of the file: the VCD copy holds every change up to time 2. FST keeps its
  hierarchy and block index until close, so only the final FST is read.
- `fst_trace.sv` (step 4): GTKWave's reader sees `tb.r`, `tb.s`, `tb.v`, end
  time 206, dump activity on@0, off@2, on@4; `v` 01@0, 03@1, 04 and 05@4 (the
  `$dumpon` checkpoint value, then the end of the step), then `t`@t for t =
  6..205; `s` 0@0, 1@1, 1@4; `r` 0.25@0, 0.5@4. With `--define LIMIT=400`
  libfst stops at a block boundary: the file still opens, has the same
  hierarchy, an earlier end time and a prefix of the changes.
- `pruning.sv` (step 6): storage no process reads or writes is dumped:
  `never_used` xxxx, `written_only` xxxx then 0001..0011, `read_only` 0101,
  `undriven` zzzz, `from_sink`/`u.constant_out` 1010, `unused_real` 0,
  `table_mem` elements xxxxxxxx; `counter`/`u.unused_in` 0000..0011. The
  library-level sweep generates this and the other VCD fixtures (and the
  waveform suite's) with and without the unused-storage pass and requires
  identical dumps for any model the pass changes.
