# SIM-041: zero-time DPI-C exports and context call chains

These fixtures cover `export "DPI-C"` functions and zero-time tasks, the
context scope of DPI-C call chains (`svGetScope`, `svSetScope`,
`svGetScopeFromName`, `svGetNameFromScope`, user data, `svGetCallerInfo`) and
the disable protocol. Each positive `<name>.sv` has a portable C11 companion
`<name>.c`; the runtime negatives share `errors.c`. The test
[`sim_041.rs`](../../../../sim_feature_completion/sim_041.rs) builds the
companion into a shared library (a DLL linked against the design's import
library, `llg --dpi-import-lib`, on Windows) and passes it with `--dpi-lib`;
import definitions carry `DPI_DLLESPEC` as in
[sim_040](../sim_040/readme.md). `scopes` also runs with the companion built
into a static archive, which needs no import library. Positives run on the legacy,
compact/portable and compact/GMP value backends in both optimizer modes;
`chains` also runs with `-funsigned-char` on the companion and the model.
Companions build with `-std=c11 -Wall -Wextra -Werror` (MSVC `/W4 /WX` for the
archive).

Line numbers refer to `SystemVerilog-1800-2009.txt` (a `pdftotext -layout`
extraction of IEEE 1800-2009).

## Clauses

35.4 (L55609-55611):

> Multiple export declarations are allowed with the same c_identifier, explicit
> or implicit, as long as they are in different scopes and have the equivalent
> type signature (as defined in 35.5.4 for imported tasks and functions).
> Multiple export declarations with the same c_identifier in the same scope are
> forbidden.

35.5.3 (L55749-55752):

> When an import invokes the svSetScope utility prior to calling the export, it
> sets the context explicitly. Otherwise, the context will be the context of
> the instantiated scope where the import declaration is located. Because
> imports with diverse instantiated scopes can export the same subroutine,
> multiple instances of such an export can exist after elaboration.

35.5.3 (L55783-55797):

> This implies that the context property at the root of the DPI import call
> chain or any intermediate import call in the call chain is not transitively
> promoted to subsequent import calls in the DPI import call chain. Since a
> noncontext imported DPI subroutine cannot make a call to a SystemVerilog
> export subroutine, the behavior of making any such calls in the DPI import
> call chain is an error.

35.5.3 (L55801-55807):

> — When a SystemVerilog subroutine calls a DPI context import, a context for
> the import call chain is created that is equal to the instantiated scope of
> the import declaration.
> — When a routine in an import call chain invokes svSetScope with a legal
> argument, the call chain's context is set to the indicated scope.
> — When a call from an import call chain to an exported SystemVerilog
> subroutine finishes and returns to the chain, the call chain's context is set
> equal to its value when the call to the export was made.

35.5.3 (L55822-55827):

> An imported subroutine not specified as context shall not access any data
> objects from SystemVerilog other than its actual arguments. [...] A context
> imported subroutine, however, can access (read or write) any SystemVerilog
> data objects by calling VPI or by calling an export subroutine. Therefore, a
> call to a context subroutine is a barrier for SystemVerilog compiler
> optimizations.

35.7 (L56193-56195, L56218-56221):

> Declaring a SystemVerilog function to be exported does not change its
> semantics or behavior from the SystemVerilog perspective [...]
> No two functions in the same SystemVerilog scope can be exported with the
> same explicit or implicit c_identifier. [...] Only one export declaration is
> permitted per SystemVerilog function, and all export functions are always
> context functions.

35.8 (L56241-56245):

> It is never legal to call an exported task from within an imported function.
> [...] It is legal for an imported task to call an exported task only if the
> imported task is declared with the context property.

35.9 (L56269-56284):

> a) When an exported task returns due to a disable, it shall return a value of
> 1. Otherwise, it shall return 0.
> b) When an imported task returns due to a disable, it shall return a value of
> 1. Otherwise, it shall return 0.
> c) Before an imported function returns due to a disable, it shall call the
> API function svAckDisabledState().
> d) Once an imported subroutine enters the disabled state, it is illegal for
> the current function call to make any further calls to exported subroutines.
> [...] If any protocol item is not correctly followed, a fatal simulation
> error is issued.

35.9 (L56309-56311, L56313-56318):

> If an exported task itself is the target of a disable, its parent imported
> task is not considered to be in the disabled state when the exported task
> returns. In such cases, the exported task shall return value 0, and calls to
> svIsDisabledState() shall return 0 as well.
> When a DPI imported subroutine returns due to a disable, the values of its
> output and inout parameters are undefined. [...] SystemVerilog simulators are
> not obligated to propagate any such values to the calling SystemVerilog code
> if a disable is in effect.

27.4 (L46316-46319):

> For each block instance created by the generate loop, the generate block
> identifier for the loop is indexed by adding the "[genvar value]" to the end
> of the generate block identifier.

H.8.10 (L72504-72508):

