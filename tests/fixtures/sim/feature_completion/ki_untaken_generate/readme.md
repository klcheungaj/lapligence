# Nothing of an untaken generate block exists

Decision S22-D10 (portable case
`tests/fixtures/sim/lrm_decisions/S22-D10_untaken_generate_class.sv`): a
generate block that elaboration does not instantiate brings nothing into the
model. These two cases cover what the portable case cannot.

`subroutine_statics.sv` declares, in an untaken `if (0)` branch (`x`), a
loop generate without iterations (`z`) and an unselected `case` item
(`c1`), a variable with a call initializer, a function and (in `x`) a task
with a static local whose initializer calls `mark()`, and initial, always
and final procedures. The selected `else` branch (`y`) and `default` item
(`cd`) declare the same. `mark()` counts its calls per name; at 1 ns the top
prints the counts in name order (associative array order of string keys),
so the order among initializers does not matter. Only `y` and `cd` exist,
each initializer runs once, and only `y`'s procedures run:

- `tb.cd.b 1`, `tb.cd.f.s 1`, `tb.y.b 1`, `tb.y.f.s 1`, `tb.y.initial 1`,
  `tb.y.t.s 1`: the variable and static-local initializers of `y` and `cd`
  and `y`'s initial procedure; no name of `x`, `z` or `c1`.
- `y.b + y.f() + cd.b + cd.f() = 4`: each is the 1 `mark()` returns.
- `tb.y.t 1`: the call of `y.t` prints its static local.
- `tb.y final`: `y`'s final procedure after `$finish` (`x` has one too).

Exit status 0. An independent industry-standard simulator rejects static
local initializers in tasks and functions of generate blocks as an
unsupported construct, so this case is llg-only; the oracle follows from
the clauses below.

`class_features.sv` declares in an untaken `if (0)` branch classes that use
randomization with an inline constraint, an event, a static property, a
queue, a mailbox of the class type, a `process` handle and a `fork`, a
static method with a static local, a class derived from a live class, a
parameterized class and its specialization, an abstract class and its
implementation, an associative array keyed by the class and a queue of
handles, and an initial procedure that uses them all. It must compile and
print only `done` at 2 ns, exit status 0: nothing of the branch runs. The
same industry-standard simulator prints the same line. The test
`untaken_generate_classes_are_not_class_roots` checks on the owned database
that these classes are marked uninstantiated and are no class roots.

Clauses (`SystemVerilog-1800-2009.txt`):

- 27.3 L45986-45988: "The elaboration of a generate construct results in
  zero or more instances of a generate block. An instance of a generate
  block is similar in some ways to an instance of a module. It creates a new
  level of hierarchy. It brings the objects, behavioral constructs, and
  module instances within the block into existence."
- 27.5 L46364-46366: "The conditional generate constructs, if-generate and
  case-generate, select at most one generate block from a set of alternative
  generate blocks based on constant expressions evaluated during
  elaboration. The selected generate block, if any, is instantiated into the
  model."
- 6.21 L6941-6943: "an explicit static keyword shall be required when an
  initialization value is specified as part of a static variable's
  declaration to indicate the user's intent of executing that
  initialization only once at the beginning of simulation."
