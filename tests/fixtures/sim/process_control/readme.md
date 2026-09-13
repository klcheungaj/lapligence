# Process-control fixtures

These checked-in SystemVerilog designs cover the bounded IEEE 1800-2009
§9.7 process-class contract. `control.sv` observes stable `self()` identity,
running/waiting/suspended/finished states, suspension of an outstanding event
wait, resume, and both later-ending and already-ended `await()` calls.
`kill_tree.sv` checks recursive descendant cleanup and confirms that a delayed
nonblocking assignment issued by an unrelated process is not canceled.
`kill_join.sv` checks that killing a child also completes its parent fork-group
accounting, allowing `wait fork` to return and preserving the terminal handle
status.
`static_handle.sv` covers a block-scoped static handle alongside the automatic
handle in `control.sv`.

The owning Rust suite runs both fixtures through `llg` and `llg --no-opt`.
Process formals, process arrays, semaphores/mailboxes, and the broader class
API remain outside this bounded subset.
