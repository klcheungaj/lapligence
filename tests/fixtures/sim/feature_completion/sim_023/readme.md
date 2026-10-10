# SIM-023 legal force/release targets and lifetime semantics

V2001 §9.3.2 and IEEE 1800-2009 §§4.9.2, 6.4, 6.21, 7.2.1, 10.6, 10.6.2,
11.5.1 and 13.3.2 supply the oracles. Every positive source runs through the
public CLI in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends (`run_case_backend_parity`); `.v` sources also run
with `--edition v2001`. `nets_and_variables`, `hier_live` and `lifetime` also
run after snapshot/Db destruction at native O0/O3. All `.out` files are
hand-derived from the clauses and the timelines below. Every observation is
made at least one time unit after the force, release or source change it
depends on, except where a clause makes the value immediate (a released net),
so no result depends on the order of processes within a time step. Line
numbers refer to `SystemVerilog-1800-2009.txt` (SV) and
`Verilog-1364-2001.txt` (V2001), the `pdftotext -layout` extractions.

## Positives

| Fixture | Clauses | Derivation |
| --- | --- | --- |
| `nets_and_variables` (A01) | SV 10.6.2, 6.4, 4.9.2 | Before any force (time 1): `w = a = 1`; `wa` is the wired AND of `a = 1` and `b = 0` (0); `wo` the wired OR (1); `t = a`; `m` resolves `1100` and `1010` per bit to `1xx0`; `cv = src = 40`, `k = 55`, `rv = 1.25 * 2.0 = 2.5`, `pv` follows the child output port (`pi = 1`). At 1 every net and variable is forced. At 2 the forced values show (the `bit` variable `b2` stores `x1z0` as `0100`). At 2 every driver input changes and every variable gets a procedural write; at 3 nothing has changed (a force overrides drivers and procedural writes). At 3 everything is released: the nets take their drivers' values immediately (`w = 0`, `wa = 0 & 1`, `wo = 0 | 1`, `t = 0`, `m = 0000`), the procedurally written variables keep the forced values, and the variables with a continuous driver (`cv`, the constant `k`, the real `rv`, the port-driven `pv`) are re-established: at 4 they show `41`, `55`, `2.0 * 2.0 = 4` and `2`. Procedural writes at 4 take effect again; `src = 42` drives `cv` again (5). |
| `partial_overlap` (A01, both editions) | V2001 9.3.2; SV 10.6.2, 11.5.1 | `n = d = 00`. Forcing `n[7:4] = f` and `n[1] = 1` gives `11110010`. Forcing `n[5:2] = 0000` replaces bits 5:4 of the first force and adds 3:2: `11000010`. Releasing `n[3:2]` gives those bits back to `d` (0): unchanged. `d = ff`: forced bits 7:6 (11), 5:4 (00) and 1 (1), the rest from `d`: `11001111`. `release n` gives `11111111`. The concatenation `{n[0], n2[7:6]} = 101` forces `n[0] = 1`, `n2[7:6] = 01`; `n2 = ~d = 00` elsewhere. `d = 0f`: `n = 00001111`, `n2 = f0` with bits 7:6 forced to `01`: `01110000`. Releasing the concatenation gives `n2 = 11110000`. The constant indexed part-selects (11.5.1: "an indexed part-select is a constant part-select if its base is a constant value") `n[3 +: 2] = 10` and `n[7 -: 2] = 11` over `d = 0f` give `11010111`; releasing `n[3 +: 2]` gives bits 4:3 back to `d`: `11001111`. `r` has two continuous drivers, one per nibble of `e`; `r[5:2] = 1010` spans both: `e = 0f` gives `00101011`, `e = f0` gives `11101000`; releasing `r[4]` gives that bit back to `e` (1): `11111000`; `release r` gives `11110000`. |
| `net_member_selects` (A01) | SV 10.6.2, 7.2.1, 11.5.1 | `d = 12`, `od = 3ff`. `sn.hi` forced to `f`: `f2`. `pa[0] = 9` and `pa[1][0] = 0` (with `pa[1] = 1`): `09`. In `on` (`inner` = bits 9:2, `tag` = 1:0) `inner.lo` (bits 5:2) is forced to 0 and `tag` to `01`: `1111000001`. `n8` is `[8:1]`: `n8[4:3] = 11` and `n8[8 -: 2]` (= `n8[8:7]`) `= 10` over `d`: `10011110`. `nr` is `[0:7]` (MSB first): `nr[2 +: 2]` is `nr[2:3]` `= 11`: `00110010`. With `d = 34`, `od = 0`: `sn = f4`, `pa[1] = 3` with bit 0 forced: `29`, `on = 0000000001`, `n8 = 10111100`, `nr = 00110100`. Releasing everything but `n8[4:3]`: `34`, `34`, `0000000000`, `n8 = 00111100`, `nr = 00110100`; `release n8` gives `00110100`. |
| `variable_concat` (A01, both editions) | V2001 9.3.2; SV 10.6.2 | `force {a, b} = {s, ~s}` with `s = 1`: `a = 1`, `b = e`; `c = 3` is untouched. `s = 3`: `a = 3`, `b = c` (live RHS). `force {b, c} = 5c` replaces the force on `b` only; `a` stays forced to `s`. `s = 7`: `a = 7`. `release a`: `a` keeps 7; nothing depends on `s` any more. `a = 9` takes effect, `b = 9` does not (forced). `release {b, c}`, then `b = 0`, `c = 0` take effect. Releasing `a` again (not forced) changes nothing. |
| `hier_live` (A02) | SV 10.6.2, 10.6, 23.6 | `d = 1`, `s = 2`, `itf.iv = 3`. Down (`m.l.v = s`, `m.l.o = d + s` over the child's own continuous driver), generate (`gen[1].gw = s`; `gen[0].gv = m.l.o`, a forced net), instance array (`la[0].o = e`; `la[1].o` is not forced), absolute (`$root.tb.m.mv = gen[1].gw`), interface net (`itf.iw = s + 1`), alias (`wa[1:0] = s[1:0]`, read through `wb = d`), upward from a child task (`tb.top_v = val + tb.s`, `val = 4`) and from a class method (`tb.cls_v = tb.s ^ f`). Time 1: `2 3 2 3 e 1 2 3 2 6 d 1` (`wb = 0001` with bits 1:0 `10`; `gen[0].gw = d + 0`). `s = 5` re-evaluates every RHS that reads it (10.6: "if any variable on the right-hand side of the assignment changes, the assignment shall be reevaluated"): `5 6 5 6 e 1 5 6 1 9 a 1`. `d = 7` changes the drivers underneath (`m.l.o` becomes `7 + 5 = c` because its RHS reads `d`; `la[1].o` and `gen[0].gw` follow `d`; `wb = 0111` with bits 1:0 forced `01`), `top_v = 0` is overridden and `itf.iv = 8` is hidden by the forced `itf.iw`: `5 c 5 c e 7 5 6 5 9 a 7`. Releasing (readers of other forced signals first): variables keep their values (`m.l.v = 5`, `gen[0].gv = c`, `m.mv = 5`, `top_v = 9`, `cls_v = a`), nets take their drivers (`m.l.o = 7`, `gen[1].gw = 8`, `la[0].o = 7`, `itf.iw = 8`, `wb = 7`): `5 7 8 c 7 7 5 8 7 9 a 7`. `s = 9` changes nothing. |
| `hier_live_v2001` (A02, both editions) | V2001 9.3.2, 12.4 | The 1364-2001 subset of `hier_live` (down, generate, instance array, upward task): `2 3 2 3 e 1 2 6 1`, then `5 6 5 6 e 1 5 9 1`, `5 c 5 c e 7 5 9 7`, and after release `5 7 8 c 7 7 5 9 7` twice. `o0`/`o1` are `la[0].o`/`la[1].o` through the instance-array port connection. |
| `lifetime` (A03) | SV 10.6.2, 4.9.2, 9.6, 13.3.2 | A force is not owned by the process that executes it. A named fork forces `v = d` and `n = d + 1` (1: `v = 1`, `n = 2`) and is disabled at 1 before its `release v`; both forces stay live: `d = 3` gives `v = 3`, `n = 4`. A detached branch forces `w2 = bump(d)` (a function with a side effect, so a guard process re-evaluates it) at 2 (`4`); `d = 4` at 3 gives `5`; the branch is killed by `disable fork` at 4 and `d = 5` still gives `6`. `release w2` keeps 6; `d = 6`: `v = 6`, `n = 7`. An automatic task forces the static `h = d` at 6 and is killed at 7; `d = 7` gives `h = 7`, `v = 7`, `n = 8`. `$finish` ends the run with `v`, `n` and `h` forced. The number of `bump` calls is not observed (it is unspecified). |

Adopted FND-002 witnesses (source unchanged): `selected_net_force_witness.v`
(L-F05-13-01; `01` then `00`), `hierarchical_force_witness.v`
(L-F05-13-02/03; a child net forced to the parent's `a = 1`, then released to
its driver 0), and the negatives `neg_force_automatic_witness`,
`neg_force_variable_select_witness` and `neg_force_dynamic_net_select_witness`.

## Negatives

Language rules, rejected with a located error in both optimizer modes:

- `neg_automatic_target`, `neg_force_automatic_witness`: an automatic task
  variable as a force target (SV 6.21, 13.3.2), reported by the frontend.
- `neg_automatic_rhs`: an automatic block variable in a force RHS (SV 13.3.2).
- `neg_ref_formal`: a `ref` formal of an automatic task as a target (SV 13.3.2).
- `neg_class_property`: a class property as a target (SV 6.21).
- `neg_dynamic_element`: a queue element as a target (SV 6.21).
- `neg_variable_select` (both editions), `neg_force_variable_select_witness`
  and `neg_struct_member_variable`: a bit-select, or a member of a packed
  structure variable (a part-select of a variable), as a target.
- `neg_force_dynamic_net_select_witness`: a non-constant net bit-select.
- `neg_array_element` (`--edition v2001`): a memory word (V2001 9.3.2).
- `neg_unpacked_array`, `neg_unpacked_struct`: unpacked arrays and
  structures are not singular (SV 6.4, 10.6.2); llg reports the target and its
  location. Portable decision `S23-D2` in `docs/lrm_decisions.md`.

Legal but not supported by llg (located errors; see `docs/known_issues.md`):

- `neg_string`, `neg_class_handle`: string and class-handle variables are
  singular (SV 6.4) but cannot be forced; events and chandles are rejected the
  same way (a chandle already by the frontend).
- `neg_whole_net_array`: a whole unpacked net array (its elements can be
  forced).
- `static_memory_force_witness` (FND-002 L-F05-13-02): an element of a static
  unpacked array read as a singular variable under SV 10.6.2. The frontend
  rejects every select of a variable, as V2001 9.3.2 requires ("It cannot be
  a memory word"); this test pins the current rejection.

## Clause text

V2001 §9.3.2 (L8917-8929):

> The left-hand side of the assignment can be a variable, a net, a constant bit-select of a
> vector net, a part-select of a vector net, or a concatenation. It cannot be a memory word (array reference) or
> a bit-select or a part-select of a vector variable.
>
> A force statement to a variable shall override a procedural assignment or procedural continuous assignment
> that takes place on the variable until a release procedural statement is executed on the variable. After the
> release procedural statement is executed, the variable shall not immediately change value (as would a net
> that is assigned with a procedural continuous assignment). The value specified in the force statement shall be
> maintained in the variable until the next procedural assignment takes place, except in the case where a proce-
> dural continuous assignment is active on the variable.
>
> A force procedural statement on a net overrides all drivers of the net—gate outputs, module outputs, and
> continuous assignments—until a release procedural statement is executed on the net.

SV §10.6.2 (L13371-13376, L13388-13399):

> Another form of procedural continuous assignment is provided by the force and release procedural state-
> ments. These statements have a similar effect to the assign-deassign pair, but a force can be applied to
> nets as well as to variables. The left-hand side of the assignment can be a reference to a singular variable, a
> net, a constant bit-select of a vector net, a constant part-select of a vector net, or a concatenation of these. It
> shall not be a bit-select or a part-select of a variable.

> A force statement to a variable shall override a procedural assignment, continuous assignment or an
> assign procedural continuous assignment to the variable until a release procedural statement is executed
> on the variable. When released, then if the variable is not driven by a continuous assignment and does not
> currently have an active assign procedural continuous assignment, the variable shall not immediately
> change value and shall maintain its current value until the next procedural assignment to the variable is
> executed. Releasing a variable that is driven by a continuous assignment or currently has an active assign
> procedural continuous assignment shall reestablish that assignment and schedule a reevaluation in the
> continuous assignment's scheduling region.
>
> A force procedural statement on a net shall override all drivers of the net—gate outputs, module outputs,
> and continuous assignments—until a release procedural statement is executed on the net. When released,
> the net shall immediately be assigned the value determined by the drivers of the net.

SV §10.6 (L13309-13311):

> The right-hand side of an assign procedural continuous assignment or a force statement can be an
> expression. This shall be treated just as a continuous assignment; that is, if any variable on the right-hand
> side of the assignment changes, the assignment shall be reevaluated while the assign or force is in effect.

SV §4.9.2 (L3515-3519):

> A procedural continuous assignment (which is the assign or force statement; see 10.6) corresponds to a
> process that is sensitive to the source elements in the expression. When the value of the expression changes,
> it causes an active update event to be added to the event region, using current values to determine the target.
>
> A deassign or a release statement deactivates any corresponding assign or force statement(s).

SV §6.4 (L4606-4607):

> A singular type shall be any data type except an
> unpacked structure, unpacked union, or unpacked array (see 7.4 on arrays).

SV §6.21 (L7007-7010):

> Automatic variables and members or elements of dynamic variables—class properties and dynamically
> sized variables—shall not be written with nonblocking, continuous, or procedural continuous assignments.
> References to automatic variables and elements or members of dynamic variables shall be limited to proce-
> dural blocks.

SV §13.3.2 (L18646-18650; the dash items are L18648-18650):

> Because variables declared in automatic tasks are deallocated at the end of the task invocation, they shall not
> be used in certain constructs that might refer to them after that point:
> — They shall not be assigned values using nonblocking assignments or procedural continuous
> assignments.
> — They shall not be referenced by procedural continuous assignments or procedural force statements.

SV §11.5.1 (L15779-15782):

> A constant bit-select is a bit-select whose position is constant. A constant part-select is a part-select whose
> position and width are both constant. The width of a part-select is always constant. Thus, a non-indexed
> part-select is always a constant part-select, and an indexed part-select is a constant part-select if its base is a
> constant value as well as its width.

SV §7.2.1 (L7696-7700):

> A packed structure is a mechanism for subdividing a vector into subfields, which can be conveniently
> accessed as members. [...] when a packed structure appears as a primary, it shall be treated as a single
> vector.
