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
| `resume_resensitize` | Six children block at 0 on `@e`, `@(posedge clk)`, `@(h.x)` (evaluated by the waiting process), `@(v or e)`, `wait (v == 1)` and `#3`; all are suspended at 1. At 2 `->e`, `clk` rises, `h.x` and `v` become 1: every child is still SUSPENDED at 3 (L12618), and `pd`'s delay transpires at 3. All are resumed at 4: the wait condition is now true and the delay has transpired, so both run at 4 (L12646-12648); the four event controls are resensitized (L12645) and are WAITING at 5. `->e` at 5 wakes `@e` and `@(v or e)`. `clk` falls at 6 and rises at 7. `h.x = 1` at 8 is no change from the value when `@(h.x)` was resensitized (1); `h.x = 0` at 9 is. All six are FINISHED at 10. |
| `handle_writers` | L11830-11831 compares a handle write with the previous value, whatever the writer. 1: the blocked `m.get(late)` takes the first message; the second stays queued. 2: `m.get(h)` writes `src` over null: `@(h)` wakes. 3: `m.peek(h)` writes the same handle: no event. 4: task output `make(h)`, 5: function output `fmake(h)`, 6: task-form `$cast(h, d)`: each writes a new object. 7: `make(n.nxt)` rebinds the handle property, so `n.nxt.v` goes from 0 to 7 (L11837-11840: the expression is reevaluated). 8: `try_get` writes `src` (`v = 3`) into `n.nxt`. 9: `$cast(n.nxt, d)` (`v = 4`). 10: `make(late)` changes `late`. 11: `m2.put` completes the get blocked since 0, which writes `h2`. |
| `callee_reads` | Waits on functions and methods whose bodies read object members (L11837-11840): `fx(h)` reads `c.x` through its handle formal; `fnx` reads `c.n.x`; `fget` calls the method `c.get()`; `fwrap` calls `fx`; the method `nx()` reads `n.x`; `gvx()` reads the module variable `gv` and `x`; `fglobal()` reads `k.x` through a module handle; `fstatic()` reads the static property `C::sx`. Each condition becomes true at the step that writes its operand: 1, 2, 3, 4, 5, 6 (`gv + x = 12 + 4`), 7 and 8. `@(fx(h))` wakes when `h.x` changes (1, 3, 4). At 9 `h` is rebound to a new object whose `x` is also 4: the value is unchanged, so no event. At 10 the old object is written, no longer named. At 11 the new object's `x` changes. |

## Negatives (implementation restrictions)

These forms are legal, but llg cannot observe their changes. They are
rejected with the source location instead of compiling a wait that could
never resume:

- `neg_callee_alias`: a function reads `d.x` through a local copy `d` of its
  handle formal; only class-handle formals, `this`, handle properties and
  static handles are followed into a callee.
- `neg_container_property`: a queue class property (`h.q.size()`); per-object
  containers publish no change marker.
- `neg_mailbox_query`: `m.num()`; mailbox and semaphore state publishes no
  change marker.
- `neg_virtual_method`: `h.get()` where `get` is virtual; the implementation
  that runs, and so the storage it reads, is chosen at run time. (L11833-11835
  admits only nonvirtual methods in event control expressions; a `wait`
  condition has no such rule, so this is a restriction.)
