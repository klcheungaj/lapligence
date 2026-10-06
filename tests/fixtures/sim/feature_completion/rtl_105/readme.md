# RTL-105 frontend admission: uwire inout ports and runtime-selected variable continuous assignments

IEEE 1800-2009 §§6.5, 6.6.2, 7.4.6, 10.3, 10.3.2, 11.5.1, 11.5.3,
23.3.3.6–23.3.3.7 and Annex A.3.3/A.6.1/A.8.5 supply the oracles. Every `.out`
file is hand-derived from those clauses. Positive sources run through the
public CLI in both optimizer modes, on the legacy, compact/portable and compact/GMP value backends, and after snapshot/Db
destruction.

- `uwire_inout_formal`: ANSI and non-ANSI `inout uwire` formals. §6.6.2 forbids
  only multiple drivers and pass-switch terminals, and §§23.3.3.6–23.3.3.7
  collapse the formal with the actual, so the collapsed net keeps one driver:
  inside the child, in the parent with a reader-only formal, uwire on both
  sides, and a two-level chain with one bit driven at each level.
- `uwire_concat_inout`: uwire nets concatenated on an inout port (RTL-099 probe
  p06) are collapsed bit by bit, not driven by the connection. The child may
  drive one bit and the parent another; a plain wire beside a uwire in the same
  concatenation keeps its own multiple drivers; a selected uwire bit, a uwire
  and a wire concatenate onto an `inout uwire` formal.
- `assign_runtime_select`: `assign a[i] = x` and its relatives on variables
  (element, packed bit, indexed part, record member of an element, and a
  concatenation target). A selector change re-evaluates the assignment and
  writes the newly selected element while the previous one keeps its value;
  an unknown or out-of-range selector writes nothing (§§7.4.6, 11.5.1).
- `assign_runtime_select_composition`: a 65,537-cell descriptor-backed array,
  a delayed (inertial) driver, two runtime indices, an expression selector of
  a wide vector, a selector driven by another continuous assignment, a child
  module whose output array links to the parent, generated drivers whose
  longest static prefixes are disjoint rows, and a procedural writer of a row
  outside the prefix.
- `assign_runtime_select_retarget`: a delayed driver whose selector changes
  while an update is pending. §10.3.3 makes the delay inertial but no clause
  covers a selector-only change; lapligence re-evaluates the whole assignment,
  so the newest evaluation's target and value replace the pending update and
  the previously selected element keeps its value (`a[0]` stays `0`, `a[1]`
  becomes `1` five units after the selector change; an interleaved value then
  selector change on `v` writes only the last target).
- Negatives: two drivers of a collapsed uwire formal (child and parent) or of a
  concatenated uwire bit, two drivers inside the module of an `inout uwire`
  formal, a uwire formal on a pass switch, a runtime select on a net and on a
  gate output terminal (both `net_lvalue`, constant select), and procedural or
  second continuous writers inside a runtime select's longest static prefix
  (whole array and constant row).

## Legality of `assign a[i] = x` on a variable

Legal. `continuous_assign` has a second form,
`assign [delay_control] list_of_variable_assignments` (§10.3, A.6.1), whose
`variable_assignment ::= variable_lvalue = expression`. A.8.5 gives
`variable_lvalue` a `select` built from `bit_select ::= { [ expression ] }`
and `part_select_range`, which admit non-constant expressions; footnote 42
restricts only a `variable_lvalue` assigned in a `sequence_match_item`. In
contrast `net_lvalue` takes `constant_select`, and gate terminals are
`net_lvalue`s (A.3.3). §6.5 states the writer rule in terms of the longest
static prefix (§11.5.3): the prefix of `a[i]` is `a` and that of `m[1][i]` is
`m[1]`, so no other continuous or procedural writer may touch any element of
it. §10.3.2 re-evaluates on right-hand operand changes; the clause does not
mention selectors, so lapligence also re-evaluates when a left-hand selector
changes, matching the runtime-selected variable output port (an implied
continuous assignment, §23.3.3.2) and the read set of `always_comb`
(§9.2.2.2.1).
