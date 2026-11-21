# KI timescale option: `--timescale` and `missing-timescale`

Decision F2-D3 (user decision of 2026-11-19): design elements that declare no
time scale take the default 1ns/1ps; `--timescale <unit>/<precision>` (or
`compile.timescale` in the `--config` file, which the command line replaces)
chooses another default, and the `missing-timescale` lint rule warns when the
built-in default applies. Decision F2-D4: a design in which only some
elements declare a time scale is an error (`ELAB-006`) unless `--timescale` or
`compile.timescale` gives the others one. The suite is
`tests/sim_feature_completion/ki_timescale_option.rs`.

## Clauses

IEEE 1800-2009, `SystemVerilog-1800-2009.txt`:

- 3.14.2.3 L2946, L2954: "If a timeunit is not specified within a module,
  program, package, or interface definition, then the time unit shall be
  determined using the following rules of precedence:" ... "d) Else, the
  default time unit shall be used."
- 3.14.2.3 L2962: "The default time unit and precision are
  implementation-specific."
- 3.14.2.3 L2964: "It shall be an error if some design elements have a time
  unit and precision specified and others do not." (F2-D4: an explicit
  default time scale gives the others one.)
- 22.7 L39668-39669: "If there is no `timescale specified or it has been reset
  by a `resetall directive, the default time unit and precision are
  tool-specific."
- 22.7 L39696-39698: "The integers in these arguments specify an order of
  magnitude for the size of the value; the valid integers are 1, 10, and 100.
  The character strings represent units of measurement; the valid character
  strings are s, ms, us, ns, ps, and fs." (the `--timescale` syntax)
- 3.14 L2848-2849: "The time precision of a design element shall be at least
  as precise as the time unit; it cannot be a longer unit of time than the
  time unit." (a coarser `--timescale` precision is an error)
- 3.14.3 L2978-2980: "The global time precision, also called the simulation
  time unit, is the minimum of all the timeprecision statements, all the time
  precision arguments to timeunit declarations, and the smallest time
  precision argument of all the `timescale compiler directives in the design."
- 20.3.1 L34195-34196: "The value 1.6 is rounded to 2, and 3.2 is rounded to 3
  because the $time system function returns an integer."
- 20.4.1 L34266-34267: "The timescale information shall appear in the
  following format: Time scale of (module_name) is unit / precision"
- 20.4.2 Table 20-3 L34373-34381: `units_number` "The smallest time precision
  argument of all the `timescale compiler directives in the source
  description", `precision_number` 0, `suffix_string` "A null character
  string", `minimum_field_width` 20.
- 21.7 L38206: "The $timescale keyword specifies what timescale was used for
  the simulation."
- 37.10 note 7 L57958-57959: "Passing a NULL handle to vpi_get() with
  properties vpiTimePrecision or vpiTimeUnit shall return the smallest time
  precision of all modules in the instantiated design."

## `consumers.sv`

No element declares a time scale, so `p`, `sub` and `tb` take the default and
the global precision is the default precision (3.14.3). `$timeformat` starts
at that precision with 0 digits, no suffix and width 20 (Table 20-3).

Default 1ns/1ps (`consumers.out`), ticks of 1ps:

- `$printtimescale` and `$printtimescale(u)`: `Time scale of (tb) is 1ns / 1ps`,
  `Time scale of (tb.u) is 1ns / 1ps`.
- `%t` of 1 (one 1ns unit) = 1000ps, width 20: `tf0=                1000|`.
- `#1` at 1ns: `%0t` of `$time` (1) = `1000`, `time=1`, `real=1.000000`.
- `sub` at `#2` = 2ns: `t=2000`, `T=` 2000 in 20 columns, `time=2`.
- `#1.6` more = 2.6ns: `$time` rounds 2.6 to 3 (20.3.1), so `%0t` prints 3000;
  `real=2.600000`.
- the package task's `#1` is one package unit (1ns): 3.6ns; `%0t` of
  `$realtime` 3.6 = 3600ps, `real=3.600000`.
- `$timeformat(-9, 1, " ns", 12)`: 3.6ns = `3.6 ns`, right-justified in 12:
  `tf=      3.6 ns|`.

`--timescale 10us/1ns` (`consumers_10us.out`), ticks of 1ns, one unit = 10us =
10000ns:

