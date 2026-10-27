# Foreign call contexts (ADV-018)

Architecture decision for DPI-C call chains whose exported tasks consume
time (IEEE 1800-2009 35.5.1.5, 35.8, 35.9, Annex H.9). Status: adopted.

## Problem

A context imported task may call an exported task that executes a delay,
event control or wait (35.5.1.5). The SystemVerilog side must then suspend
while the foreign C frames below the export stay alive, and several such
chains may be suspended at once and resume in any order: chain A enters
first and suspends, chain B enters and suspends, A resumes and returns
before B. Generated HDL is stackless (`llg_co`, [process ABI](../process_abi.md)):
every live HDL value is in a frame, never on the native stack. Foreign C
code is not: its locals and return addresses are on the native stack of the
thread that called it, and the runtime cannot move or rebuild them.

A recursive scheduler pump (run the scheduler from inside the export until
its wait ends) cannot do this: B's foreign frames would sit above A's on the
one native stack, so A could not return before B. Every crossing-order
witness needs one native stack per suspended foreign chain.

## Decision

Each call of a suspendable import runs its foreign code on a **foreign call
context**: an OS thread with its own native stack, taken from a pool. A
single **run token** serializes execution: exactly one thread runs (the
scheduler thread or one context), every other one is blocked on its own
condition variable. There is no parallel HDL and no parallel foreign code.

- **HDL stays on the scheduler thread and stays stackless.** A context runs
  only the import thunk (argument conversion, the foreign call, copy-out)
  and zero-time exports. When foreign code calls an exported task that can
  consume time, the export thunk converts the arguments into the task's
  coroutine frame, pushed on the calling process's chain arena, and hands
  the token back. The import's coroutine shim on the scheduler thread runs
  that frame as an arena call (`LLG_CO_CALL_ARENA`), so its waits suspend
  the calling process like any task call, with the usual scope, activation,
  disable and cancellation handling. When the task returns, the shim hands
  the token to the context, the export thunk copies the outputs back and
  returns to the foreign code.
- **Nesting** (C -> SV -> C -> SV) is a stack of contexts per process chain:
  an exported task running on the scheduler thread may call another
  suspendable import, which takes another context. Each context is resumed
  only by the shim frame that started it, so contexts of different chains
  resume in any order.
- **Who pays.** A context import task is suspendable only when the design
  has an exported task that can consume time (it is then a coroutine with
  one arena call site); its foreign code may reach any export through
  `svSetScope`. Imported functions, noncontext imports and every import of a
  design without such an export keep the direct call path, and their models
  are generated as before.

### Alternatives

| Mechanism | Crossing order | Linux glibc / musl | macOS arm64 | Windows x64 MSVC | Sanitizers | Switch (this host) |
| --- | --- | --- | --- | --- | --- | --- |
| Recursive scheduler pump | no (LIFO only) | yes | yes | yes | yes | none |
| `ucontext` (`swapcontext`) | yes | glibc only; musl has no `makecontext` | deprecated API | none | needs fiber annotations (ASan, TSan) | 0.6 us round trip |
| Windows fibers | yes | n/a | n/a | yes | n/a | not measurable here |
| libaco / assembly stack switch | yes | per-ABI assembly | per-ABI assembly | per-ABI assembly | unsupported | ~0.05 us |
| **OS-thread contexts + run token** | **yes** | **yes** | **yes** | **yes** | **unchanged** | **57 us round trip sleeping, ~1 us polling** |

The switch costs are pthread mutex/condition ping-pong and `swapcontext`
round trips measured on the 32-core WSL2 development host under load;
bare-metal Linux typically shows 5-10 us for a hand-off that wakes a
sleeping thread. The run token therefore polls briefly before it sleeps
(below), which brings a hand-off that returns quickly to about a
microsecond. Threads are the only candidate that builds from the same
C11 source on every target ABI, needs no assembly or deprecated API, and runs
unchanged under ASan, UBSan and TSan (each hand-off is a mutex
acquire/release pair, so TSan sees the ordering). The plan excludes
libaco-style stack tricks. The cost is paid twice per crossing that blocks
(start and return of the import, call and return of a time-consuming
export), only by suspendable chains. A faster fiber backend could later sit
behind the same `llg_dpi_context_*` interface.