> An output mode string is represented by a const char** variable. [...] If it
> is desired to refer to the string's value at some point in the future, the
> user shall copy the string value to memory owned by the C domain.

H.9.2 (L72598-72599):

> The context of an imported or exported subroutine corresponds to the fully
> qualified name of the subroutine, minus the subroutine name itself.

H.9.3 (L72671-72672, L72702, L72711-72714, L72723-72726):

> Unless a prior call to svSetScope has occurred, this is the scope of the
> function's declaration site, not call site.
> The return value shall be NULL for unrecognized scope names.
> It is illegal to pass in NULL values for either the scope or userData
> arguments. It is also an error to call svPutUserData() with an invalid
> svScope. This function returns -1 for all error cases, 0 upon success.
> This function returns NULL for all error cases, and a non-Null user data
> pointer upon success. This function also returns NULL in the event that a
> prior call to svPutUserData() was never made.

## Fixtures

### `scopes` (A01, A02)

`unit_m #(ID)` declares `value`, exports `cell_value(k) = ID*100 + value + k`,
`cell_bump(d)` (`value += d`, prints `%m`) and `cell_nest(k) =
cell_visit(k) + cell_peek()`, and imports the context function `cell_visit`
and the noncontext `cell_peek`. `tb` has `u1` (ID 1), `u2` (ID 2) and
`g[0].c`, `g[1].c` (ID 10, 11).

- `tb_scopes` runs in `tb` (H.9.3: the declaration site). Every instance name
  resolves to a scope whose name is the same string (H.9.2, 27.4 for `g[0]`);
  `tb.nope` gives NULL (H.9.3). User data stored under `key_a` reads back per
  scope (10, 20, 30, 40) and `key_b` was never stored (NULL). Replacing
  `tb.u1`'s `key_a` data with `&data[3]` reads 40. NULL scope, NULL data and
  NULL key each return -1 (H.9.3; the NULL key is an llg check). 4 scopes
  found.
- `tb_drive` selects each instance with `svSetScope`, whose return value is
  the caller scope `tb` (H.9.3), and calls the one exported symbol
  `cell_value(i)` then `cell_bump(10*(i+1))`: u1 100 -> 110, u2 201 -> 220,
  g[0].c 1002 -> 1030, g[1].c 1103 -> 1140 (sum 2406). Each bump changes
  that instance's `value` (35.5.3 L55822-55827) and `%m` names the instance.
- Nested re-entry (35.5.3 L55801-55807): with `tb.u1` set, `cell_nest(3)` runs
  in u1 and calls u1's context import `cell_visit(3)`, whose scope is `tb.u1`.
  It switches to `tb.u2`, reads `cell_value(3) = 200 + 20 + 3 = 223`, adds
  `plain_twice(1) = 2`: 225. `cell_peek`, a noncontext import inside the
  chain, has no scope (`NULL`, not promoted: L55783-55797). `cell_nest`
  returns 225 + 1 = 226; back in `tb_drive` the scope is again `tb.u1`, the
  value when the export was called, and `svSetScope(self)` restores `tb`.
  `tb_drive` returns 2406 + 226 = 2632.
- From SV: values 10 20 30 40, sum 0 -> 100. `u2.cell_visit(4)` = 224 + 2 =
  226; `g[1].c.cell_nest(5)` = 225 + 2 + 1 = 228. Pure and noncontext imports
  outside chains: 5 and 42.

### `chains` (A03)

- `ch_loop(1000)` calls `ch_step(i, &sq, &total)` 1000 times: `total` is the
  sum of squares 0..999 = 332833500, the returns sum to 500500, every `sq`
  matches, `acc` = 1000.
- 200 calls of `ch_text("abc", &o, &io)` with `io = "seed"`: the first gives
  `abc-out`, `seed+`, `abc-ret`, copied at once (H.8.10 L72504-72508).
- `ch_wide`: `b = ~a` for a 100-bit 4-state input whose low byte is 4 ones
  then 4 Z (`aval 0x0f`, `bval 0xf0`): `~` gives 4 zeros and 4 X, so `aval =
  bval = 0xf0`; words 1 and 2 are the complements `edcba987` and `6543210f`;
  bits 99:96 = {0, 1, Z, 1} (MSB first; `aval 0x5`, `bval 0x2`) become {1, 0,
  X, 0}: `aval 0xa`, `bval 0x2`. The 65-bit inout `c = 2^64 - 1` plus 1 is
  `2^64`: bit 64 set, words 0 and 1 zero.
- `ch_byte(-5)`: SV sees `b = -5` whatever the signedness of `char`; `o = -6`,
  result -10.
