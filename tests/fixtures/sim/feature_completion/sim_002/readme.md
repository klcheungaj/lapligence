# SIM-002 time-reporting scope and formatting controls

IEEE 1800-2009 §§3.14.2.3, 20.3, 20.4 and 21.2.1.3 and IEEE 1364-2001
§§17.3 and 17.7 supply the oracles; every expectation is computed by hand
from those clauses. Executed sources run through the public CLI in both
optimizer modes on the legacy and compact (portable and, with
`LLG_TEST_GMP_ROOT`, GMP) value backends; `.v` fixtures also run with
`--edition 2001`. Executed fixtures, including the adopted positive FND-002
witnesses, end with `$finish(0)` so no diagnostic reaches stderr; the witnesses
are otherwise unchanged.

Project contracts these fixtures fix:

- `$printtimescale` prints `Time scale of (<name>) is <unit> / <precision>`
  (the §20.4.1 format). `<name>` is the hierarchical instance path, a package
  name, or `$unit`. Without an operand it names the module, package or
  `$unit` that contains the call.
- `$time`, `$stime`, `$realtime`, `%t` and delays use the unit and precision
  of the scope that contains them: a module, interface or program instance,
  a package, `$unit`, or the scope that declares a class. The calling scope
  never changes them.
- The design tick, which is also the default `$timeformat` unit, is the
  finest precision of every instantiated module and of every package or
  `$unit` that declares subroutines or classes.

Fixtures:

- `printtimescale_scope`: named child, instance array element, nested
  generate instance, interface, program and `$unit` operands, and no-operand
  calls from the caller, a child task, an interface task, a package function
  and a `$unit` function, each at a different time scale.
- `printtimescale_v2001.v`: the 1364-2001 §17.3.1 example shape, with the
  printed instance nested one module deeper.
- `printtimescale_child_scope.v`: adopted FND-002 witness
  `printtimescale_scope`.
- `time_scopes`: integer, short and real time and `%t` in modules,
  interfaces, packages, `$unit` and package/module classes; a package
  function called from two scopes; a `time` value passed into a package;
  delays inside package and child tasks.
- `time_scopes_v2001.v`: module-only rules in 1364-2001, including
  hierarchical task calls, rounding a 22.5ns delay to 1ns precision, and
  `$time` rounding an exact half upward.
- `timeformat_controls`: Table 20-3 defaults, four literal and four variable
  arguments, a suffix variable changed after the call, repeated calls, a
  `$timeformat` in a package function, other-module and package `%t`
  conversion, real and negative real `%t` values, exact and too-narrow
  widths, the zero-argument reset and `$timeformat()`.
- `timeformat_four_args.v`, `timeformat_zero_args.v`,
  `timeformat_reset_defaults.v`: adopted FND-002 witnesses `timeformat_four`,
  `timeformat_zero` and `timeformat_reset`.
- `timeformat_boundary`: time 0 and 2^64-1 fs in seconds, milliseconds and
  femtoseconds; `$stime` keeps the low 32 bits; a 1s module's 18447 seconds
  formats as 18447000000000000000 fs, past 64 bits.
- `timeformat_schedule`: format changes between scheduling and firing never
  move a delayed NBA, a forked delay, or a delay entered after the change.
- Negatives: `neg_timeformat_one_arg.v`, `neg_timeformat_two_args.v`,
  `neg_timeformat_three_args.v`, `neg_timeformat_five_args.v` (adopted
  FND-002 extension probes outside the zero-or-four grammar);
  `neg_timeformat_suffix_type` and `neg_timeformat_units_string` (argument
  types); `neg_printtimescale_generate`, `neg_printtimescale_expression` and
  `neg_printtimescale_two_args` (operand is not a module instance, or too
  many operands); `neg_printtimescale_root` (`$root` is rejected rather than
  given an invented time scale); `neg_timeformat_units_range` (a units value
  outside 0 through -15 fails when the call runs and leaves the earlier format
  in place).
