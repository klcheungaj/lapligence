# RTL-007b effectful helpers in runtime-evaluated contexts

IEEE 1800-2009 §§4.4.2.9, 9.4.2, 9.4.2.3, 9.4.5, 10.4.2, 10.6, 10.6.2, 13.3.2,
13.4, 15.5, 21.2.2 and 21.2.3 supply the oracles. Every positive source runs
through the public CLI in both optimizer modes and on the legacy and compact
value backends; three also run after snapshot/Db destruction at native O0/O3.

A legal zero-time helper with visible writes, persistent static state or
descriptor-array formals cannot be a read-only runtime callback. These
fixtures cover the contexts the runtime used to evaluate only as callbacks:

- `force_sources` and `force_lifecycle`: a force whose RHS calls such a helper
  takes effect at the force statement and is re-evaluated while it is in
  effect, like a continuous assignment (§10.6). Release or replacement stops
  re-evaluation (`cnt=0` and `other_cnt=0` after an operand change are exact).
  Targets cover variables, a net, a concatenation, a real, a static task's
  force and a descriptor array.
- `intra_assignments`: blocking `a = @(e) b` and `repeat` forms wait in their
  process; nonblocking `a <= @(e) b` and `->> @(e) ev` arm at issue and wait
  in a detached process, so the issuer is not blocked, a change the issuer
  makes right after the statement is an event, the destination selector is
  captured at issue, and `wait fork`/`disable fork` neither wait for nor
  cancel the pending update.
- `mixed_events`: named events and real-valued helpers in the same list as
  process-evaluated helpers; a trigger before the control is reached is not
  observed (§15.5.2), a named event filtered by an effectful `iff`, and an
  unchanged real value is no event.
- `postponed_helpers`: `$strobe`/`$monitor` arguments whose helpers keep their
  own static state (local, result) or take a descriptor array.

The language leaves the number of helper evaluations open. Counters are
therefore checked as lower bounds, and helpers whose results depend on their
evaluation count appear only where the count is fixed by the context
(`$strobe` evaluates once at the end of its time step).

Negatives: a `$strobe` helper that writes a variable it does not own
(language: writes are illegal in Postponed, §4.4.2.9), an automatic variable
in a nonblocking event control (language, §13.3.2), and a named event in the
same list as a helper that reads unpacked-array storage (unsupported boundary,
not a language rule).

All `.out` files are hand-derived from the clauses above.
