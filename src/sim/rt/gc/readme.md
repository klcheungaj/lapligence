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

Finalizers release fields only and never follow handles, so cycles and order
do not matter. A collector allocation failure abandons the whole collection
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
