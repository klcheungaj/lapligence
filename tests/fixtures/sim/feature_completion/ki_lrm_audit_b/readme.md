# LRM audit part B fixes

Fixtures for the deviations found by the part B conformance audit of
processes, events and objects. Clause text is quoted from IEEE 1800-2009
(`docs/specification/SystemVerilog-1800-2009.txt`, cited as "L<line>"). Every
positive fixture runs through the public CLI in both optimizer modes on the
legacy and compact value backends, and again after Db destruction. Each
waiter appends `$time` to its own log string, printed at the end, so wakes of
different processes in one time step cannot reorder the output.

## Clauses

§9.4.2, L11822-11824:

> A variable used with the event control can be any one of the integral data types (see 6.11.1) or string. The
> variable can be either a simple variable or a ref argument (variable passed by reference); it can be a mem-
> ber of an array, associative array, or object (class instance) of the aforementioned types.

§9.4.2, L11830-11831:

> If the event expression is a reference to a simple object handle or chandle variable, an event is created when
> a write to that variable is not equal to its previous value.

§9.4.2, L11833-11835:

> Nonvirtual methods of an object and built-in methods or system functions for an aggregate type are allowed
> in event control expressions as long as the type of the return value is singular and the method is defined as a
> function, not a task.

§9.4.2, L11837-11840:

> Changing the value of object data members, aggregate elements, or the size of a dynamically sized array ref-
> erenced by a method or function shall cause the event expression to be reevaluated. An implementation can
> cause the event expression to be reevaluated when changing the value or size even if the members are not
> referenced by the method or function.

§15.4.2, L20504-20507 (mailbox `num()`):

> The num() method returns the number of messages currently in the mailbox.
>
> The returned value should be used with care because it is valid only until the next get() or put() is exe-
> cuted on the mailbox. These mailbox operations can be from different processes from the one executing the
> num() method.

§9.4.3, L12032-12033:

> The wait statement shall evaluate a condition; and, if it is false, the procedural statements following the wait
> statement shall remain blocked until that condition becomes true before continuing.

§9.7, L12644-12648:

> The resume() function restarts a previously suspended process. Calling resume on a process that was sus-
> pended while blocked on another condition shall resensitize the process to the event expression or to wait for
> the wait condition to become true or for the delay to expire. If the wait condition is now true or the original
> delay has transpired, the process is scheduled onto the Active or Reactive region to continue its execution in
> the current time step.

§9.7, L12617-12618 (the `process::state` values printed):

> —      WAITING means the process is waiting in a blocking statement.
> —      SUSPENDED means the process is stopped awaiting a resume.

## Positives

