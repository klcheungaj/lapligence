# Reclaiming unreachable object graphs (SIM-018)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set), once with the default collector policy and once
with `LLG_GC_STRESS=1 LLG_GC_VERIFY=1`: a collection at every scheduler safe
point, with unreachable objects kept allocated but poisoned, so an access
through a root the collector missed ends the run with
`access to a reclaimed class object`. Expected outputs are derived by hand
from IEEE 1800-2009, not captured from llg.

Rules used throughout:

- An object stays alive while any handle can still reach it, directly or
  through other objects; there are no finalizers, so reclamation is not
  observable (SV 8.27). A cycle that nothing outside it reaches is garbage.
- Handles are compared by identity (SV 8.4, 11.4.5); `$urandom` draws come from
  the calling process's random stream (SV 18.13, 18.14). Neither depends on
  when the collector runs.
- Collector statistics (`LLG_GC_STATS=1`, printed at model close) are the
  memory oracle: `allocated` objects, `freed` (or `condemned` under
  `LLG_GC_VERIFY`), `live` at close, `peak` live. A collection is requested
  when the number of objects allocated since the last one reaches the
  threshold (`LLG_GC_THRESHOLD`, default 4096) and runs at the next safe point:
  after the running process suspends. The next threshold is
  `max(threshold, live * 100% )`.

| Fixture | Clause and independent oracle |
| --- | --- |
| `cycle_plateau` | §8.27. One kept object, then 20000 iterations that each create a two-object cycle `a <-> b` (static `a`/`b` overwrite the previous pair) and wait `#1`. Per iteration `a.peer.v - b.peer.v + b.peer.pad[3] - i` = `(i+1) - i + i - i` = 1: `sum=20000 keep=-1`. 40001 objects are allocated. With threshold 100 the 100th allocation since a collection is the first (keep counts) allocation of iteration 50, so collections run after iterations 50, 100, ..., 20000: 400 collections. Each keeps `keep` and the current pair (3); just before each one 3 + 100 objects are live: `peak=103`, final `live=3`, `freed=39998`. With the default threshold 4096 collections run after iterations 2048k (k = 1..9), each leaving 3 objects, `peak=4099`; at close 3 + 2 x (20000 - 18432) = 3139 are live and 40001 - 3139 = 36862 were freed. With `LLG_GC=0` all 40001 stay until close. |
| `suspended_child` | §§8.27, 9.3.2. `hold_cycle(10)` builds `a(10) <-> b(11)`, forks a `join_none` branch that captures `a` and copies `a.peer` into its own `c`, and returns; only the suspended branch reaches the cycle while the parent churns 40 self-cycles (one per time unit). At 40 the parent prints `parent done at 40`; at 50 the branch prints `a.v c.v c.peer.v c.peer.peer.v` = `child 10 11 10 11`. |
| `mailbox_only` | §§8.27, 15.4. `make_cycle(1)` is put into `queued` and the producer churns 30 objects; the message is the only reference. `get` returns the head: `queued 1 2 1 n=0`. A getter blocked on `handed` from time 0 is handed `make_cycle(5)` at 30; before it resumes the producer's churn continues in the same slot, so the pending delivery is the only reference: `handed 5 6 5 at 30`. |
| `queued_action` | §§8.27, 4.4.2.4, 10.4.2. `later <= #20 make_cycle(7)` holds the cycle in the scheduled update until 20; `#3 soon <= make_cycle(30)` holds another across the Active-region turns of time 3 until the NBA region. After 30 churn steps: `later 7 8 7 soon 30 31 30`. |
| `receiver_pin` | §§8.27, 8.10, 13.5. `h.slow_sum(r)` suspends for 30 inside the method; at 1 another branch sets `h = null`, so only the activation's `this` reaches `h(3) -> peer(4) -> h`. At 30 `r = 3 + 4 + 3`: `r=10 null=1`. |
| `containers_records` | §§8.27, 8.9, 7.5, 7.8, 7.10. Each churn step builds `x <-> y` twice over: through their queue properties and through record properties whose `peer_c` member points back. Kept: `q[0]` = bag 1, whose queue holds bag 2, whose record names peer 10 with `back` = bag 1 and string `kept`: `q 1 2 10 1 kept`; `da[1]` = bag 3, `da[0]` null: `da 3 null=1`; `aa["k"]` = bag 4 with record peer 5: `aa 4 5`; class static `registry` = bag 6 <-> bag 7 through queue properties: `static 6 7 6`. Allocations: 8 kept + 40 x 4 churned = 168; under verify all 160 churned are condemned and 8 stay live. |
| `deep_chain` | §8.27. A list `1 -> 2 -> ... -> 5000` (yielding every 100 nodes) sums to 5000 x 5001 / 2 = `full 12502500`. Cutting after node 2500 and churning 20 self-cycles leaves 2500 x 2501 / 2 = `half 3126250`. Under stress/verify: allocated 5000 + 20, peak 5001 (the whole list plus the first churn object), condemned 2500 + 19, live 2501. |
| `identity_random` | §§8.4, 18.13, 18.14. `b = a`, `c` a different object with equal contents: `eq 1 0 1 1`; after `b = new(2)`: `eq 0 0 1`. The eight `$urandom` draws (seed 5) are printed too; their values are implementation-defined, so the oracle is that the whole output with `LLG_GC_STRESS=1` equals the output with `LLG_GC=0`. |

Invalid collector settings (`LLG_GC_THRESHOLD=0`, `LLG_GC_GROWTH_PERCENT=x`,
`LLG_GC=2`, `LLG_GC_STRESS=yes`) are configuration errors reported before
execution (exit 1, no output), like `LLG_ZERO_LOOP_LIMIT`.
