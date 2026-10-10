# Process-control fixtures

These checked-in SystemVerilog designs cover the bounded IEEE 1800-2009
§9.7 process-class contract. `control.sv` observes stable `self()` identity,
running/waiting/suspended/finished states, suspension of an outstanding event
wait, resume, and both later-ending and already-ended `await()` calls. The
posedge of `trigger` while the worker is suspended is not delivered: §9.7
(`SystemVerilog-1800-2009.txt` L12644-12647) says "Calling resume on a process
that was suspended while blocked on another condition shall resensitize the
process to the event expression or to wait for the wait condition to become
true or for the delay to expire. If the wait condition is now true or the
original delay has transpired, the process is scheduled onto the Active or
Reactive region to continue its execution in the current time step." After
`resume()` the worker is WAITING (2) again, and a second posedge wakes it.
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
