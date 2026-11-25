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

### Descendants of finished processes

The fixtures below check that `kill()` and `disable fork` reach processes
whose parent, and any process above it, has already finished. Each `.out`
holds the exact stdout, which an independent industry-standard simulator
prints too. Clause lines are from `SystemVerilog-1800-2009.txt`:

- 9.7 L12631-12632: "The kill() function terminates the given process and
  all its subprocesses, that is, processes spawned using fork statements by
  the process being killed."
- 9.6.3 L12548: "The disable fork statement terminates all active
  descendants (subprocesses) of the calling process." L12553-12555: "The
  disable fork statement terminates all descendants of the calling process as
  well as the descendants of the process's descendants. In other words, if
  any of the child processes have descendants of their own, the disable fork
  statement shall terminate them as well."
- 9.6.1 L12335-12336: "The wait fork statement blocks process execution flow
  until all immediate child subprocesses (processes created by the current
  process, excluding their descendants) have completed their execution."
- 9.7 L12615, L12619: "FINISHED means the process terminated normally."
  "KILLED means the process was forcibly killed (via kill or disable)."

| Fixture | Checks |
| --- | --- |
| `kill_orphan_depth` | Roots 2 to 4 are killed with their leaves two to four generations down, every process between them finished; root 5 is not killed and its leaf fires. |
| `kill_orphan_joins` | Leaves outliving a finished `join_any` parent and a finished joined branch end with their root, as does a live sibling. |
| `kill_orphan_waits` | Killed leaves waiting on an event, a delay, a semaphore, a mailbox, a wait condition and an edge never resume; the key and the message stay (`try_get=1 num=1`). |
| `kill_orphan_self` | A root kills itself and its orphaned leaf; an orphaned leaf kills its own root, which ends the leaf itself (nothing after its `kill()` runs) and its sibling. |
| `kill_orphan_await` | `await()` of the orphaned leaf returns at the kill with `KILLED`; the finished middle process stays `FINISHED`. |
| `kill_orphan_status` | After the root's kill its live descendants are `KILLED`; its finished ones, with or without live children, stay `FINISHED`. |
| `kill_finished_process` | `kill()` of a process that has finished ends the child it left running; the process stays `FINISHED`. |
| `wait_fork_orphans` | `wait fork` returns when the immediate child finishes, not when its grandchild does; `disable fork` then ends the grandchild. |
| `disable_fork_orphans` | `disable fork` ends leaves below a finished child's finished child, a live child's finished child and a finished `join_any` branch; another initial's tree runs on. The status of the finished processes is decision KO-D1 (the industry-standard simulator reports `KILLED` for those that still had live children). |
| `orphan_records_reclaimed` | 2000 trees ended by a kill of the live root, `disable fork`, the leaves' own completion and a kill of the finished root: `fired=1000`, and the `$finish(2)` statistics show one registered process, so the records kept for finished processes are reclaimed (llg-specific stderr). |
| `bound_updates_reclaimed` | 2000 sets of timed triggers and handle and static property updates whose issuers are killed live or finished, end with `disable fork` or a `disable` of the block they were started in, or commit: only the last fire, the ordinary updates between them commit in issue order, and the `$finish(2)` statistics show the six static processes (llg-specific stderr), with updates of every kind still pending. |
| `bound_updates_exit` | `$exit` drops the timed triggers of its program's processes, also of a finished one; another program's fire. |
| `bound_updates_times` | Dropped triggers leave no time to advance to; an ordinary update of the same time keeps it. |

The owning Rust suite runs every fixture through `llg` and `llg --no-opt`.
Process formals, process arrays, semaphores/mailboxes, and the broader class
API remain outside this bounded subset.
