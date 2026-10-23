# dense_cell_runs fixtures

Whole unpacked-array values whose reads and writes cover runs of consecutive
dense cells (`tests/sim_dense_cell_runs.rs`). The emitter reads or writes such
a run with one runtime range call instead of one access per element; these
fixtures check that the element semantics are unchanged and that the
generated C does not grow with the element count. Expected values are
computed by hand from the clauses below.

- IEEE 1800-2009 7.6 (`SystemVerilog-1800-2009.txt` L8436-8439): "Assignment
  shall be done by assigning each element of the source array to the
  corresponding element of the target array. Correspondence between elements
  is determined by the left-to-right order of elements in each array. For
  example, if array A is declared as int A[7:0] and array B is declared as int
  B[1:8], the assignment A = B; will assign element B[1] to element A[7], and
  so on."
- 7.4.3 (L8091-8095): reading and writing the whole array, a slice
  (`A[i:j] = B[i:j]`) and equality on the array (`A==B`).
- 11.4.5 (L14794-14795): "if, due to unknown or high-impedance bits in the
  operands, the relation is ambiguous, then the result shall be a 1-bit
  unknown value (x)"; L14798: for `===` "Bits that are x or z shall be
  included in the comparison and shall match".
- 11.4.11 (L15213-15216): with an ambiguous condition "both first expression
  and second expression shall be evaluated, and their results shall be
  combined element by element. If the elements match, the element is
  returned. If they do not match, then the default-uninitialized value for
  that element's type shall be returned." Record elements merge leaf by leaf
  (decision R05-D1).

`whole_values.sv`, line by line:

- `A`: `int A[10:1] = B[0:9]` with `B[k] = 3k`: `A[10]` takes `B[0]` (0),
  `A[5]` takes `B[5]` (15) and `A[1]` takes `B[9]` (27).
- `cond`: `ra[k] = {k, 10k}`, `rb` a copy with `rb[5].i = 1`; an `x`
  condition keeps every equal leaf and gives the differing `int` leaf its
  default 0: `rc[5] = {5, 0}`, `rc[4] = {4, 40}`.
- `pick`: a 0 condition selects `rb`: `rc[5].i = 1`, `rc[6].i = 60`.
- `rb`: a whole copy keeps the `x01z` leaf of `ra[3].a`.
- `eq`: the copies differ only in X/Z bits they share, so `==` and `!=` are
  `x`, `===` is 1 and `!==` is 0.
- `ne`: after `rb[5].i = 1` the known mismatch makes `==` 0, and `===` 0.
- `out`: an output formal copies the whole input and sets `y[0].i = -1`.
- `m2`, `m0`: `m[i][j] = 16i + j`; `m2 = m[1:2]` copies rows 1 and 2
  (`m2[0][0] = 'h10`, `m2[0][4] = 'h14`, `m2[1][4] = 'h24`), and
  `m[0] = m[3]` copies one row (`'h30`, `'h34`), leaving row 1 (`'h10`).
- `overlap`: `src = 1..8`, then `src[1:6] = src[0:5]` reads every source
  element before writing, giving `1, 1, 2, 3, 4, 5, 6, 8`.
- `cont`: `assign w = src` follows the overlapped value one step later.
- `inout`, `func`: an inout formal adds 1 to every element; a function
  returns the elements reversed.
- `fill`, `default`, `records`: `'{default: 'h5a}` at declaration,
  `'{default: 'h33}` with element 2 then set to `'h44` (68), and a
  type-keyed `'{rec_t: '{'hc, 7}}` fill.
- `bits`: a two-state array copy (`100..105`) compares equal (1).
- `short`: three-element (below the four-cell run threshold) and four-element
  (at it) copies.
- `nba-before`, `nba-after`: `rn <= ra` at time 10 leaves the default 0 until
  the NBA region, then shows `ra[6].i` (60) and `ra[3].a` (`x01z`).

`whole_read_4096.sv`: 4096-record and 4096-byte arrays copied whole, merged
under an `x` condition, passed through a task, compared and formatted with
`%p`. `rb[4095].i = 4095`, `rc[17] = {1, 17}` (4 bits of 17), `copy[4095]` is
255, both comparisons are 1, and the `%p` text has 18,721 characters: the
values `k mod 256` take 16 * (10 + 90 * 2 + 156 * 3) = 10,528 digits, the 4,095
separators `, ` take 8,190 and the braces `'{` and `}` take 3.

`size_mismatch.sv`: nearest illegal form, a whole copy between arrays of 10
and 24 elements (7.6: "the source array shall have the same number of
elements as the target", L8425-8426), rejected by the frontend.