| Fixture | Derivation |
| --- | --- |
| `string_props` | `h` names `a` (`s = "x"`); `b.s = "y"`. 1: an equal store to `a.s` is no change (L11830-11831 compares with the previous value), so no event. 2: `b` is not named. 3: `h = b` changes `h.s` from `"x"` to `"zz"`: `@(h.s)` wakes. 4: `h.s = "go"`: `@(h.s)` wakes and the wait on `h.s == "go"` completes (L12032-12033). 5: `a` is no longer named. 6: `h.nxt = a`: the wait on `h.nxt != null` completes and `@(h.nxt)` wakes (L11830-11831 applied to the handle property's value). 7: `h.s = "four"`: `@(h.s)` wakes and `h.s.len() == 4` completes. 8: equal stores to `h.s` and `h.nxt` wake nothing. 9: `h = a` changes `h.s` (`"four"` to `"go!!"`) and `h.nxt` (`a` to null): both event controls wake. |
| `suspended_queue_waiters` | N1, §9.7 L12618 "SUSPENDED means the process is stopped awaiting a resume." and L12644-12647 (resume resensitizes "to wait for the wait condition to become true"); §15.3.2 L20400-20402 ("If a process has been suspended waiting for a key, that process shall execute if enough keys have been returned"). Decisions AB-N1 (industry-standard behaviour): a suspended waiter keeps its queue place and stays a candidate for keys, messages and space; it takes them while suspended and continues when resumed. AB-N4: a waiting request that cannot be met does not block a later one that can. At 0 `s1` waits for 3 keys of `big` and `s2` (queued behind it) for 1; `p1` then `p2` block putting into the full bound-1 mailbox; `k1` gets and `pk` peeks on the empty `pm`. At 1 `s1`, `p1`, `k1`, `pk` are suspended and one key is put: `s1` still needs 3, `s2`'s request fits and is served (`s2 done 1, s1 SUSPENDED`). At 2 `try_get` frees space (`first=1`); the suspended `p1`, first in line, fills it (`n=1`) and stays SUSPENDED. The suspended `k1` is killed and 9 is put: the suspended peek waiter `pk` copies it and it stays queued (`n=1`). At 3 `pk` and `p1` resume; 3 keys are put, the suspended `s1` takes them and resumes. `p2` is still blocked behind the full mailbox. The resumed processes run at 3 in llg's order for one time step: log `s2@1 pk9@3 s1@3 p1@3`; `full n=1 pm n=1`. |
| `disable_branch_status` | N3, §9.7 L12619 "KILLED means the process was forcibly killed (via kill or disable)." and §9.6.2 L12387-12388 "Execution shall resume at the statement following the block". Decision: when the disabled named block is a fork branch's entire body nothing follows it, so the branch process is forcibly ended: `KILLED`. `other_blk` is disabled by the parent at 1 and `self_blk` disables itself at 1 (no `unexpected self`): both `KILLED`. `part_blk` is followed by `after = 1`, so its process resumes and ends `FINISHED` (`after=1`). `normal_blk` ends at 1 without a disable: `FINISHED`. A `fork ... join` whose branch `joined_blk` is disabled by its sibling at 2 joins at 2 with `joined KILLED`. |
| `resume_resensitize` | Six children block at 0 on `@e`, `@(posedge clk)`, `@(h.x)` (evaluated by the waiting process), `@(v or e)`, `wait (v == 1)` and `#3`; all are suspended at 1. At 2 `->e`, `clk` rises, `h.x` and `v` become 1: every child is still SUSPENDED at 3 (L12618), and `pd`'s delay transpires at 3. All are resumed at 4: the wait condition is now true and the delay has transpired, so both run at 4 (L12646-12648); the four event controls are resensitized (L12645) and are WAITING at 5. `->e` at 5 wakes `@e` and `@(v or e)`. `clk` falls at 6 and rises at 7. `h.x = 1` at 8 is no change from the value when `@(h.x)` was resensitized (1); `h.x = 0` at 9 is. All six are FINISHED at 10. |
| `handle_writers` | L11830-11831 compares a handle write with the previous value, whatever the writer. 1: the blocked `m.get(late)` takes the first message; the second stays queued. 2: `m.get(h)` writes `src` over null: `@(h)` wakes. 3: `m.peek(h)` writes the same handle: no event. 4: task output `make(h)`, 5: function output `fmake(h)`, 6: task-form `$cast(h, d)`: each writes a new object. 7: `make(n.nxt)` rebinds the handle property, so `n.nxt.v` goes from 0 to 7 (L11837-11840: the expression is reevaluated). 8: `try_get` writes `src` (`v = 3`) into `n.nxt`. 9: `$cast(n.nxt, d)` (`v = 4`). 10: `make(late)` changes `late`. 11: `m2.put` completes the get blocked since 0, which writes `h2`. |
| `callee_reads` | Waits on functions and methods whose bodies read object members (L11837-11840): `fx(h)` reads `c.x` through its handle formal; `fnx` reads `c.n.x`; `fget` calls the method `c.get()`; `fwrap` calls `fx`; the method `nx()` reads `n.x`; `gvx()` reads the module variable `gv` and `x`; `fglobal()` reads `k.x` through a module handle; `fstatic()` reads the static property `C::sx`. Each condition becomes true at the step that writes its operand: 1, 2, 3, 4, 5, 6 (`gv + x = 12 + 4`), 7 and 8. `@(fx(h))` wakes when `h.x` changes (1, 3, 4). At 9 `h` is rebound to a new object whose `x` is also 4: the value is unchanged, so no event. At 10 the old object is written, no longer named. At 11 the new object's `x` changes. |

| `observed_storage` | Waits whose operands are aggregate elements, sizes and members of class objects, and mailbox counts (L11822-11824 "a member of an array, associative array, or object"; L11833-11835 built-in methods of an aggregate type; L11837-11840 "the size of a dynamically sized array ... shall cause the event expression to be reevaluated"; L20504-20507 `num()`; L12032-12033 the wait completes when its condition becomes true). Each waiter records its own wake time, so same-step order does not show. 1: `h.q.push_back(3)` makes `h.q.size() == 1` (`q=1`) and changes `h.q.size()`, so `@(h.q.size())` armed at 0 wakes (`ev=1`). 2: `h.fa[1] = 5` (`fa=2`). 3: `h.da = new[2]` (`da=3`). 4: `h.aa["k"] = 1` (`aa=4`). 5: the second push makes `q.size() == 2` inside the method `wait_own` (implicit `this`, `own=5`). 6: the third push of 7 makes `h.q[2] == 7` (`q2=6`). 7, 8: two puts make `m.num() == 2` at 8 (`m=8`). 9: `h.n.q` gains an element (`nq=9`, selected through the handle property `n`). 10: `h.n.mb.put` (`mb=10`). 11: the inherited queue of the derived object `d` (`d=11`). 12: `k.aa["x"]` (`bind=12`). 13: `k.q` is assigned three elements; `g` still names its own object. 14: `g = k` rebinds the handle, so `g.q.size()` is now 3 (`rebind=14`). 15: `m2.put(8)` wakes the waiter on `mw.num()`, which `mw = m2` bound at 0 (`mbind=15`). 16: `m = m2` rebinds a handle no one still waits on. `k.q.size()` never reaches 9 (`never=-1`). |
| `callee_dataflow` | L11837-11840 "Changing the value of object data members, aggregate elements, or the size of a dynamically sized array referenced by a method or function shall cause the event expression to be reevaluated", applied to storage a function reads through handles it receives or copies. 1: `alias_x` reads `d.x` after `d = c`, with `c` bound to `h` (`alias=1`). 2: `chain_x` copies `c` into `first` and `first` into `second` (`chain=2`). 3: `vread` reads `vv.s` through its virtual-interface formal bound to `v`, which names `i0` (`vif=3`). 4: `vglob` reads the module handle `v` (`vglob=4`). 5: `qsize` reads `t.q.size()` through a local copy (`size=5`). 6: the method `total()` sums `q`: 4 + 5 = 9 (`total=6`). 7: `mnum` reads `c.mb.num()` (`num=7`). 8: `v = i1` rebinds the handle `vglob` reads; `i1.s` is 6 (`rebind=8`). |

## Negatives (implementation restrictions)

These forms are legal, but llg cannot observe their changes. They are
rejected with the source location instead of compiling a wait that could
never resume:

- `neg_callee_alias`: a function reads `d.x` through a local that holds one
  of two handles chosen at run time; a callee local is followed only when
  every write stores the same handle a wait can evaluate again (a bound
  handle formal, `this`, a handle property, a static handle or another such
  local).
- `neg_container_property`: the same for a queue property (`d.q.size()`).
- `neg_mailbox_query`: `s.try_get(1)`; only a mailbox `num()` publishes a
  change marker, and `try_get` takes keys whenever it is evaluated.
- `neg_virtual_method`: `h.get()` where `get` is virtual; the implementation
  that runs, and so the storage it reads, is chosen at run time. (L11833-11835
  admits only nonvirtual methods in event control expressions; a `wait`
  condition has no such rule. Industry-standard behaviour also rejects a
  virtual method in a `wait` condition, so llg keeps the rejection.)
