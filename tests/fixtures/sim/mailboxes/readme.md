# Mailbox fixtures

These fixtures cover the simulator's bounded mailbox subset from IEEE
1800-2009 §15.4 and Annex G.4. `basic.sv` checks typed/untyped FIFO values,
copy semantics, and class-handle identity; `blocking.sv` checks producer and
consumer waiter order plus non-consuming peeks; `cancellation.sv` checks
`disable fork` cleanup; `typed_values.sv` checks four-state, real, shortreal,
typedef, and enum elements; and `locals.sv` checks nested and automatic local
mailbox storage. `tests/sim_mailboxes.rs` runs each fixture with and without
optimization and compares exact output.