**Memory per context:** one thread with a reserved stack of
`LLG_DPI_CONTEXT_STACK_BYTES` (8 MiB by default, a compile-time tunable of
the model build, for example `LLG_CFLAGS=-DLLG_DPI_CONTEXT_STACK_BYTES=...`)
plus a small record. Stacks are reserved address space; pages are committed
only as foreign code touches them. The size is set explicitly because the
platform defaults differ (musl 128 KiB, macOS secondary threads 512 KiB,
Windows 1 MiB) and zero-time exports run their HDL plain functions on the
context stack. Finished contexts return to an idle pool and are reused, so
a chain pays thread creation only when more chains are active at once than
ever before.

## Run token protocol

- Participants are the scheduler thread and each context. A hand-off, under
  the one runtime mutex, records the giver as the receiver's resumer,
  switches the per-participant DPI state (the open-array guard stack and the
  unwinding flag), sets the token and signals only the receiver; the giver
  then waits on its own condition variable until the token returns.
- Before sleeping, a waiting participant polls its own wake flag up to
  `LLG_DPI_CONTEXT_SPIN` times (65536 by default, measured at about 15 us on
  the x86-64 development host and not yet on other targets; a compile-time
  tunable of the model build, 0 sleeps at once).
  Most hand-offs return within that bound (foreign code that returns at
  once, a zero-time export), and waking a sleeping thread costs tens of
  microseconds. Measured on 20000 calls of a context import task on the
  development host: about 3 us per call that also calls an exported task,
  against 125 us when every hand-off sleeps; a direct import call costs
  0.6 us. A chain that stays suspended spends at most one bound of CPU
  time per suspension before it sleeps. The token itself is read only under
  the mutex, so the flag adds no ordering of its own.
- `llg_dpi_now` (the scope/site/depth chain state) stays one global: every
  crossing saves the state it replaces and restores it on the way back, and
  the scheduler thread only runs when every context is blocked in
  SystemVerilog code, so a resumed chain always restores its own state.
- The runtime and generated code keep their single-thread assumptions
  (`g.current`, value-scope stacks, chain arenas, the arena chunk cache):
  only the token holder touches them, and every hand-off orders memory.
  `llg_co` stays free of globals and TLS.

## Disable protocol (35.9)

- **Disable.** A `disable` that targets a block or task around a suspended
  chain wakes the process (cooperative disable, as for any task). The
  exported task unwinds, its thunk returns 1 (item a), `svIsDisabledState()`
  is 1 in the import's C code, which must return 1 (item b); returning 0 is
  a fatal error, as is a further export call (item d). A disable of the
  exported task itself returns 0 and the chain continues.
- **Kill.** A process killed while its chain is suspended (`kill()`,
  `disable fork`, a disable that kills a fork branch) cannot resume HDL.
  Before its frames and value scopes are released, its contexts are unwound
  innermost first through the same protocol: each blocked export returns 1,
  `svIsDisabledState()` is 1 and the import must return 1. An export call
  from unwinding foreign code is a fatal error (item d) and parks that
  context for good, so a kill cannot hang on foreign code.
- **End of simulation.** `$finish`, `$fatal` or the end of the event queue
  while a chain is suspended does not resume it: no foreign code runs after
  the simulation ended, and the blocked threads end with the process. The
  idle pool is joined at shutdown.
- **Imported functions** may not enable an exported task that can consume
  time (35.8): such a call is a fatal error at the call, whether or not
  that call would wait. A zero-time exported task called from an imported
  function still runs (decision S41-D8).

## Rules kept

- No `longjmp` across languages and no asynchronous kill or cancellation of
  foreign threads: foreign code only stops by returning, or blocks in a
  runtime call. The open-array guard (S40-D6) still jumps only within one
  context, from the `svdpi.h` access back to its own import thunk.
- No Rust `unsafe`; the runtime C uses only the platform layer
  (`llg_platform_native.h`) for threads.
- `llg_dpi_context.c` is its own runtime translation unit, so only models
  with suspendable imports reference it, and only they link the platform
  thread library.
