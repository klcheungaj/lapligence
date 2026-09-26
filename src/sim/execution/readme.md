# Event and executable model

`ExecutionModel` owns typed process operations and is the sole whole-model input
to optimization and C emission. Processes have an entry block, explicit blocks,
effects, and terminators. Wrapper control is explicit; nested branches and loops
remain structured operations with source evaluation order.

Ordinary processes use the design scheduling regions. Program processes start
in Reactive, route zero-delay/NBA work through Re-Inactive/Re-NBA, and retain
natural or `$exit` completion accounting.

| Item | Meaning |
| --- | --- |
| `ExecutionBlock.operations` | Ordered typed operations, including structured branches and loops. |
| `Complete` | End the process and release its coroutine. |
| `Jump` | Enter another block without advancing time. |
| `Suspend(Signals)` | Atomically wait on unique signal addresses, then enter the named resume block. |
| `Suspend(BodyControlled)` | Enter the resume block after a waiting operation yields and returns. |
| `ImmediateStore` | Blocking variable/net-contribution update. |
| `EnqueueUpdate(NonblockingAssign)` | Capture a payload for its later NBA commit. |
| `Trigger` | Wake registered named-event waiters. |
| `Spawn` | Create dynamic fork-process ancestry. |
| `RuntimeService` | Observable services such as display, waveform control, or termination. |

Optimizers update every execution-owned block, recompute effects, and validate
summaries. Resume and entry blocks may differ; bodies are never reconstructed
from the emptied staging table.

`ExecutionModel::validate` checks reachable targets, typed references, unique
signal triggers backed by emitted packed storage (including bounded constant
array elements), block-local labels, body-controlled wait ownership, resume
regions, and packed capacity. Emission follows terminators using reserved C
labels and independent declaration scopes. Stack sizing covers every block.

Structured waits remain inside operations, not separate blocks. Block-local
temporaries cannot escape; cross-block live locals require checked frame storage,
not jumps past C initializers.

Process origins survive transfer from semantic capture through staging to
execution. Structurally created processes retain the origin of their assignment,
primitive, port, or procedural statement rather than generated C names.
