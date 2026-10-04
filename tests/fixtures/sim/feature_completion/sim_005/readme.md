# Real references, real arrays and real sampling (SIM-005)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes. The five feature fixtures also run on legacy,
compact/portable and compact/GMP backends (GMP when `LLG_TEST_GMP_ROOT` is set)
and, except the numeric-extremes fixture, after the frontend snapshot and owned
Db are destroyed; the extremes fixture also runs at model `-O0` and `-O3`.
Expected outputs are derived by hand from the cited clauses, not captured from
llg.

Reals keep numeric semantics throughout: a real `ref` formal is the actual's
`double` cell, real arrays are `double` cells inside and across subroutines,
and comparisons use real `==`, never a bit pattern. Shortreal storage rounds
to the nearest IEEE single.

| Fixture | Clause and independent oracle |
| --- | --- |
| `real_refs` | IEEE 1800-2009 §§13.5.2, 6.12. `add(r,0.5)` gives 1.25+0.5=1.75. Shortreal 0.1 is 0.100000001490116; times 3.0 is 0.30000000447, nearest single `3e99999a`. Forwarding through `forward` adds 1.0 then 0.25 to each element: 2.25 3.25 4.25. `look(a[2])` = 2*4.25+1 = 9.50 through two `const ref` levels. `m[1][2]` = 12 + 0.5 (run-time row index). Index 7 is out of range, so the write goes nowhere and `a` is unchanged. Record members: 0.5+1.0 = 1.50, `k` stays 3, shortreal 1.0*3 = 3.0 = `40400000`. `swap(lo,hi)` gives 2.0 1.0; `swap(lo,lo)` writes `lo` twice and leaves 2.0. Recursion: each activation adds k*0.5 for k=4..0, total 5.00. At time 2 two timed activations run in a fork: `r` = 1.75+3*0.5 = 3.25 and `a[1]` = 0-3*1.5 = -4.50; `@(a[1])` sees the time-1 write plus three reference writes: 4. |
| `real_array_calls` | §§7.6, 13.3-13.5, 11.4.11, 7.4.6. `scale` doubles 1.5 2.5 3.5 into 3 5 7. `total` sums 7.50 and 1+2+4 = 7.00 and its write to its input copy leaves `a[0]` = 1.50. `inout` doubles to 6 10 14; `ref` negates the middle: -10. Equality is element-wise numeric: 1 1 0. Known predicates select whole arms; the X predicate keeps equal elements 1.5 and 3.5 and yields 0.0 for 2.5 vs 9.0. `transpose` of {{1,2},{3,4}} is {{1,3},{2,4}}; row `m[1]` sums 7.00; a nested call result feeds an input formal and `transpose(transpose(n))` equals `n` (1, `m[0][1]` = 3.0). Static `mean` keeps its local array: 1/1, 3/2, 9/3. Shortreal `base/3*(i+1)` rounds to `3eaaaaab` `3f2aaaab` `3f800000`. `a = scale(a, 0.5)` reads before writing: 0.750 1.250 1.750. The timed output copies 7 8 9 back at time 2. |
| `real_array_methods` | §§7.12.1, 7.12.2. Stable numeric `sort`/`rsort`/`reverse` of {1.5,-2,4.25,0.5,-2}; the three in-place changes of `a[0]` in one time step wake its waiter once. Shortreal 0.1/0.2/0.3 sort to `3dcccccd` `3e4ccccd` `3e99999a`. `reverse` moves rows whole, then reverses row 1. A NaN keeps its position and the keys on each side sort separately: {1,3,NaN,-1,2}. Over {3,1,2,1,-0.0,0.0}: `find` x>1.5 gives 3 2; `find_index` x<2.5 gives indices 1..5; `find_first` x<2 is 1.0; `find_last_index` x==1 is 3; `min` is the first minimum -0.0 (`8000000000000000`, since 0.0 < -0.0 is false); `max` 3.0; `unique` keeps 3,1,2,-0.0 (indices 0 1 2 4, -0.0 == 0.0). Stable `sort` gives -0.0 0.0 1 1 2 3; `rsort` then `reverse` gives 0.0 ... 3.0. A dynamic array sorts to -1 0.25 0.5 2 and `find` with `e.index != 1` gives 0.50 and 2.0. With NaNs, `min` skips them (-3.0) and every NaN is unique (4). `max` of an empty queue is empty. |
| `real_sampled` | §16.9.3. Procedural `$sampled` returns the Preponed values 1.25 and shortreal 0.5 (`3f000000`). On posedges at 5..55 the samples are 2.5, 2.5, 0.0, -0.0, NaN, NaN. `$past` before the first tick is the registration value 1.25. `$stable` compares with real `==`: 2.5/1.25 0, 2.5/2.5 1, 0.0/2.5 0, -0.0/0.0 1, NaN/-0.0 0, NaN/NaN 0. |
| `real_numeric_extremes` | §§6.12.2, 20.5. Conversion to integral rounds half away from zero and keeps the low target bits: 1e20 mod 2^32 = 1661992960, mod 2^64 = 7766279631452241920, 301 mod 256 = 45; -0.5 -1, 0.5 1, 1.5 2, -2.5 -3. `$rtoi` truncates: -1, 2. Policy: a nonfinite real converted to an integral type yields 0 (the standard leaves it unspecified). Shortreal: 2^24+1 rounds to 16777216, 1e40 overflows to +inf `7f800000`, 1e-45 rounds to the smallest subnormal `00000001`, -0.0 is `80000000`. The largest double times 2 is +inf; the smallest subnormal halves to +0 (ties to even). -0.0 == 0.0. NaN compares unequal and unordered; a payload NaN is still NaN. |
| `real_ref`, `real_array_value`, `real_sample` | Adopted FND-002 witnesses with quiet `$finish(0)`: 1.75, `1.25 2.50`, 1.25. |

## Negatives

| Fixture | Boundary |
| --- | --- |
| `neg_real_bitwise`, `neg_real_concat`, `neg_real_select`, `neg_real_edge` | §§6.12.1, 11.3.1 Table 11-1, 11.4.12, 9.4.2: bitwise operators, concatenation, selects and edge descriptors need integral operands (frontend). Adopted FND-002 `neg_real_bitwise`. |
| `neg_real_case_equality`, `neg_real_array_case_equality` | Table 11-1: case equality is not a real operator, also over real arrays (llg; the frontend accepts the syntax). |
| `neg_real_rose` | §16.9.3: `$rose`/`$fell` read a least significant bit, which a real lacks. |
| `neg_automatic_nba`, `neg_ref_nba` | §§6.21, 10.4.2, 13.5.2: no NBA to automatic variables or ref formals (adopted FND-002 witnesses). |
| `neg_real_map` | §7.12.3: reduction methods need integral array elements; the adopted FND-002 `real_map` witness is frontend-illegal. |
| `neg_real_queue_ref` | Legal by §13.5.2 (FND-002 `real_queue_ref`); a ref to a real queue element needs a retained element cell that survives reallocation. That pinning belongs to SIM-008 and is rejected explicitly. |
