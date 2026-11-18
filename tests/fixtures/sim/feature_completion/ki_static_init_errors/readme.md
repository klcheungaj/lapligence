# Run-time errors in static declaration initializers

Static declaration initializers run before any procedure, outside every
process. A run-time error, `$finish` or `$fatal` there is the same event as in
a process:

- one that ends the run stops the initializer at once; no later initializer
  and no procedure runs, and the run ends through its normal end: final
  procedures (except after an error that skips them everywhere, such as an
  index-check stop, S07-D15), the message summary and the exit status;
- any other reports, and initialization and the run go on.

The task module runs each fixture through the public CLI in both HDL
optimizer modes and checks stdout, the exit status and the run-time messages.
Expected stdout was checked against an independent industry-standard
simulator, which matches every row except `coverage_option` (see its row).
None of the fixtures has a final procedure: whether one runs after a run that
a static initializer ends is decision S22-D9. Clause lines are from
`SystemVerilog-1800-2009.txt`.

Common clauses:

- 6.8 L5286-5287: "Setting the initial value of a static variable as part of
  the variable declaration (including static class members) shall occur
  before any initial or always procedures are started".
- 20.2 L34119: "The $finish system task causes the simulator to exit and pass
  control back to the host operating system."
- 20.9 L34909-34913: "$fatal shall generate a run-time fatal error, which
  terminates the simulation with an error code. ... Calling $fatal results in
  an implicit call to $finish. — $error shall be a run-time error."

| Fixture | Clause and oracle |
| --- | --- |
| `coverage_option` | 19.7 Table 19-1 L33254-33260: weight's default is 1, and "The specified weight shall be a non-negative integral value." `g a = new(-1)` sets `option.weight` to -1 in the constructor a module variable's initializer runs: the run-time error located at `option.weight = w` (line 11), the option keeps its default, and, as for the same construction in a process (`sim_045` `option_errors`), the run goes on: `init first`, `init last`, then the initial procedure prints `first=1 last=1 weight=1`. Exit status 1 for the error. The industry-standard simulator ends the run at this option error in every context (in a process, after its final procedures); llg reports it and continues everywhere, so this row pins llg's current behaviour, which follows the process form. |
| `index_check` | 7.10.2.2 L9090: "The insert() method inserts the given item at the specified index position." S07-D15: an `insert` with an index outside the queue stops the run under the default index checks. `q.insert(5, 3)` on a one-element queue, in a function the initializer of `size` calls: `init first`, then the stop; neither the rest of the function, `last`'s initializer nor the initial procedure runs. Exit status 1. |
| `null_access` | 8.4 L9653-9654: "Accessing nonstatic members (see 8.8) or virtual methods (see 8.19) via a null object handle is illegal. The result of an illegal access via a null object is indeterminate, and implementations may issue an error." The constructor that `holder h = new` runs reads `l.x` with `l` null: `init first`, `holder new`, then the null-access error that ends the run (as in a process); the rest of the constructor, `last`'s initializer and the initial procedure do not run. Exit status 1. |
| `error_task` | 20.9 L34913 (above). 6.8 L5286-5287 includes static class members. `$error` in a static class function that a class static property's initializer and a module variable's initializer call: each reports, the function returns its value and the run goes on: `init first`, `init last`, `initial first=1 v=7 last=1 base=5`, `at 1`. Exit status 0 (ES-D2). |
| `fatal_task` | 20.9 L34909-34912 (above). `$fatal` in the function that the package variable `pv`'s initializer calls; package initializers run before those of the design (S22-D8): `stop called`, then the fatal report and its `$finish`; `after fatal`, `pw`'s initializer, the module's initializer and the initial procedure do not run. Exit status 1 (ES-D1). |
| `finish_task` | 20.2 L34119 (above). `$finish` in the function called by the second element of an unpacked array pattern initializer (a statement-shaped initializer): `init pattern`, `stop called`, then the `$finish` report; `after finish`, `last`'s initializer and the initial procedure do not run. Exit status 0. |
