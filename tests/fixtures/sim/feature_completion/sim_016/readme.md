# Semaphore collections and shared-lifetime synchronization (SIM-016)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses, not captured from llg.

| Fixture | Clause and independent oracle |
| --- | --- |
| `shared_handles` | §§9.3.2, 15.3, 15.4. The first branch's `get` starts at 0 on the original semaphore, which never receives a key. The task rebinds `s` to `other` at 1 and puts 7 into the mailbox at 2: `box 7 2`. The third branch puts into `s` at 3, which is now `other`, so the fourth branch (waiting on `other` since 2) resumes: `other got 3`. |
| `element_outputs` | §§13.3, 13.5.2, 15.3. `make(pool[1], 2)` copies its new semaphore into the element: `fixed element`. `make_later` returns at 1 and 2, storing into `q[0]` and `m["k"]`: `queue element 1`, `assoc element 2`. `swap(m["k"])` copies in the drained semaphore (`inout saw drained`) and out a new one with 5 keys: `swapped`. `use_ref(r, 1)` waits through the alias; `r.put(1)` at 3 releases it: `ref 1 3`. |
| `neg_ref_element_handle` | Legal by §13.5.2; a `ref` handle formal bound to an element needs retained handle element references and is rejected explicitly. |
| `pools_and_aliases` | §15.3. `pool[i]` starts with `i` keys: `pool[0]` has none, `pool[1]` one, `pool[2]` two. `a`, `b` and `q[0]` name one semaphore; gets arrive at 0, 1 and 2 and each later put of one key (through `b` at 3, `q[0]` at 4, `a` at 5) serves the oldest: `take 1 3`, `take 2 4`, `take 3 5`. |
| `grant_cancellation` | §§15.3, 9.6.3. At 3 the put grants the waiting first branch, which `disable fork` kills before it resumes, so the key is still available: `key returned 3`. The second holder takes a key at 3 and is killed at 4 while holding it; keys are not returned on kill: `held key kept 4`. |
| `shared_handle_formals` | §§9.3.2, 13.5.1. `local_handle` rebinds `p` at 0 before its branch reads it at 2: `branch id=2`. `formal_handle` rebinds its by-value formal before the branch reads it at 3: `formal id=7`. `joined_handle` reads the caller's object: `join id=5`; the caller's `x` is unchanged: `caller id=5`. |
| `fifo_contention` | §15.3.3. `get(0)` with no waiters returns at 0: `zero 0`. A asks for 2 keys at 0 and B for 1 at 1. The single key put at 3 does not satisfy A, and B may not overtake it; the second key at 4 completes A's request: `A got 2 4`. The key put at 5 serves B: `B got 1 5`; no key is left: `left 0`. |
| `record_members` | §§15.3, 7.2. `r.lock` starts with one key: A takes it at 0 and returns it at 2, when B (waiting since 1) takes it and returns it; `try_get` then succeeds and the handle is not null. `arr[0] = b` copies the handle, so `take(arr[0])` (from 2) waits on `b`'s empty pool until the put at 5: `a0 took 5`. `local_rec` (from 5) waits for two keys, put by its branch at 7, which leaves none: `local got 7 try 0`. |
