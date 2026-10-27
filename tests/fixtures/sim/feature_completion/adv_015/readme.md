# ADV-015: checker execution and checker bind

These public HDL fixtures cover checkers (IEEE 1800-2009 Clause 17) and
checkers injected by `bind` (23.11) through
[`sim_feature_completion::adv_015`](../../../../sim_feature_completion/adv_015.rs).
Positive cases run in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends. Action lines of different assertions in one time
step and final procedures of different checker instances have no defined
order, so each `.out` file is compared as a set of lines. Every fixture
declares `` `timescale 1ns / 1ns ``. Every expected line was derived by hand
from the stimulus and the clauses quoted below, not copied from a simulator
transcript. Line numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt`. The portable decision cases `A15-D1` to
`A15-D3` live in `tests/fixtures/sim/lrm_decisions/`.

Run the suite with:

```sh
scripts/run-tests.sh -E 'binary(sim_feature_completion) & test(adv_015::)'
```

| Fixture | Covers |
| --- | --- |
| `sampling.sv` | procedures and action blocks in Reactive, sampled formals, Re-NBA updates (A01) |
| `nested.sv` | `$inferred_clock`/`$inferred_disable`, nested checker with an inferred clock, let/function/sequence, cover, deferred assertion item, final (A01) |
| `bind.sv` | checker bound into a module, interface port actuals, covergroup, subchecker (A01) |
| `boundary.sv` | disjoint bits of one variable from two procedures, named block, two instances, unused checker (A01) |
| `neg_*.sv` | 17.5 and 17.7.1 restrictions, later-revision forms, located limits (A03) |

## Clauses the expectations rely on

Sampled formals and directly substituted events (17.3, L28294-28297,
L28308-28310):

> each formal argument shall be assigned the sampled value of its actual argument during the Preponed region of
> each time step, with the following exceptions and clarifications:

> —      Arguments that cannot be sampled, such as events, sequences, and properties, are treated similarly
>        to such arguments for sequences and properties (see 16.8): they are substituted directly for the
>        formal argument when it is used in statements or expressions within the checker.

Nested checkers (17.3, L28322-28323):

> —      If the checker is instantiated within another checker, then all formal arguments are considered to be
>        directly connected to their actual arguments, as in a module instantiation.

Scheduling (17.7.3, L28987-28989):

> Statements and constructs within a checker that are sensitive to changes (e.g., clocking events) are scheduled
> in the Reactive region (similarly to programs, see 24.3.1). The nonblocking assignments of checker vari-
> ables schedule their updates is the Re-NBA region.

Inferred context (17.4 example, L28488-28491):

> In the above example the default values of clock and reset in check_in_context are taken from the
> instantiation context. In the instantiation my_check1 all formal arguments are provided explicitly. In the
> instantiation my_check2 all optional arguments are passed their default value: the clock is inferred from the
> clock of the always procedure of the module m, the disable condition is inferred to be 1'b0.

Checker items (17.2, L28181-28184):

> —    Covergroup declarations and instances
> —    Checker variable declarations and assignments (see 17.7)
> —    default clocking and default disable iff declarations
> —    initial, always and final procedures (see 9.2)

Deferred assertions (17.3.1, L28339-28340), decision A15-D2:

> Immediate assertions, including deferred assertions, are handled normally as described in
> 16.3 and 16.4.

Checker procedures (17.5, L28501-28506), `neg_blocking`, `neg_if`,
`neg_delay`, `neg_immediate`, `neg_initial_nba`:

> An initial procedure in a checker body may contain deferred and concurrent assertions and a procedural
> timing control statement using an event control only.
>
> An always procedure in a checker body may contain deferred and concurrent assertions, nonblocking vari-
> able assignments (see 17.7.1) and a procedural timing control statement using an event control. All other
> statements shall not appear inside an always procedure.

Checker variable assignments (17.7.1, L28790-28791, L28826-28827,
L28860), `neg_blocking`, `neg_sar`, `neg_select`, `boundary`:

> Checker variables may be assigned using nonblocking procedural assignment only. Blocking procedural
> assignments to checker variables are not allowed.

> —      Single Assignment Rule (SAR): it shall be illegal to use the same bit of a checker variable in several
>        assignment-like contexts.

> —      The left hand side of an assignment shall be the longest static prefix of a select (see 11.5.3).

`neg_foreign` assigns a module variable from a checker procedure; 17.7.1
allows nonblocking assignments of checker variables only (L28790-28791).

`neg_always_ff` and `neg_output` use checker forms that IEEE 1800-2009 does
not define (17.2 L28109-28110: a `checker_port_item` has no direction; 17.5
L28496-28499 allows only `initial`, `always` and `final` procedures) and are
edition errors.

Located limits (A02): free variables (17.7, L28630-28631) and procedural
checker instances (17.3.1, L28345-28347) are rejected at their location; see
the [known issue](../../../../../docs/known_issues.md#checker-limits-after-adv-015).

> Checker variables may have an optional rand qualifier. In this case, they are called free variables; free vari-
> ables may behave non-deterministically.
