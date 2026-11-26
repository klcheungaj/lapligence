# Automatic block strings, handles, events and records captured by a fork that runs again

An automatic string, class handle, virtual interface or event declared in a
procedural block and read by a `join_none`/`join_any` branch while the block
runs again was refused ("can be live in two activations through a
`join_any`/`join_none` fork that runs again"): it was one model object per
declaration. Such a variable is now a local of each activation of its
block, and the fork branches of that activation share it like an integral
automatic. An automatic block record with string, real, handle or container
members in that position (`records.sv`) is a native value of each
activation, as a subroutine record is: declared at its statement and shared
through the fork frame. A branch that waits on such a record while another
process stores into it is rejected, as for a subroutine record (a record
publishes no change), and so is a fork branch that names a record of
lexical columns (a member array of 16 or more cells), which has no fork frame
form (`sim_007/bad_block_record_fork.sv`,
`sim_007/bad_block_column_record_fork.sv`).

The task module (`sim_feature_completion/ki_block_activation_locals.rs`)
runs the fixture through the public CLI in both HDL optimizer modes and on
every value backend. Expected output was checked against an independent
industry-standard simulator, which gives every line. Clause lines are from
`SystemVerilog-1800-2009.txt`.

- 6.21 L6896-6898: "Specific variables within a static task, function, or
  block can be explicitly declared as automatic. Such variables have the
  lifetime of the call or block and are initialized on each entry to the call
  or block"
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

- (A) A `foreach` body's string, handle and virtual interface: the parent's
  write after the fork (`late`) is what its branches read, and a branch's
  writes reach the object and interface of its own iteration
  (`late! 11 15`, `21 16`, `31 25`).
- (B) An `always @(posedge clk)` block's string, handle and event: each edge's
  waiter wakes on that edge's event after its sibling appended `+` and added
  100 (`c0+ 100`, `c1+ 101`, `c2+ 102`).
- (C) A string declared in a `join_none` branch that runs again is that
  branch's own, shared with its nested fork: `changed` twice.
- (D) `@(h)` on a shared handle wakes when the parent replaces it; a `ref`
  actual writes the shared string (`AB0`); `wait (z == "go")` wakes on the
  parent's write. The loop variable `i` is shared too: `go 1`, `go 2`.
- (E) A fork's own declaration in a loop: `w0`, `w1`.

`records.sv`:

- (A) A loop body record with string, real and queue members: each
  iteration's branch reads the parent's writes made after the fork
  (`alate`, `'{i, 7}`) and its own writes (`A' 10`).
- (B) A record passed to a task's `ref` formal by a detached branch, and read
  by a joined fork nested in it: the task's write is the parent's (`go!`).
- (C) A record declared in a `join_none` branch that runs again is that
  branch's own, shared with its nested fork: `changed2` (the for-loop
  variable is shared too).
- (D) Whole-record copies out of and into the shared record.
- (E) An `always @(posedge clk)` block's record: each edge's branch reads
  that edge's value (`E e 19`, `E e 21`).
