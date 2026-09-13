# Semaphore fixtures

These checked-in designs cover the bounded IEEE 1800-2009 §15.3 semaphore
contract. `basic.sv` checks zero-key construction, the default key count,
blocking-free `try_get`, and zero-key `put` behavior. `fifo.sv` checks that
contention with different requested counts preserves the specified waiter
order. `kill.sv` checks that cancelling a process blocked in `get` removes its
waiter without consuming a later `put`. `task_arg.sv` checks that an automatic
task's semaphore argument remains valid across a blocking `get`.
`local.sv` checks that an automatic semaphore captured by a fork remains live
until its blocked child resumes.
`static.sv` checks that a static procedural declaration initializer runs once
when an enclosing `always` process re-enters.
`suspend.sv` checks that a put waking a suspended waiter records a pending wake
and resumes it only after the process handle is resumed.
`invalid_count.sv` checks that a negative signed key count is rejected at the
runtime boundary.
`task_local.sv` checks that an automatic task can construct and consume a
semaphore local.
`null.sv` checks that an explicit null semaphore initializer preserves a null
handle.

The owning Rust suite runs all fixtures through `llg` and `llg --no-opt`.
Semaphore arrays, mailbox operations, and other advanced synchronization
constructs remain outside this bounded subset.
