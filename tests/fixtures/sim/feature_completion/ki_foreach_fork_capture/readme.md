# `foreach` loop variables captured by a detached fork

A `join_none`/`join_any` branch that named a `foreach` loop variable read a
copy of the iteration's index, and a string key iterator in that position
was refused ("automatic declaration `s` was not collected before fork
capture"). A `foreach` loop now declares one variable per loop, as a
for-loop with declared variables does, and its detached branches share it:
they read the value the loop leaves (decision
[S12-D3](../../../../../docs/lrm_decisions.md), whose portable case is
`lrm_decisions/S12-D3_foreach_loop_variables_shared_with_forks.sv`).

The task module (`sim_feature_completion/ki_foreach_fork_capture.rs`) runs the
fixture through the public CLI in both HDL optimizer modes and on every value
backend. Expected output was checked against an independent
industry-standard simulator, which gives every line. Clause lines are from
`SystemVerilog-1800-2009.txt`.

- 12.7.3 L18211-18213: "As in a for-loop (12.7.1), a foreach-loop creates an
  implicit begin-end block around the loop statement, containing declarations
  of the loop variables with automatic lifetime. This block creates a new
  hierarchical scope, making the variables local to the loop scope."
- 12.7.3 L18234-18235: "Multiple loop variables correspond to nested loops
  that iterate over the given indices."
- 6.21 L6908-6909: "The lifetime of a fork-join block (see 9.3.2) shall
  encompass the execution of all processes spawned by the block. The lifetime
  of a scope enclosing any fork-join block includes the lifetime of the
  fork-join block."
- 9.3.2 L11480-11484: "Variables declared in the block_item_declaration of a
  fork-join block shall be initialized to their initialization value
  expression whenever execution enters their scope and before any processes
  are spawned. ... These variables are useful in processes spawned by
  looping constructs to store unique, per-iteration data."

`forks.sv`:

- (A) A `foreach` nested in another loop's body, a string key level above a
  queue level, a class key, a dynamic array and an empty associative array:
  every branch reads the final values (`i=3 j=3`, `s=y j=1`, `k.v=2`, `i=2`);
  the empty loop spawns nothing.
- (B) Loops in an automatic task, a static task (string keys) and an
  automatic function: `i=2`, `s=q`.
- (C) A fork block-item copy keeps each index (`k=0`, `1`, `2`) beside the
  shared `i=3`; `wait (i == 3)` and `wait (s == "y")` wake on the loop's own
  steps; a named fork shares the variable too.
- (D) A joined branch and a branch that runs before the next step read each
  index.
- (E) Each activation of an `always` block that runs again has its own loop
  variable, which its branches share.
