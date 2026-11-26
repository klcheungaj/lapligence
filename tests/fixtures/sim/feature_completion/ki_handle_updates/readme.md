# Nonblocking assignments to handles

Nonblocking assignments to mailbox, semaphore and process handles were rejected,
like the event-controlled ones to elements and whole arrays of handles and
strings, and updates of an element of a static class property did not end with
their issuer.

The task module (`sim_feature_completion/ki_handle_updates.rs`) runs every
fixture through the public CLI in both HDL optimizer modes. Expected output was
checked against an independent industry-standard simulator, which gives every
line except those named below. Clause lines are from
`SystemVerilog-1800-2009.txt`.

Common clauses:

- 10.4.2 L13068: "In this syntax, variable_lvalue is a data type that is valid
  for a procedural assignment statement".
- 6.21 L7007-7008: "Automatic variables and members or elements of dynamic
  variables—class properties and dynamically sized variables—shall not be
  written with nonblocking, continuous, or procedural continuous assignments."

## Kinds

`kinds.sv`: variables, elements and static properties of the three built-in
handle types, plain, with a conditional source, and with an event control.
`delays.sv`: with a delay, a constructor and a wait for the handle; the other
simulator does not implement delays and constructors there.

## Event controls

`elements.sv`: elements, slices and whole arrays of class handles in a module,
a package, an interface and a class. 9.4.5 L12123-12125: "An intra-assignment
delay or event control shall delay the assignment of the new value to the
left-hand side, but the right-hand expression shall be evaluated before the
delay, instead of after the delay." The other simulator ignores a control on a
signal here and assigns at once, so `h2[1][2]` differs there until the edge.
`strings.sv`: arrays of strings, whose nonblocking assignments the other
simulator refuses.

## Operand order

`operand_order.sv`: the delay before the value and the value before the
selectors of the target; under an event control the selectors, the value and
then the repeat count. 10.4.2 leaves the order open; the other simulator has it
for the forms it supports.

## Ended issuers

`static_ends.sv`: decision
[S09-D8](../../../../../docs/lrm_decisions.md) for elements and members of
static class properties and for built-in handles: their updates of the time
slot end with the process that issues them; ordinary ones stay.

## Rejected

`neg_class_property.sv`: a handle in an object.
