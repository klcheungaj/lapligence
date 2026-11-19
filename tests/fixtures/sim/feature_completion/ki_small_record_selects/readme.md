# Selections of records held as one signal per leaf

A record of integral members whose member arrays have fewer than 16 cells
keeps one signal per leaf ([record storage by size](../../../../../docs/sim_performance_tradeoffs.md#record-storage-by-size)).
A constant element, member or part selection of it reads and writes the one
leaf it lies in, and a run-time element index compares the index with each
declared index, as for records with a `string` member. These fixtures check
the values; `leaf_selects_shape` also checks that `model.c` joins no leaves
and builds no composite reference for such selections, and the
`member_name_alias` pair checks that a subroutine's own array is never taken
for a record member of the same name. Every `.out` file is
derived by hand below; none were captured from `llg`.

Clauses (`docs/specification/SystemVerilog-1800-2009.txt`):

- 7.4.6, lines 8236-8237: "be invalid. The result of reading from an array
  with an invalid index shall return the default uninitialized" / "value for
  the array element type. Writing to an array with an invalid index shall
  perform no operation."
- 6.8, Table 6-7, lines 5313 and 5315: "4-state integral 'X" and "2-state
  integral '0".
- 13.5.2, line 19139: "of the argument; therefore, any changes made to the
  argument, within either the caller or the subroutine,".
- 11.2.2, line 14175: "Aggregate expressions can also be compared with
  equality or inequality operators."
- 9.4.2, line 11768: "occurrence of a declared event. The value changes on nets
  and variables can be used as events to trigger".
- 10.6.2, lines 13388-13393: "A force statement to a variable shall override a
  procedural assignment, continuous assignment or an" ... "change value and
  shall maintain its current value until the next procedural assignment to the
  variable is" / "executed."

## `leaf_selects`

`r_t` is `{t[7:0], int a[3], pair_t p, logic [7:0] m[2], inner_t n}` with
`pair_t = {hi[3:0], lo[3:0]}` and `inner_t = {pair_t pp[2], int z}`.

- `c`: `t = 5a`, then `t[3:0] = f` gives `5f`. `a = 1 11 21`, then
  `a[1] = 7`. `p = 3c`, then `p.hi = 9` gives `9c`. `m[0] = 01`; `m[1] = 02`
  with bit 7 set is `82`.
- `r`: `t[7:4] = 5`, `p.lo = c`, `m[1][7] = 1`, `m[1][3:0] = 2`.
- `m`: `n.pp[1] = 34` with `lo = 7` is `37`; `n.pp[0] = 12`; `n.z = -3`;
  `n.pp[1].hi = 3`.
- `k`: run-time indices. `a[2] = 21 + 5 = 26`, `a[0] += 1` gives `2`, and
  `v = a[1] = 7`.
- `p`: `n.pp[0].hi = f` gives `f2`; `m[0] = 01 + 10 = 11`, whose `[7:4]` is
  `1`.
- `f` (time 1): `bump` adds 100 through a `ref` to `a[0]` (constant) and to
  `a[2]` (run-time index): `102` and `126` (13.5.2).
- `w`, `e`: the whole copy `q = r` holds `a[0] = 102`, `a[2] = 126` and
  compares equal (`1`); after `q.a[1] = 0` it does not (`0`) (11.2.2).
- `a1` (time 2): the waiter armed on `r.a[1]` at time 1 wakes on the run-time
  index write `a[1] = 55` (9.4.2). It then waits on `r.t`.
- `r` (time 3): `t[0] = 0` turns `5f` into `5e` and wakes that wait.
- `z` (time 4): `a[1] = 55`, `t = 5e`.
- `n` (time 6): the NBAs of time 5 set `a[2] = 42`, `a[j] = 43` with `j = 1`,
  and `p.lo = 1`, so `p = 91`; `a[0]` stays `102`.

## `leaf_selects_invalid` (run with `--no-index-checks`)

`r_t` is `{t[7:0], int a[3], logic [7:0] m[2]}` with `a = 1 2 3`,
`m = 44 55` and `t = 5a`.

- `o`: index 3 is outside `[0:2]`: the read gives the `int` default `0` and
  the write of `99` changes nothing (7.4.6, Table 6-7).
- `n`: index -1 is outside `[0:1]`: the read gives the 4-state default `xx`
  and the write of `00` changes nothing.
- `x`: an index of `'x` is invalid for both arrays: `a[u]` reads `0`, `m[u]`
  reads `xx`, and neither the write nor the compound assignment changes a
  cell.
- `t`: `t` stays `5a`.

## `leaf_selects_shape`

`t = 5a` with `t[3:0] = f` is `5f`; `p = 3c` with `p.hi = 9` is `9c`;
`a[2] = 3 + 5 = 8`. `y = a[1] + a[2] + t[7:4] + p.lo = 2 + 8 + 5 + 12 = 27`.

## `leaf_selects_force`

`force r.a[1] = 9` overrides the value `7` (`f 55 1 9 3`) and the later
`r.a[1] = 3`, while `r.a[0] = 5` still assigns (`o 5 9`). After `release`
the element keeps `9` (`r 9`) until `r.a[1] = 4` (`a 4`) (10.6.2).

## `member_name_alias`, `member_name_alias_packed`

The function's and the task's own `cells` are automatic arrays, whatever a
module record's member is called. `function_sum` fills its array with
`10 11 12 13` and returns `0x10 + 0x11 + 0x12 + 0x13 = 70`; `fill(2)` writes
`ee` to its own `cells[2]` and reads it back. The record's `cells` keeps the
values the `initial` block wrote: `01` in each cell of the record with a
`string` member, `02` in each cell of the record of integral members.
