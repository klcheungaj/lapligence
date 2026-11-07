# Record storage forms agree at every size

A record with a member array of `LLG_RECORD_COLUMN_MIN_CELLS` (16) cells or more
is stored as a column; below that each cell is its own leaf
([record storage by size](../../../../../docs/sim_performance_tradeoffs.md#record-storage-by-size)).
The two forms are an implementation choice, so a design must print the same
with either. The fixtures here build one design at `-D N=16` (column) and
`-D N=3` (leaves) and require the same output. Each design is checked in; the
IEEE text gives no result that depends on the size of a member array, so the
oracle is the other form, which the older record fixtures validate against
the clauses. The sweep this grid comes from found seven groups of shapes (a call
into an element, an automatic record with an element, a member array of records
in a column, a conditional with a member array, a call statement with a nested
call, a whole-record nonblocking write with a container member, a default pattern
with a nested record) that lowered per cell and failed at 16 cells; fixtures of
single shapes had missed them.

## Grid

- 7 record types (`gen_record_shapes.py`): `str` (int, string, `int arr[N]`),
  `pk` (packed members, `logic [39:0]`, byte array), `mix` (string, 2-D byte
  array, real, `int few[2]`), `nest` (a nested record and an array of them),
  `rl` (`real arr[N]`), `st` (`string arr[N]`), `obj` (`obj_t arr[N]`, a class).
- 19 sources: module, static and automatic block records, a call result,
  `id(loc)`, queue, dynamic, associative, fixed and nested elements, a class
  property, a member of an element, `q.pop_back()`, both conditionals, an
  assignment pattern and a run-time queue index.
- 22 targets: the same storage forms, `push_back`, an output and a `ref`
  actual, an initializer, `==`, a function argument (two forms) and the
  nonblocking `fa[2] <= S` and `mod <= S`.

That is 2,926 shapes. `rows.txt` lists the 2,356 that
build and print the same at both sizes; 406 do not build at either size (the
limits in `docs/known_issues.md`, such as `x = oq[0].e[1]`, `ref` to a queue of
packed records) and 164 build only at 16 cells (`# ` lines at the end of
`rows.txt`). No shape builds only at 3 cells: the column form has none of the
per-cell form's limits.

The tests run with `--no-index-checks`, so a `ref` actual whose element an
input actual removes (`rf(q[0], q.pop_back())`) gets the IEEE 1800 result at
both sizes instead of the default index-check stop
([S07-D15](../../../../../docs/lrm_decisions.md)).

## Files

- `shapes_<type>.sv`, `shapes_<type>_nba.sv`: the covering subset the default
  suite runs, 157 rows. Every target is used once per type with a rotating
  source, then every source once per type. Each row is its own `initial` block
  (`row K`), which resets the shared containers, sets ten records of the type
  to distinct values and performs one transfer. The `_nba` files hold the
  nonblocking targets; their processes mix `=` and `<=` and are listed in
  `lint/expected_errors.tsv`, which the harness asserts and then reruns with
  the rule disabled.
- `gen_record_shapes.py`: `write` regenerates the checked-in files from
  `rows.txt`; `check` fails when they or the lint manifest differ
  (`component_record_shapes_are_generated`); `full <dir>` writes the whole grid
  (one file per type and source) for `sim_record_shape_grid`; `survey <llg>
  <work dir>` rebuilds `rows.txt` by compiling every shape once at each size.
  Edit the tables in the script and regenerate; do not edit the `.sv` files.

## Tests

`sim_feature_completion/ki_record_shapes.rs`: each record type builds both
files at 16 and 3 cells in both optimizer modes and compares stdout; every run
must exit 0 and print its rows. Time of the 8 tests with a cold model cache:
154 s wall, 520 s summed, on a loaded host (`mix`, `pk` and `nest` take 105-154 s
each); with the model objects cached 21 s wall.

`sim_record_shape_grid.rs` (ignored): the full grid, 248 designs (one per type
and source) at both sizes, in both nonblocking and plain files, with the mixed
assignment rule disabled. It passed in 1,073 s on a host at load 40 with 8 worker
threads. Run it with `scripts/run-tests.sh --test-work-dir /build --test
sim_record_shape_grid --run-ignored only`.

Not covered, by design: shapes that build only at 16 cells have no second form
to compare with. A new transfer shape belongs in the tables of the script; run
`survey` once and regenerate.
