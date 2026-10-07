# Mailbox aggregate messages, arrays and captures (SIM-017)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses of IEEE 1800-2009, not captured from llg.

Rules used throughout:

- A message is a copy of the value put (SV 15.4.4): unpacked records, arrays,
  queues and dynamic arrays are copied deeply (SV 7.2, 7.5, 7.10), so changing
  the source after `put`, or the received copy afterwards, affects nothing
  else. Class, event, process, virtual-interface and mailbox handles are
  copied as handles and keep naming the same object (SV 8.4, 15.5.5).
- `try_put`/`try_get`/`try_peek` return 0 when the mailbox is full or empty,
  a positive value on success and a negative value when the message type is
  not equivalent to the destination type (SV 15.4.6, 15.4.8, 6.22). A failed
  call changes neither the mailbox nor the destination; `peek` never removes
  the message (SV 15.4.7). `num` counts queued messages (SV 15.4.3).
- The message argument of `get`/`peek` is a `ref` (Annex G.4), so its
  selectors are fixed when the call starts (SV 13.5.2).
- A blocked receiver writes its destination when it resumes. One killed or
  disabled before then never writes; a message that was already handed to it
  goes back to the head of the mailbox (a peek copy is discarded). The project
  defines this order; it is the reading of SV 9.6.3 and 9.7 under which no
  message is lost, received twice or written into dead storage.

| Fixture | Clause and independent oracle |
| --- | --- |
| `records` | §§15.4, 7.2, 7.10, 8.4. `src` = `{1, "alpha", 0.5, {11,22}, n}` with `n.v = 10` is put into an unbounded and a bounded typed mailbox and an untyped one; afterwards `src` becomes `{2, "beta", 2.25, {99,22,33}}` and `n.v = 20`. Every received copy is the original value (`1 alpha 0.50 2 11 22`), but its handle names `n` (`20 same=1`). Changing the copy's queue leaves `src.data[1] = 22`, size 3. A nested record built from `src` with tag `outer` is changed after the put; the received copy is `outer beta 3 33 2.25`. |
| `containers` | §§15.4, 7.5, 7.10. A bounded(1) mailbox holds `{1,2,3}`: `try_put` is 0. The received queue is `1 2 3`; the source is `100 2`, size 4 after its later changes. `peek` then `get` of a string queue return `x yy` (the source changed `s[0]` after the put; the peeked copy's change is overwritten by the get). A dynamic array of `logic [3:0]` keeps `0001 xxxx 1111`, size 3. A queue of records arrives as `7 seven 8 eight` although the source was changed and emptied. In an untyped mailbox an `int` queue does not match a `string` queue (`-1`, still `n=1`) and matches another `int` queue (`1`, size 3, `1 50 3`). |
| `array_patterns` | §§15.4, 10.9. Pattern messages `{1,2,3,4}` and `{default:7}` into a record member of the array type: `1 2 3 4 h` (the other member is kept), then `try_peek` succeeds with `7 7`, `n=1`. |
| `handles` | §§15.4, 8.4, 15.5.5, 9.7, 25.9. The received class handle is the object (`same=1 v=2`); rebinding the variable later does not change it (`2 3`). The received event names `e1`'s object, so `->e1` at 5 wakes `@(e2)` (`e2 woke at 5`). A peeked process handle is the waiting child (`same=1 WAITING n=1`). Virtual interfaces arrive in FIFO order (`5a`, `a5`). A mailbox sent through a mailbox is the same mailbox (`same=1 x=9`). In an untyped mailbox holding a class handle, an event and a mailbox, only the matching destination type succeeds: `-1 n=3`, `1 v=3`, `-1`, `1`, `1 same=1 n=0`. |
| `try_variants` | §§15.4.3-15.4.8, 6.22. Empty: both 0, `y` kept. A bound of 2 accepts two `try_put`s and refuses the third (`1 1 0 n=2`). `try_peek` copies `1 one` and keeps `n=2`; `try_get` removes it. A full mailbox refuses `try_put`; FIFO gives `2 two`. An untyped record message does not match a different record type with the same members (nominal, §6.22.1), `int`, `string` or `real`: all `-1`, `n=1`, destinations kept; the matching type gets `2 two`. `logic [7:0]` `x5` does not match `byte` or `int` (different sign/state/width, §6.22.2), matches `logic [7:0]` (`x5`). |
| `selected` | §§13.5.2, 15.4.5, 15.4.7. Receivers block at 0 on `a[idx]`, `r.f`, `q[$]` (then `q[0]`), an automatic task local and `rq[idx]` with `idx = 0`; `idx` becomes 1 before the messages arrive at 1. Writes go to the selections made at the call: `a 11 0 0`, `r.f 22`, `q 33 33` (peek into `q[1]`, then get into `q[0]`), the task returns `330`, `rq 7 seven 2 b`. All mailboxes end empty. |
| `members` | §§15.4, 8.4, 7.2, 13.5. A record copy shares its mailbox member (`record 5 n=0`). A class's bounded(1) property is full after one put (`class full=0`); its untyped property works (`class 6 8`). The same mailbox stored in fixed, dynamic, associative and queue elements and passed by value receives `9` and `10` (`arrays 9 n=2`, `arrays 9 10`). A `ref` formal constructs the caller's mailbox (`ref 11 null=0`). |
| `cancellation` | §§15.4, 9.6.3, 9.7. A killed blocked getter never writes (`keep=-1 untouched`, the later message stays, `n=1`). A killed blocked putter adds nothing (`n=2`; FIFO `2 two`). A getter handed message 4 is killed before it resumes; message 5 was queued meanwhile: before the kill `n=1`, after it 4 is back at the head (`n=2`, `wrote=0`, then `4 four`, `5 five`). A peek waiter killed after its wake leaves the message (`n=1`, `str` empty). `disable fork` of a getter with an automatic destination leaves the next message queued (`n=1 wrote=0`). The model ends with two queued records, a blocked putter and a blocked getter (`teardown n=2 0 str=p`); the sanitizer runs cover the teardown. |
| `reentry` | §§15.4, 9.4.2, 10.3. Message 1 resumes the receiver (`dest=1`); `always @(dest)` puts 101, handed to the receiver already blocked in its next get; likewise 2 and 102: `echoes 4: 1 101 2 102`. The continuous assignment's `try_peek` always sees an empty mailbox (`peeked=-1`). After the receiver finished, 3 stays queued (`n=1`). |
| `mailbox_aggregate`, `mailbox_auto_capture` | Adopted FND-002 witnesses (ledger L-F12-07-02, L-F12-07-03); each expected line is the witness's `Expected:` header; the copies end with a quiet `$finish(0)`. |
| `neg_blocking_mismatch` | §15.4.5: a blocking `get` whose destination type does not match the message is a run-time error; it reports `mailbox retrieval type mismatch` and ends the simulation (exit 1) after `before`. |
| `neg_assoc_message` | Legal by §15.4; the runtime message value has no nested associative-array form, so it is rejected explicitly. |
| `neg_array_variable` | Legal by §15.4; a whole unpacked-array variable has no transfer to or from a message value (patterns and record members do), so it is rejected explicitly. |
| `neg_loop_condition` | Legal by §15.4.6; the copy-out of a record `try_get` in a loop condition would need statements around every evaluation of the condition, so it is rejected explicitly. |
