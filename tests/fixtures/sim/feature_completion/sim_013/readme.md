# SIM-013 evaluated-event and implicit dependency environments

IEEE 1800-2009 §§4.4.2.9, 6.14, 9.2.2.2, 9.4.2, 9.4.2.2, 9.4.2.3, 9.4.3,
13.4, 13.5, 25.9 and 35.5 supply the oracles. Every positive source runs
through the public CLI in both optimizer modes and on the legacy and compact
value backends; `nested_values`, `class_handles`, `helper_activations` and
`composition` also run after snapshot/Db destruction at native O0/O3. All
`.out` files are hand-derived from the clauses and the timelines below; each
waiter appends `$time` to its own log, so same-time wakes of different
processes cannot reorder the output.

## Positives

| Fixture | Clauses | Derivation |
| --- | --- | --- |
| `nested_values` | §§9.4.2, 9.4.3, 7.2, 7.4, 7.5, 7.8, 7.10 | An event is a change of the expression's value (§9.4.2). `@(s.a)`: `s.b` at 1 and an unchanged `s.a` at 3 are no events, `s.a` at 2 is. `@(o.in.b)`: siblings `o.in.a` (4) and `o.k` (5) and a whole-record store that keeps `o.in.b` (7) are no events; 6 and 8 are. `@(arr[i])` with `i = 0`: `arr[1]` at 9 is unselected; `i = 1` at 10 changes the value (10 to 21); the old element at 11 is unselected; 12 changes `arr[1]`; at 13 `arr[2]` and `i` change together but the value stays 22; 14 is the old element; 15 changes `arr[2]`. `@(arr[3])` wakes only at 16. `@(d[k])`: 17 is unselected, 18 changes `d[1]`, `k = 2` at 19 selects 0 instead of 5, `new[5](d)` at 20 keeps `d[2]`, 25 changes it. `@(d.size())` wakes at 20 (3 to 5). `@(q[0])`: `push_back` at 21 keeps it, `push_front(0)` at 22 changes it. `@(aa["b"])`: 23 writes another key, 24 changes it. The level wait becomes true at 26 when `o.in.a` is 2 and `arr[i]` (`i = 2`) is 5. |
| `class_handles` | §§9.4.2 (the `@(p.status)` example), 8.4 | `h` names object A (`v = w = 0`), `h2` object B (`v = 7`). `@(h.v)`: 2 (A.v), 3 (`h = h2`, 2 to 7), 5 (B.v 8); 4 stores an equal value; 6 and 7 write A, no longer named; 8 assigns the same handle. `@(h)` wakes only at 3. `@(h.sum())` (nonvirtual method, `v + w`): 1, 2, 3, 5 and 9 (B.w = 3 gives 11). `@(hs[i].v)`: 10 writes the unselected element's object; 11; `i = 1` at 12 (1 to 5); 13 unselected; 14; at 21 `hs[1] = hs[0]` names an object whose `v` is 2 (was 6). `@(n.next.v)`: 15; rebinding the handle property `n.next` at 16 (1 to 0); 17 stores an equal value; 18. `@(posedge h.w[0])`: 1 (0 to 1); `h = h2` at 3 is a falling LSB; 9 (B.w LSB 0 to 1). The wait on `p != null && p.v == 3` arms while `p` is null without accessing it (short-circuit, §11.4.7) and is true at 20. |
| `interfaces` | §§25.9, 9.4.2 | `v` names `i0`. `@(v.sig)`: 2 (`i0.sig`), 3 (`v = i1`, 4 to 2), 7 (`i1.sig`), 9; 4 and 8 write `i0`, no longer named. `@(posedge v.clk)`: 1; `v = i1` at 3 is a falling clock; 5 writes `i0`; 6. The wait for `v.sig == 9` ignores `i0` at 8 and completes at 9. |
| `descriptor_selectors` | §§9.4.2, 9.4.3 | A 65,537-cell array (descriptor storage). `@(big[i])`: 1 is unselected, 2, `i = 65536` at 3 (2 to 1), 4 writes the old cell, 5. The wait for `big[i] == 3` ignores 4 and completes at 5. |
| `comb_sensitivity` | §§9.2.2.2.1, 9.2.2.2.2, 9.4.2.2 | always_comb runs at 0 and is sensitive to `g` inside `rd`; `@*` only to the argument `a` (§9.2.2.2.2), so at 2 `comb = 5` and `star = 0`, and after `a = 1` both are 6. The second always_comb writes `t`, `scratch` (inside `dbl`) and `n_comb`, which are excluded (§9.2.2.2.1 b); it runs at 0 and when `b` becomes 3 (5), not when `b` is rewritten with 3 (6): `z = (3 + 1) * 2 + 6 = 14`, `n_comb = 2`. §9.2.2.2.1 L11192-11194: "References to class objects and method calls of class objects do not add anything to the sensitivity list of an always_comb." Read literally (user decision N2 of LRM audit B), the reference `h.x` adds nothing, not even the handle variable `h`: the third always_comb runs only at 0 (`h.x` of the first object, 0), and neither `h.x = 2` (7), rebinding `h` (8) nor `h2.x = 9` (9) wakes it: `yc = 0`, `n_cls = 1`. (`@*` still adds the identifier `h`, §9.4.2.2; see `lrm_decisions/AB-N2_always_comb_class_reference.sv`.) |
| `edges` | §9.4.2 Table 9-2, §9.4.2.3 | `s`: 0→x, x→1 posedges (1, 2); 1→z, z→0 negedges (3, 4); 0→z posedge (5); z→x none (6); x→1 posedge (7); 1→x, x→0 negedges (8, 9); `edge` is either. `posedge vec` uses the LSB: 0→0 none (10), 0→x and x→1 (11, 12), 1→1 none (13). `posedge (a & b)`: 0→x (14), x→1 (15), 1→0 none (16). `iff` is evaluated when the event occurs: `c` changes at 17 with `en = 0`; `en` alone (18) is no event; 19 and 20 with `en = 1` (20 is also a posedge); at 21 `en` falls before `c` changes and at 22 `c` changes before `en` rises, so neither qualifies; 23. |
| `helper_activations` | §§9.4.2.3, 9.6.2, 13.3.2, 13.5.2 | `watch(5, "A")` waits for `sig > 5`; `sig = 3` at 1 does not qualify; `disable first` at 2 ends it, so `sig = 6` at 4 prints nothing for A. `watch(1, "B")` qualifies at 3 with its own `local_lim`. `@(pick(arr, i))` (const ref helper): 5 writes an unselected element, 6 (`arr[0] = 7`), 7 (`i = 1`, 7 to 9), 8 unselected. `local_event` (9) waits on its own automatic `a`: 2 at 10 fails the qualifier, 1 at 11 prints. `drive_local` (12) calls `rise(l, 1)`, which waits on `posedge s[0]` of its `ref` formal and recurses: 13 (0 to 1), 14 falls, 15 (0 to 1). `above(arr[2], 6)` (15) follows the element through its `ref` formal: 5 at 16 fails, 17 writes another element, 7 at 18 prints. |
| `foreign_helpers` | §§9.4.2, 35.5.2, 35.5.3, 21.2.2 | `dpi_twice` is pure: `a = 1` at 1 changes it (2), an equal store at 2 is no event, 3 (6). `dpi_count(a) > 2` is true at 3. `$strobe` at 4 calls the non-context import in Postponed and prints `count=3`. The import's call count is library state, not observed. |
| `context_strobe` | §§4.4.2.9, 35.5.3 | The design declares no DPI export, so the context import `dpi_twice` has no SystemVerilog subroutine to call and can write nothing; `$strobe` at 1 evaluates it in Postponed after `a = 5`: `strobe=10`. Quotes below. |
| `composition` | §§13.5.1, 25.9, 8.4, 27.4 | `watch(h, 3)` copies the handle (object A) at the call; `h = other` (1) and writes to `other` (2, 4) do not move its waits; `keep.v = 1` writes A at 3, and `keep.v = 3` completes its wait at 9. `port.vb` names `lane[0].b`: 5 writes `lane[1]`; 6 (0 to 4); assigning `port.vb = lane[1].b` at 7 changes the observed value (4 to 7); 8 writes `lane[0]`. |

