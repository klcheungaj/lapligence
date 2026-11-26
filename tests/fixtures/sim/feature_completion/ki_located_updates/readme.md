# Updates of the operands of a nonblocking assignment

A nonblocking assignment to a part of a record, one with an event control and
one that ends with its issuer tell the runtime what the next update is for. The
marks used to be set before the operands ran, so an update that a function in
an operand queued was taken for the assignment's own: `ra[2].s <= g(5)` crashed
when `g` made a nonblocking assignment, and `ra[1].a <= f(4)` lost the update
`f` made. The marks now enclose only the call that queues the update.

The task module (`sim_feature_completion/ki_located_updates.rs`) runs every
fixture through the public CLI in both HDL optimizer modes. An independent
industry-standard simulator refuses nonblocking assignments to strings and to
members of a structure that holds one, so the expected output follows from the
text, and each statement prints the same with its operands assigned to
variables first. For packed targets that simulator prints the same too. Clause
lines are from `SystemVerilog-1800-2009.txt`.

Common clauses:

- 10.4.2 L13070-13072: "If variable_lvalue requires an evaluation, it shall be
  evaluated at the same time as the expression on the right-hand side. The
  order of evaluation of the variable_lvalue and the expression on the
  right-hand side is undefined".
- 9.4.5 L12123-12124: "An intra-assignment delay or event control shall delay
  the assignment of the new value to the left-hand side, but the right-hand
  expression shall be evaluated before the delay".

## Operands

`operands.sv`: functions that make nonblocking assignments, of variables and of
parts of records, as the source, an index and a delay of assignments to members,
selects, nested records, member arrays and container members of record
variables, elements, package records and static properties. The last section
replaces, in an operand, the record the target selects from.

## Event controls

`holds.sv`: the same operands under `@(ev)` and `repeat (n) @(ev)`: the
operand's updates are made in the time slot the statement runs in, the
assignment's after the event.

## Other operands

`ends.sv`: an operand that kills its process or disables its block, so that
nothing is assigned and the next assignments are made as written; one that
forks; and an imported function (`ends.c`) that calls an exported function
with nonblocking assignments.
