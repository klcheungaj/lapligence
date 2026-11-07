# Runtime collected objects

The heap model, safe points, root kinds and exact-address rule are in the
"Collected objects" comment of `llg_rt.h`; `gc/collector.c` owns the heap
(`llg_gc_alloc`) and `gc/roots.c` the scheduler-owned roots. Safe points are in
`run_region_queue` (`!g.current && !g.process_turn_active`) or an explicit
`llg_gc_collect` with no running process; never collect during cleanup. Every new runtime structure that can hold a
class handle across a safe point must be enumerated: add it to
`gc_scheduler_roots` (queued NBAs, wait subscription keys, value scopes and
handle cells tagged by `llg_gc_handle_cell_drop`, captured frames, native
roots, mailboxes, suspended coroutine frames through registered frame maps) or
register a root producer/payload tracer (`llg_gc_register_*`). Covergroup
objects sampled by a static coverage event are pinned by `llg_cov_register`;
a per-instance event waiter holds its object in its captured frame. Addresses that may point into object storage
are reported with `llg_gc_visit_interior`.

An associative array with class keys holds its keys as roots: global ones
through `render_gc_roots`, the rest through the traced storage that holds them
(`llg_gc_trace_assoc`, `llg_gc_trace_assoc_values`, `llg_nested_assoc_trace`).

`llg_gc_header_t` is 16 bytes: `type`, then one word with masks (not bitfields,
so MSVC lays it out alike) holding, from bit 0, 8 mark bits, 16 pin bits and a
40-bit creation serial.

- **Mark:** epochs run 1..255 and a new object's mark is 0. Reaching 255 clears
  every mark before the next collection, so no stale mark can equal the current
  epoch; the clear touches each header once per 255 collections. Six bits cost
  3.5% on a design with 2182 collections of a small heap; eight cost 0.7%.
- **Pins:** the only caller is `llg_cov_register`, which pins a covergroup
  instance with a block sampling event once, when it is built, so generated
  code never holds a count above 1. Waiters, forks, recursion, NBA payloads and
  containers hold objects through roots, not pins, whatever their number. The
  16 bits serve `llg_gc_pin` as an API; pin 65,536 is a fatal naming the limit.
- **Serial:** 2^40 objects, about 12 days at a million objects a second. A model
  with class keys stops with a fatal naming the time; a model without them
  never compares serials and wraps.

The three widths can be narrowed with `-DLLG_GC_MARK_BITS` and friends, which is
how `sim_018` reaches each limit.

Finalizers release fields only and never follow handles, so cycles and order
do not matter. One reclaim hook (`llg_gc_reclaim_hook`, installed by the VPI layer once
an application observes a class object or registers an object callback) sees
every unreachable object of a collection, all still intact, before the first
finalizer runs; it must not allocate collected objects or run model code.
Without a hook the collector pays one predictable branch per collection. A collector allocation failure abandons the whole collection
before anything is finalized, counts it and doubles the threshold.
Registrations are released by `llg_rt_cleanup` and re-made after
`llg_rt_init`; objects are model storage freed by `llg_gc_teardown` at model
close. Policy is read at init (`LLG_GC`, `LLG_GC_THRESHOLD`,
`LLG_GC_GROWTH_PERCENT`, `LLG_GC_STRESS`, `LLG_GC_VERIFY`, `LLG_GC_STATS`;
invalid values are configuration errors). Verify mode keeps unreachable
objects allocated but condemned so an access through a missed root is a
located fatal error; use `LLG_GC_STRESS=1 LLG_GC_VERIFY=1` when adding a root
source. Mailboxes, semaphores, events and pinned process handles stay
model-lifetime ([known issue](../../../../docs/known_issues.md#collected-objects-keep-some-runtime-objects-and-defer-collection)).