Adopted FND-002 witnesses (source unchanged): `class_sensitivity_witness`
(L-F06-02-01, L-F02-13-05; `@(a)` on a class handle wakes when `new` is
assigned), `automatic_event_capture_witness` (L-F06-03-02; an automatic
task's fork branch waits on the task's automatic `a iff a == 1`),
`real_anychange_witness` and `real_boolean_edge_witness` (L-F06-03-01; real
any-change and the edge of a Boolean derived from a real are legal).

## Negatives

Language rules, rejected in both optimizer modes:

- `neg_chandle_event`: chandles shall not be used in event expressions
  (§6.14), unlike class handles. Also the FND-002 `neg_chandle_event` witness.
- `neg_real_edge`: an edge of a real operand (§6.12.1, §9.4.2), reported by the
  frontend.
- `neg_output_helper`: a function with output, inout or ref formals in an event
  expression (§13.4), reported by the frontend.

Implementation restriction (rejected in both optimizer modes):

- `neg_context_strobe`: the design exports `set_a`, so the context import
  called by `$strobe` may write `a` through it, and llg cannot see whether the
  foreign code does. Context imports in `$monitor`/`$strobe` are rejected
  only when the design declares a DPI export (`context_strobe` is the legal
  counterpart).

§4.4.2.9 (`SystemVerilog-1800-2009.txt` L3212-3214):

> No new value changes are allowed to happen in the current time slot once the Postponed region is reached.
> Within this region, it is illegal to write values to any net or variable or to schedule an event in any previous
> region within the current time slot.

§35.5.3 (L55794-55795, L55821-55826):

> import call chain. Since a noncontext imported DPI subroutine cannot make a call to a SystemVerilog export
> subroutine, the behavior of making any such calls in the DPI import call chain is an error.

> optimizations. An imported subroutine not specified as context shall not access any data objects from
> SystemVerilog other than its actual arguments. Only the actual arguments can be affected (read or written)
> by its call. Therefore, a call of a noncontext subroutine is not a barrier for optimizations. A context imported
> subroutine, however, can access (read or write) any SystemVerilog data objects by calling VPI or by calling
> an export subroutine.

The text forbids writes in Postponed, not calling a context import. llg does
not inspect foreign code: without an export, the remaining path is VPI, which
this check does not cover.

## Policies

- `always_comb` and `@*` add no dependency for class-property or method reads
  (§9.2.2.2.1: references to class objects add nothing) or for
  virtual-interface members (§25.9: they cannot be used in sensitivity lists);
  the handle variables they read are ordinary variables.
- The number of helper evaluations is unspecified; event controls that read a
  class property, an interface member, a method, a foreign function or a `ref`
  formal through an expression are evaluated by the waiting process once when
  armed and once per wake, so no fixture depends on the count.
