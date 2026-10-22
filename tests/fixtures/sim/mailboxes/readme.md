# Mailbox fixtures

These fixtures cover the simulator's bounded mailbox subset from IEEE
1800-2009 §15.4 and Annex G.4. `basic.sv` checks typed/untyped FIFO values,
copy semantics, and class-handle identity; `blocking.sv` checks producer and
consumer waiter order plus non-consuming peeks; `cancellation.sv` checks
`disable fork` cleanup; `delivery.sv` checks that the put waking a blocked
get writes whole-variable, class-property and class-handle destinations at
once and selected destinations when the receiver resumes, and
`killed_receiver.sv` checks what receivers killed right after the put keep
and that a handle written at the put wakes `@(h)` (decision AB-O3);
`typed_values.sv` checks four-state, real, shortreal,
typedef, and enum elements; `locals.sv` checks nested and automatic local
mailbox storage; `block_records.sv` checks that a fixed unpacked record
declared in a procedural block (static or automatic) travels through `put`,
`try_put`, `get`, queue, dynamic-array and associative-array element writes
with its four-state bits; and `scoped_typedefs.sv` checks that same-named module,
package and compilation-unit typedef elements keep their own widths, on both
value backends. `tests/sim_mailboxes.rs` runs each fixture in both optimizer modes and compares exact output.

## Clauses

IEEE 1800-2009 15.4.3 (`SystemVerilog-1800-2009.txt` L20513):

    The put() method places a message in a mailbox.

IEEE 1800-2009 15.4.5 (L20540):

    The get() method retrieves a message from a mailbox.