- `Time scale of (tb) is 10us / 1ns`, `Time scale of (tb.u) is 10us / 1ns`.
- `%t` of 1 = 10000ns: `tf0=               10000|`.
- 10us: `t=10000 time=1 real=1.000000`; `sub` at 20us: `t=20000`, `time=2`.
- 26us: `$time` = 3 (2.6 rounded), `%0t` of 3 = 30000: `t=30000 time=3 real=2.600000`.
- package `#1` = 10us more, 36us: `pkg t=36000 real=3.600000`.
- `$timeformat(-9, 1, " ns", 12)`: 36us = `36000.0 ns` in 12: `tf=  36000.0 ns|`.

The same lines come from `compile.timescale = "10us/1ns"`, and with
`--timescale 1ns/1ps` given beside that config the default output returns
(command line over config). `--timescale "10 us / 1 ns"` is the same value:
a `timescale directive allows blanks between the parts (Syntax 22-6).

## `declared.sv`, `mixed.sv`

`declared.sv`: a `timescale 1ns/1ps covers `pkg` and `sub`, `tb` declares
`timeunit 1us; timeprecision 1ns;`. Global precision 1ps. `sub` at 1ns:
`%t` of `$realtime` 1.0 in 1ps = `sub t=1000`; `tb` at 2us = 2000000ps:
`tb t=2000000 W=4`. No element takes the default, so `--timescale 10us/1ns`
changes nothing (`declared.out` for both).

`mixed.sv`: `pkg` and `sub` precede the `timescale 1us/1ns of `tb`, so only
some elements have a time scale: without a default time scale the run stops
before lint with one error at `pkg`, the first element without one, naming
`tb`, which has one (3.14.2.3 L2964, F2-D4):
``ERROR mixed.sv:2:9 package `pkg` has no `timescale, timeunit or
timeprecision, but module `tb` at <path>/mixed.sv:11:8 has a time scale; ...``
(`ELAB-006`, exit 1, nothing printed). `--timescale 1ns/1ps`: `sub` is
1ns/1ps, so the global precision is 1ps and the lines are those of
`declared.sv` (`mixed.out`). With `--timescale 1us/1ns` (or
`compile.timescale = "1us/1ns"`) every element is 1us/1ns, global precision
1ns: `sub` at 1us = `sub t=1000`, `tb` at 2us = `tb t=2000 W=4`
(`mixed_1us.out`). Neither prints a `missing-timescale` warning.

## `wave.sv`

`a` rises at `#1`. Default: `$timescale 1ps $end` (the global precision, 21.7)
and the rise at `#1000`. `--timescale 10us/1ns`: `$timescale 1ns $end` and the
rise at `#10000`.

## `vpi_time.sv`, `vpi_time.c`

At `#1` the application prints `vpi_get(vpiTimeUnit/vpiTimePrecision)` of
`tb` and of NULL (37.10 note 7: the smallest precision), the simulation time
in ticks and `tb`'s scaled real time. llg reports a module's
`vpiTimePrecision` as the global precision. Default: `tb unit=-9
precision=-12`, `simulation unit=-12 precision=-12`, `sim time=1000`,
`tb time=1.000`. `--timescale 10us/1ns`: `tb unit=-5 precision=-9`,
`simulation unit=-9 precision=-9`, `sim time=10000`, `tb time=1.000`.

## `missing-timescale` warnings

One warning per run, at the name of the first element in source order that
takes the built-in default, counting the others:

- `consumers.sv:3:9`: "package `p` and 2 other design elements have no
  `timescale, timeunit or timeprecision and use the default time scale
  1ns/1ps (missing-timescale)".
- `wave.sv:2:8`: "module `tb` has no `timescale, timeunit or timeprecision
  and uses the default time scale 1ns/1ps (missing-timescale)".
- none for `declared.sv`, and none whenever `--timescale` is given.

`-Werror` reports it as an error (`ERROR consumers.sv:3:9 ...`), which stops
the run before a model is built (exit 1); with `--timescale 1ns/1ps` the same
run succeeds and prints `consumers.out`. `[lint.rules.missing-timescale] enabled = false` in the config
removes it, so `-Werror` then builds and runs `consumers.sv` unchanged.

`--timescale` values that are not `<1|10|100><unit>/<1|10|100><unit>`
(`1ns`, `2ns/1ps`, `1ns/1xs`) or whose precision is coarser than the unit
(`1ps/1ns`) are usage errors (`CLI-003`, exit 2) naming the option.
