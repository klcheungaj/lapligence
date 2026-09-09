# Event and executable model

`ExecutionModel` owns typed process operations and is the sole whole-model
input to optimization and C emission. Each process has explicit blocks, an
entry block, effects, and a terminator. Terminators distinguish completion,
backedges, and atomic suspend/resume with a trigger plan and scheduling region.

The current vertical slice makes process-wrapper control explicit. Nested
branches and loops remain structured typed operations because flattening them
would not improve the C coroutine backend. Blocking stores and NBA enqueue,
suspension, event triggers, process spawning, and runtime services remain
distinct effects. Optimizers walk every execution-owned block in place, then
recompute and validate effect summaries. Resume blocks may differ from entry
blocks; no process body is reconstructed from the staging shape.

The scheduling-region enum reserves the IEEE regions needed by later property
and program support. The current backend emits active work and NBA updates;
the presence of an enum variant does not claim runtime support.

| Executable item | Current meaning |
| --- | --- |
| `ExecutionBlock.operations` | Ordered typed operations; structured branches and loops retain source evaluation order. |
| `Complete` | End the process and release its coroutine. |
| `Jump` | Continue at another block without advancing simulation time. |
| `Suspend(Signals)` | Atomically wait on its unique signal-address set, then continue in the named resume block. |
| `Suspend(BodyControlled)` | Continue in the named resume block after a waiting operation in the block yields and returns. |
| `ImmediateStore` | Blocking variable/net contribution update in the active region. |
| `EnqueueUpdate(NonblockingAssign)` | Capture the update payload now and commit it in the NBA region. |
| `Trigger` | Wake the waiters registered on a named event. |
| `Spawn` | Create dynamic process ancestry for a fork operation. |
| `RuntimeService` | Observable simulator service such as display, waveform control, or termination. |

`ExecutionModel::validate` checks block reachability and target closure,
typed-operation references, unique signal triggers backed by emitted packed
storage (including bounded constant array elements), block-local control-label
closure, body-controlled wait ownership, supported resume regions, and packed
capacity. The C emitter uses reserved C labels and independent declaration
scopes for distinct execution blocks, then follows terminators directly. It
derives coroutine stack capacity from all blocks of each process, rather than
from the emptied staging process bodies.

Structured waits have not yet been split into separate execution blocks. That
future lowering must first move locals that live across suspension into checked
frame storage; jumping past a C local initializer cannot represent their
lifetime safely. Current multi-block emission therefore assumes block-local
temporaries do not escape their defining block.

Source-backed process origins are copied from the semantic database into the
staging process and preserved when execution takes ownership. Processes made
by structural lowering use the origin of the continuous assignment, primitive,
port, or procedural statement that caused them. Future tracing and coverage
passes can therefore attach metadata without reconstructing source locations
from generated C names.
