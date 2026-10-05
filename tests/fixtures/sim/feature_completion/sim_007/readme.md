# Native aggregates, tagged values and pattern expressions (SIM-007)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set), and after the frontend snapshot and owned Db are
destroyed. Expected outputs are derived by hand from the cited clauses, not
captured from llg.

A one-dimensional fixed unpacked array whose elements are strings, identity
handles or records with string, real or handle members is stored as a fixed
array view of the recursive container runtime: each element is one owned
value, a runtime select reads or writes it in place, and copies are deep
except for handles (SV 7.4, 7.6, 8.4).

| Fixture | Clause and independent oracle |
| --- | --- |
| `native_arrays` | IEEE 1800-2009 §§7.4, 7.4.6, 7.6, 10.4.2, 10.9, 11.4.5, 11.4.11, 13.4. (1) Elements start as empty strings; `$size` is 3. (2) Positional patterns fill in declaration order: `up[1..3]` = a b c, `dn[3..1]` = x y z. (3-6) `always_comb` follows the runtime select `up[k]`: a, then c for k = 3, then C after `up[3] = "C"`, then the empty string for the out-of-range k = 7 (§7.4.6, no warning). (7, 8) Index keys name declared indices and `default` fills the rest: `d two d`; `three ? one`. (9) `'{3{"r"}}` replicates: rrr. (10) The reversing pattern reads every source before writing: rqp. (11) `dn = up` copies by position (dn[3] = up[1]) and stays independent of a later write: `rqp R`. (12) `cp == dn` compares element by element in storage order (r q p both): 1, 0; `cp === up` sees `r` against `R`: 0, 1. (13) `cp[0:1] = dn[2:1]` copies q p: qpp. (14) `cp[k+:2]` with k = 1 takes `up[1:2]` = R q: qRq. (15, 16) A known condition selects one arm: ttt, then `up` = Rqp. (17) An unknown condition keeps equal elements and gives the empty string elsewhere: `[R][][p]`. (18-20) Calls copy the array in and out: `R--p`; `fill` writes z, zz, zzz; a pattern actual is built before the display: `m-n-o`. (21) `rb = ra` copies the records; renaming `rb[0].s` leaves `ra[0]`: `one uno 2 2.5`. (22) `ra == rb` is 0 (one string differs), `!=` 1. (23) `'{default: rec_t'{...}}` fills both records: `9 nine 9 nine`. (24) `swap_pick(ra, 0)` returns `ra[1]` through its swapped local copy: `2 two 2.5`. (25) Nonblocking writes use issue-time values: `up[1]` becomes N1, `up[2]` the old `up[1]` (R), and `rb <= ra` copies record 0 (`1 one`). |
| `handle_records` | §§7.2, 8.4, 13.4, 13.5. Copying a record copies its class handle, so `bump` changes the shared object: `a a+ 11 11 1`. A record element of a fixed array is passed by value and the result stored in another element: `a++ 12 0.25`. An `inout` record formal copies back into an element: `z a+`. Rebinding `b.h` leaves `a.h` at 12; a never-set element handle is null: `12 1 1`. Assigning `a` into `list[0]` shares the object, so writing 99 through the element is seen through `a.h`: 99. |