- `ch_rec(50)`: SV -> C -> SV 50 levels deep, `ch_down(n) = n + ch_rec(n-1)`:
  1275. `ch_down` is automatic, so each level has its own `n` (13.4.2
  L18862-18863: "An automatic function is reentrant, with all the function
  declarations allocated dynamically for each concurrent function call"); with
  a static `ch_down` the deeper calls overwrite the shared `n`, and the result
  would depend on whether `n` is read before the call, which 11.4.2
  L14499-14501 leaves undefined ("The ordering of assignment operations
  relative to any other operation within an expression is undefined").
- `ch_alias`: C passes one address for both outputs of `ch_pair(3)`;
  copy-back runs in declaration order, so `y = 4` is written last (S41-D2).
- `ch_task(7, out)`: the context import task calls the exported task
  `ch_tick(i)` for i = 1..7, each returning 0 (no disable; 35.9 a) with
  `o = 2i`: 56; `acc` = 1000 + 28 = 1028.
- `$time` is 5 ns (5000 in the fixture's 1 ps precision, set by its
  `` `timescale 1ns/1ps `` so the printed time does not depend on a tool's
  default time unit) before and after: the chains take no simulation time.

### `copy_out_targets` (decision S13-D1)

Each context import calls the exported `set_i` before it writes its formals,
and the outputs are written after the import returns, into the targets their
selectors name then (13.5.2 L19185-19187: an inout is copied "once from the
argument into the actual when the subroutine returns"; the text does not say
when the actual's selectors are evaluated).

- `co_output(a[i], 3)` with `i = 0`: `i` becomes 3, so 5 lands in `a[3]`:
  `output 0 0 0 5`.
- `co_inout(a[i], 2)` reads `a[0] = 1` at the call, `i` becomes 2, so 1 + 100
  lands in `a[2]`: `inout 1 2 101 4`.
- `r = co_result(v[i], 6)` with `i = 1`: the bit lands in `v[6]` and the
  result 7 is assigned after the copy-out: `result 7 bit 01000000`.

### `disables` (35.9)

- `dt_outer` (context task) calls exported task `dt_kill(3)`, which adds 3 to
  `seen` and disables `outer_task`, the block running `dt_outer`. The export
  returns 1 and `svIsDisabledState()` is 1 (35.9 a); `dt_outer` returns 1
  (35.9 b). The block ends; its `$display` does not run and `out` keeps 11
  (the output is not propagated, 35.9 L56313-56318, S41-D5).
- `dt_self` disables itself after `seen += 100`: it returns 0 and
  `svIsDisabledState()` is 0 (35.9 L56309-56311). `dt_self_outer` writes
  `o = 42`, which is copied out; `seen` = 103.
- `df_kill`, an exported function, disables `outer_function`; the context
  import function `df_outer` sees `svIsDisabledState() = 1`, calls
  `svAckDisabledState()` (35.9 c) and returns. `r` keeps 1.

### Negatives

Runtime protocol errors, each a fatal error after the output before it
(companion `errors.c`):

| Fixture | Rule |
| --- | --- |
| `neg_noncontext_export` | a noncontext import calls an export (35.5.3 L55796-55797) |
| `neg_scope_without_export` | the chain's scope `tb.uq` does not export `err_inc`; a legal string call in `tb` precedes it |
| `neg_missing_ack` | a disabled imported function returns without `svAckDisabledState` (35.9 c) |
| `neg_call_after_disable` | a disabled import calls another export (35.9 d) |

`task_from_function` (also companion `errors.c`): an imported function calls
the exported task `err_tick`. 35.8 (L56241-56242) makes this illegal but
mandates no check, and the call cannot be checked when the model is compiled,
so the task runs and returns 0 (35.9 a), decision S41-D8): `start`, then
`after r=0 ticks=1`.

`neg_missing_export`: `errors.c` calls `err_tick` and `err_kill`, which the
design does not export; the model link names both.

Code generation rejects `neg_reserved_c_name` (C identifier `svGetScope`).
Exported tasks that consume time are covered by
[adv_018](../adv_018/readme.md).
The frontend rejects `neg_duplicate_c_name` (35.7), `neg_conflicting_export`
(35.4), `neg_import_and_export` (one C identifier imported and exported) and
`neg_undeclared_export`.

## Other implementations

The expected lines follow the clause text. Other simulators were seen to
differ as follows; the fixtures keep llg's lines.

- `chains`, `disables`, `task_from_function`: some simulators return a value
  other than 0 from an exported task that was not disabled (`c unexpected
  disable`, `after r=1`), and no 1 with `svIsDisabledState` 0 from one that
  was (`dt_kill returned <garbage> disabled=0`, `df_kill returned
  disabled=0`). llg follows 35.9 a) (decisions S41-D5 and S41-D8).
- `scopes`: some simulators run `u2.cell_visit(4)` in the caller's scope `tb`
  (decision S41-D1: the context is "the instantiated scope of the import
  declaration", 35.5.3 L55801-55802) and return 0 from `svPutUserData` with
  NULL data (decision S41-D4; H.9.3 L72711-72713: "It is illegal to pass in
  NULL values for either the scope or userData arguments. [...] This function
  returns -1 for all error cases").
