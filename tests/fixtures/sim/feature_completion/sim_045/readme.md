# SIM-045 fixtures: coverpoint bins, transitions, crosses and options

Run by [`tests/sim_feature_completion/sim_045.rs`](../../../../sim_feature_completion/sim_045.rs).
Positive cases run in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends; negative cases run in both optimizer modes. Line
numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt` (SV). The interpretations that needed a choice
are the register decisions
[S45-D1 and later](../../../../../docs/lrm_decisions.md#coverpoint-bins-transitions-crosses-and-options-sim-045).

## Portability of instance coverage

Every covergroup in `bins_count_runtime`, `cross_illegal`, `crosses`,
`illegal`, `illegal_default` and `transition_illegal` sets
`option.per_instance = 1` on its header line (so no other line moves).
Without it, 19.7 (L33285-33289) does not require an implementation to keep
instance-specific information, and some simulators then return 0 from
`get_inst_coverage()`; llg's results are the same either way.

## Clauses relied on

- 19.5, L32409-32411: "The bins for a coverage point can be automatically
  created by SystemVerilog or explicitly defined using the bins construct to
  name each bin. If the bins are not explicitly defined, they are
  automatically created by SystemVerilog."
- 19.5, L32413-32420: "The bins construct allows creating a separate bin for
  each value in the given range list or a single bin for the entire range of
  values. To create a separate bin for each value (an array of bins), the
  square brackets, [], shall follow the bin name. To create a fixed number of
  bins for a set of values, a number can be specified inside the square
  brackets. The open_range_list used to specify the set of values associated
  with a bin shall be constant expressions (see 11.2.1), instance constants
  (for classes only), or non-ref arguments to the coverage group. It shall be
  legal to use the $ primary in an open_value_range of the form
  [ expression : $ ] or [ $ : expression ]."
- 19.5, L32513-32520: "trans_list ::= ( trans_set ) { , ( trans_set ) }",
  "trans_set ::= trans_range_list { => trans_range_list }", "trans_item ::=
  range_list": a transition's values are the same value ranges as a bin's,
  so `transition_arguments` reads non-ref arguments in them as L32413-32420
  allows (industry-standard behaviour accepts it).
- 19.5, L32422-32431: "If a fixed number of bins is specified and that number
  is smaller than the specified number of values, then the possible bin values
  are uniformly distributed among the specified bins. The first 'n' specified
  values are assigned to the first bin, the next 'n' specified values are
  assigned to the next bin, etc. Duplicate values are retained; thus the same
  value can be assigned to multiple bins. If the number of values is not
  divisible by the number of bins, then the last bin will include the
  remaining items. For example: bins fixed [4] = { [1:10], 1, 4, 7 }; The 13
  possible values are distributed as follows: <1,2,3>, <4,5,6>, <7,8,9>,
  <10,1,4,7>. If the number of bins exceeds the number of values, then some
  of the bins will be empty."
- 19.5, L32433-32434: "The expression within the iff construct at the end of
  a bin definition provides a per-bin guard condition. If the expression is
  false at a sampling point, the count for the bin is not incremented."
- 19.5, L32436-32438: "The default specification defines a bin that is
  associated with none of the defined value bins. The default bin catches the
  values of the coverage point that do not lie within any of the defined
  bins. However, the coverage calculation for a coverage point shall not take
  into account the coverage captured by the default bin."
- 19.5.2, L32741, L32746-32749: "If a coverage point does not define any
  bins, SystemVerilog automatically creates state bins." "For an enum
  coverage point, N is the cardinality of the enumeration. For any other
  integral coverage point, N is the minimum of 2M and the value of the
  auto_bin_max option".
- 19.5.3, L32769-32770: "A wildcard bin definition causes all X, Z, or ? to
  be treated as wildcards for 0 or 1."
- 19.5.4, L32811-32812, L32814-32815, L32819-32820: "For state bins, each ignored value is
  removed from the set of values associated with any coverage bin." "The
  removal of ignored values shall occur after the distribution of values to
  the specified bins." "The above may result in a bin that is associated with
  no values or sequences. Such empty bins are excluded from coverage".
- 19.5.5, L32835-32841: "For state bins, each illegal value is removed from
  the set of values associated with any coverage bin. ... If an illegal value
  or transition occurs, a run-time error is issued. Illegal bins take
  precedence over any other bins, that is, they will result in a run-time
  error even if they are also included in another bin."
- 19.5.1, L32543-32544: value1 => value2 "represents the value of coverage
  point at two successive sample points, that is, value1 followed by value2
  at the next sample point."
- 19.5.1, L32597-32600: "Any number of sample points can occur before the
  first occurrence of the specified value and any number of sample points can
  occur between each occurrence of the specified value. The transition
  following the goto repetition must immediately follow the last occurrence
  of the repetition."
- 19.5.1, L32621-32622: "The transition following the nonconsecutive
  repetition may occur after any number of sample points so long as the
  repetition value does not occur again."
- 19.5.1, L32668-32678: the `sa`/`sb`/`sc`/`allother` example: "The first
  bins construct associates the following sequences with bin sa: 4=>5=>6, or
  7=>11, 8=>11, 9=>11, 10=>11, 7=>12, 8=>12, 9=>12, 10=>12. The second bins
  construct associates an individual bin with each of the above sequences"
  and "The bin allother increments twice."
- 19.5.1, L32690-32691: "A transition bin is incremented every time the
  sequence of value transitions of its corresponding coverage point matches a
  complete sequence, even when the sequences overlap."
- 19.5.1, L32710-32733: the `b2`..`b7` example: "the above sequence causes
  transition bin b2 to be incremented on the 8th sample (3 nonconsecutive
  twos), and transition bin b3 to be incremented on the 6th sample (3
  nonconsecutive threes). Likewise, transition bin b2 is incremented on the
  10th sample, and transition bin b3 is incremented on the 9th and 11th
  samples. Transition bin b5 is incremented on the 15th, 16th, 17th, and 18th
  samples. Transition bin b6 is incremented on the 12th sample. Transition bin
  b7 is incremented on the 13th sample. A transition bin is incremented at
  most once per sample."
- 19.5.1, L32735-32737: "Transition bin specifications of length 0 shall be
  illegal."
- 19.5.1, L32685-32688: "Transitions that specify sequences of unbounded or
  undetermined varying length cannot be used with the multiple bins construct
  (the [] notation). ... An attempt to specify multiple bins with such
  sequences shall result in an error."
- 19.5.4, L32812-32814: "For transition bins, any covered sequence is removed
  when it cannot be matched without also matching an ignored sequence. (For
  example, the ignored sequence 2=>3 would remove the covered sequence
  1=>2=>3=>4.)" L32816-32817: "Ignored transition bins cannot specify a
  sequence of unbounded or undetermined varying length."
- 19.5.5, L32839-32843: illegal transitions are a run-time error; "Illegal
  transition bins cannot specify a sequence of unbounded or undetermined
  varying length."
- 19.5.6, L32862-32863: "For wildcard bins, x and z values in b shall be
  treated as all possible 0 and 1 values prior to applying these rules."
  L32876-32884: "If a warning is issued for a bins element, the following
  rules shall apply: ... If an element of a bins open_range_list is a range
  [b1 : b2] and there exists at least one value in the range for which a
  warning would not be issued then the range shall be treated as containing
  the intersection of the values in the range and the values expressible by
  type(e)."
- 19.6, L32923-32925: "When a variable V is part of a cross coverage,
  SystemVerilog implicitly creates a coverage point for the variable, as if it
  had been created by the statement coverpoint V;."
- 19.6, L32980-32983: "If at any sample point, the condition evaluates to
  false, the cross coverage is ignored. The expression within the optional
  iff construct at the end of a cross bin definition provides a per-bin guard
  condition. If the expression is false, the cross bin is ignored."
- 19.6, L33033-33034: "No cross coverage bins shall be created for
  coverpoint bins that are specified as default, ignored, or illegal bins."
- 19.6, L33059-33061: "Automatically-generated bins are retained for those
  cross products which do not intersect cross products specified by any
  user-defined cross bin."
- 19.6.1, L33150-33191: the `cg`/`c` example: c1 "will cover only four
  cross products", c2 "includes the following seven cross products", c3
  "includes only one cross product", and "the cross retains those
  automatically-generated bins that represent cross products not
  intersecting any of the user-defined bins. There are 6 of these".
- 19.6.2, L33208-33209: "All cross products that satisfy the select
  expression are excluded from coverage. Ignored cross products are excluded
  even if they are included in other cross coverage bins of the enclosing
  cross."
- 19.6.3, L33223-33225: "All cross products that satisfy the select
  expression are excluded from coverage, and a run-time error is issued.
  Illegal cross products take precedence over any other cross products".
- 19.11.2, L33911-33921: "Bc = (Π Bj) − Bb", with Bu "the number of
  significant user-defined cross bins—excluding ignore_bins and
  illegal_bins" and Bb "the number of cross products that comprise all
  user-defined cross bins".
- 19.11.1, L33858-33860: "If there is no value or transition associated with
  a bin, the bin is ignored and shall not contribute to the coverage
  computation. That is, the bin is excluded from both the numerator and the
  denominator of the coverage equation."
- 19.11.3, L33937-33938: "When type_option.merge_instances is false, type
  coverage is computed as the weighted average of all instances."

- 19.7, Table 19-1, L33254-33260: weight "If set at the covergroup
  syntactic level, it specifies the weight of this covergroup instance for
  computing the overall instance coverage of the simulation. If set at the
  coverpoint (or cross) syntactic level, it specifies the weight of a
  coverpoint (or cross) for computing the instance coverage of the enclosing
  covergroup."
- 19.7, Table 19-1, L33273-33277: "at_least=number 1 Minimum number of hits
  for each bin." "detect_overlap=boolean 0 When true, a warning is issued if
  there is an overlap between the range list (or transition list) of two
  bins of a coverpoint."
- 19.7, Table 19-1, L33291-33295: get_inst_coverage "Only applies when the
  merge_instances type option is set. [...] When false, the value returned by
  get_inst_coverage shall equal the value returned by get_coverage."
- 19.7, Table 19-2, L33389: "at_least Yes (default for coverpoints &
  crosses) Yes Yes".
- 19.7.1, Table 19-3, L33409-33415: type_option weight "If set at the
  coverpoint (or cross) syntactic level, it specifies the weight of a
  coverpoint (or cross) for computing the cumulative (or type) coverage of
  the enclosing covergroup. The specified weight shall be a non-negative
  integral value."
- 19.7.1, L33449-33450: "Different instances of a covergroup cannot assign
  different values to type options. This is syntactically disallowed because
  these options can only be initialized via constant expressions".
- 19.11.3, L33939-33940: "When type_option.merge_instances is true, type
  coverage is computed as if instances were merged together into the type, as
  a union of coverage of all instances."
- 19.11.3, L33960-33962: "When bins overlap among instances, the cumulative
  coverage count of an overlapping bin is the sum of counts of that bin in all
  instances containing it."
- 19.11.3, L33987-34000: bins are compared by name: "binname[value]" for
  `[]` state bins, "binname[0]" through "binname[N-1]" for fixed arrays,
  "auto[value]" or "auto[low:high]" for automatic bins, "binname[transition]"
  for transition arrays, and "<binname1,...,binnameN>" for automatic cross
  bins; the `gt` and `ga` examples (66.6667; 100.0 and 50.0; 96 bins).
## Expected values

`bins.sv` (every weight is 1, so a covergroup is the mean of its coverpoints):

- `values`: `low` = {0..2} (`[$:2]`), `mid` = {2..5, 9}, `high` = {12..15}
  (`[12:$]`); `d` is a default bin and does not count. Samples 2 (`low` and
  `mid`) and 7 (`d` only): 2/3 = 66.67; 13 (`high`): 100.00.
- `arrays`: `each[]` = one bin per distinct value 1, 3, 4 (3 bins, S45-D6);
  `fixed[4]` is the clause's example `<1,2,3>`, `<4,5,6>`, `<7,8,9>`,
  `<10,1,4,7>`; `few[4]` = {14, 15} gives `few[0]` = {14}, `few[1]` = {15}
  and two empty bins (S45-D1). |bins| = 3 + 4 + 2 = 9. Samples 3 (`each[3]`,
  `fixed[0]`), 10 (`fixed[3]`), 15 (`few[1]`): 4/9 = 44.44; 4 (`each[4]`,
  `fixed[1]`, `fixed[3]`): 6/9 = 66.67.
- `removed`: `cb` is automatic with `auto_bin_max` 4: `<0:1>`, `<2:3>`,
  `<4:5>`, `<6:7>`; ignoring {0, 1, 5, 6} after the distribution empties the
  first bin and leaves {2, 3}, {4}, {7}: 3 bins. `cq`: `pair[2]` = `<0,1>`,
  `<2,3>`; ignoring {0, 1} empties `pair[0]`: 1 bin. Samples 4 (`cb` {4}) and
  0 (ignored): `cb` 1/3 = 33.33, `cq` 0.00, group 16.67. Sample 3: `cb` 2/3 =
  66.67, `cq` 100.00, group 83.33.
- `wild` (8 bits): `top` = 192..255, `odd` = the odd values, `mid` =
  64..95 (S45-D4: from `8'b0100_0000` to `8'b0101_1111`). Samples 0xc2
  (`top`) and 7 (`odd`): 2/3 = 66.67; 70 (`mid`): 100.00.
- `guard`: 1 with `en` = 0 does not count `one`; 2 counts `two`: 50.00; 1
  with `en` = 1: 100.00.
- `args`: `args1` = `new(2, 5, 2)`: `good` = {2..5}, `split[2]` = `<2,3>`,
  `<4,5>`, `rest` default; sample 4: `good`, `split[1]`: 2/3 = 66.67.
  `args2` = `new(10, 20, 3)`: both ranges are limited to the 4-bit values
  {10..15} with a run-time warning each (19.5.6); `split[3]` = `<10,11>`,
  `<12,13>`, `<14,15>`; sample 4 falls in `rest` only: 0/4 = 0.00. Type:
  (66.67 + 0) / 2 = 33.33.
- `typed`: `ce` has 3 enum bins, `G` ignored: 2 bins, R sampled: 50.00. `cs`
  (signed 4 bits): `neg` = {-8..-1}, `pos[]` = 1, 2, 3: 4 bins; samples -3
  (`neg`) and 2 (`pos[2]`): 50.00.

`illegal.sv`:

- `g`: `lo` = {0..7} minus the illegal 5; `rest` is an illegal default bin.
  Sample 3: 100.00. Sample 5 is a run-time error (19.5.5, S45-D3) that ends
  the run: `after 5` never prints, the final procedure runs and shows 100.00
  (5 counted nothing, also not in `lo`). Exit status 1.

`illegal_default.sv`: sample 2 hits `lo` (100.00); 12 is in no other bin, so
it hits the illegal default bin `rest`: a run-time error that ends the run
(S45-D3); the final procedure shows 100.00. Exit status 1.

`bins_count_runtime.sv`: `two` = `new(2)`: `b[2]` = <0,1>, <2,3> and `other`
= {9}: sample 9 is 1/3 = 33.33. `new(0)` gives `b[n]` a size that is not
positive, a run-time error when the instance is constructed that ends the run
(S45-D5): `after new(0)` never prints; the final procedure shows 33.33. Exit
status 1.

`transitions.sv` (A02):

- `sg` is the clause's `b2`..`b7` example: each bin at `at_least` 1 to 4, so
  the line for each sample shows the bin's count so far: `b3` on samples 6,
  9 and 11; `b2` on 8 and 10; `b6` on 12; `b7` on 13 only (S45-D8); `b5` on
  15 to 18.
- `cg` is the `sa`/`sb`/`sc` example with samples 4 5 7 11 8 12 2 2 3: `sa`
  is hit (7=>11, 8=>12), `sb[]` has 1 + 8 = 9 bins of which 7=>11 and 8=>12
  are hit, `sc` is hit (12=>2=>2=>3); `allother` does not count. `all`:
  (1 + 2 + 1) / (1 + 9 + 1) = 36.36; `p_sa` 100.00; `p_sb` 2/9 = 22.22.
- `g_removed`, ignored 2=>3: `long` (1=>2=>3=>4) and `mid` (2=>3=>4) always
  contain it and are empty; `part` keeps 1=>3; `arr[]` keeps only `arr[1=>3]`;
  `short` and `five` are untouched: 4 bins. Samples 1 2 3 4 hit `short` only
  (2=>3 removes `part`'s 2=>3): 25.00. Samples 1 3 hit `part` and
  `arr[1=>3]`: 75.00.
- `g_inst`: `ia` samples 1 then 2 (100.00); `ib`, interleaved, samples 2
  then 1 (0.00): histories are per instance. Type: 50.00.
- `g_skip` (S45-D9): 1, a sample with `en` = 0, 2 matches neither `cp` (the
  disabled sample ends the attempt) nor `guarded` (it sees 1 5 2). Then 1
  with `en` = 0 and 2 with `en` = 1: `cp` still 0.00, `guarded` completes with
  its guard true at the last sample: 100.00. `stopped`: 1, `stop()`,
  `start()`, 2 matches, since a stop does not end the attempt; 1 2 then
  matches again: 100.00, 100.00.
- `g_x` (S45-D7): 1, `4'bxx01`, 2: the X sample ends the attempts of both
  `next` (1 => 2) and `later` (1 => 2 [-> 1]), also in a goto gap: 0.00,
  0.00.
- `g_wild`: 3 => 1 is `2'b1?` => `2'b0?`: `w` of {`w`, `other`}: 50.00.

`transition_illegal.sv` (S45-D10, S45-D3): samples 5 6 9 9 1 2 3. 5 starts
`pair` but ends nothing and nothing was pending before (the first sample, no
report); 6 completes `pair`; each 9 ends nothing with nothing pending, and so
does 1 (it starts `up` but nothing was pending before it): the illegal
default sequence reports three samples and the run continues. 2 continues
`up`; 3 completes `bad` (2=>3), a run-time error that ends the run, and `up`
(1=>2=>3), which contains it and is removed, so it is empty. `coverage` never
prints; the final procedure shows `pair` only, 100.00. Exit status 1.

`crosses.sv` (A01, A03):

- `ct` is the 19.6 example, samples (0,0) and (1,1): `x1` has 4 automatic
  bins, 2 hit: 50.00; `x2` has `i_zero` (both `i[0]` products) and the 2
  automatic `i[1]` products: `i_zero` and `<i[1],j[1]>` hit, 2/3 = 66.67.
- `cg` is the 19.6.1 example: Bb = 10 (c1 the `a1` row, c2 the `a2` row and
  the `b2` column, c3 inside c1), Bc = 16 − 10 = 6, Bu = 3: 9 bins. (10, 200)
  is <a1,b4>, in c1 and c3; (200, 0) is the automatic <a4,b1>: 3/9 = 33.33.
  (100, 50) is <a2,b2>, in c2: 4/9 = 44.44. Group: `a` and `b` have 3 of 4
  bins each: (75 + 75 + 44.44) / 3 = 64.81.
- `g_implicit` crosses variable `p` (an implicit coverpoint with 4 automatic
  bins) with `cq` (2 bins): 8 products, 2 hit: 25.00. Group: `cq` 100, `p`
  50, `px` 25: 58.33.
- `g_ignore`: `ca` and `cb` have 4 automatic bins each. `plain` ignores the
  `ca` = 0 row: Bc = 12; (0,1) is ignored, (1,1) counts: 0.00, then 8.33.
  `both` adds `one` (the `cb` = 1 column): Bb = 4 + 3 = 7, Bc = 9, Bu = 1;
  (0,1) is ignored although `one` holds it; (1,1) counts in `one`: 0.00,
  then 1/10 = 10.00.
- `g_guard`: `on` has 4 products and the cross `iff (q)`; `picked` has
  `only` = (hi, *) with `iff (q)` and `arr` = (lo or hi, hi) (`cb.lo`
  intersects {0}, so `!` leaves `hi`): Bb = 3, Bc = 1, Bu = 2. Sample 1 has
  `en` = 0, so `cb` counts nothing and neither cross counts (S45-D14). Sample
  2, (hi, hi) with `q` = 0: `on` is off, `only` is guarded off, `arr` counts:
  0.00, 33.33. Sample 3, (lo, lo) with `q` = 1: `on` 1/4 = 25.00, `picked`
  adds its automatic bin: 66.67.
- `g_large` (A03): two 12-bit coverpoints with 4096 automatic bins each:
  `all` has 2^24 = 16777216 products in sparse storage; 3 hit:
  300 / 16777216 = 0.00001788. `half` ignores `ca` < 2048: Bc = 2048 x 4096
  = 8388608, and (0,0) is ignored: 200 / 8388608 = 0.00002384.

`cross_illegal.sv`: 16 products; `diag` = <1,1>, `skip` the `cx` = 3 row,
`illegal` the `cy` = 2 column (the argument `bad`): Bb = 1 + 4 + 4 − 1 = 8,
Bc = 8, Bu = 1. (1,1) counts `diag`: 1/9 = 11.11; (0,0) an automatic bin:
22.22. (3,2) is illegal although ignored: a run-time error that ends the run
(S45-D3), nothing counts; the final procedure shows 22.22. Exit status 1.

`options.sv`:

- `gw` (merge_instances 0): group option.at_least = 2 is the default of `cy`
  and `xy` (Table 19-2); `cx` overrides it with 1. `w1` has `cx` weight 1,
  `w2` weight 0; `cy` weight 1 (its type_option.weight 0 only affects type
  coverage); `xy` weight 3; goal, comment, per_instance and
  cross_num_print_missing change no number. After (1,1) once: `cx` `lo` 1/2
  = 50.00, `cy` 0.00 (1 hit < 2), `xy` 0.00, `w1` (50·1 + 0 + 0·3)/5 = 10.00.
  After (1,1) again in `w1`: `cy` 1/4 = 25.00, `xy` 1/8 = 12.50, `w1` (50 + 25
  + 37.5)/5 = 22.50; `w2` 0.00 (`cx` has weight 0); `gw::get_coverage()` is
  the option.weight (2 and 2) average of the instances: 11.2500.
- `gt` and `gi` are the 19.11.3 example: 66.6667; `gt` instances report the
  type value (get_inst_coverage 0): 66.67 66.67; `gi` sets
  get_inst_coverage: 100.00 and 50.00.
- `ga` is the 19.11.3 automatic-bin example: 16 + 64 = 96 names, none shared
  (`auto[0:3]` ... vs `auto[0:7]` ...). 5 hits `auto[4:7]` and `auto[0:7]`:
  2/96 = 2.0833.
- `gm` merges `m1` (0,0) and `m2` (0,0), (1,1): `cx` `lo` and `mid`: 100.00;
  `tx` has `up`, `any[0=>1]`, `any[0=>2]`, `m2` completes 0 => 1: 2/3 =
  66.67; `cy` 2/4 = 50; `xy` (lo,0), (lo,1), (mid,1) of 8: 37.50. The type is
  weighted by type_option.weight (cx 3, tx 0, cy 1, xy 1): (300 + 50 +
  37.5)/5 = 77.5000, and `m1.get_inst_coverage()` returns it (Table 19-1).
- `go` warns for `lo`/`mid` (they share 1; `top` is emptied by `skip`, S45-D17)
  and for `t1`/`t2` (both hold 0 => 1); `quiet` turns the check off. After
  sample 0: `cx` `lo` of `lo`, `mid` = 50, `tx` 0, `quiet` `p` (y = 1) 1/2 =
  50: 33.33.
- `$get_coverage()` averages the type coverage of the six types with their
  type_option.weight (`gm` 2): (11.25 + 66.6667 + 66.6667 + 2.0833 + 2·77.5
  + 33.3333)/7 = 47.8571.

`merge_cross.sv`: `ab` has 2 × 2 bins in `g1` and 3 × 2 in `g2`, so the
merged cross has no common layout: a run-time error at the type query (llg
limitation), which reports 0.00; the instance values (get_inst_coverage 1)
are 1/4 = 25.00 and 1/6 = 16.67.

`transition_arguments.sv` (industry-standard behaviour, probed): the
samples are 2 3 5 4 2 7 8 3 1 4 4 6 2 2 4 8 1 4 6 (positions 1 to 19); every
coverpoint of `cg` has one bin unless noted, so it shows 100.00 when the bin
is hit and 0.00 otherwise, and `all` is the mean of the eight coverpoints.

- `a` = `new(2, 4, 20)`: `tr` (2=>3) at 1-2; `rng` ([2:4]=>5) at 2-3; `two`
  (2=>4 or 4=>2) at 4-5; `out` (20=>1) is outside the 4-bit type, so the
  bin is ignored with a run-time warning (19.5.6) and is not coverable, and
  `c` (7=>8) at 6-7 makes `p_out` 100.00; `part` ([2:20]=>1) is limited to
  [2:15]=>1 with a warning and hit at 8-9 (3 1); `rep` (2 [* 2] => 4) at
  13-15; `g` (2 [-> 2] => 4) at 13-15; `p_mix`: `m` (2=>8) never, `k` (8=>1)
  at 16-17: 50.00. `all` = (7 · 100 + 50) / 8 = 93.75.
- `b` = `new(4, 6, 30)`: `tr` (4=>3) and `rng` ([4:6]=>5) never: 0.00;
  `two` (4=>6) at 11-12; `out` (30=>1) ignored with a warning, `c` hit;
  `part` ([4:30]=>1) limited to [4:15]=>1 with a warning, hit at 16-17 (8
  1); `rep` (4 [* 2] => 6) at 10-12; `g` (4 [-> 2] => 6) at 10-12; `p_mix`:
  `m` (4=>8) at 15-16, `k` at 16-17: 100.00. `all` = 6 · 100 / 8 = 75.00.
- `cm` merges its instances (19.11.3): `m1` = `new(2, 20)` has `t` (2=>3,
  hit), `c` (hit) and `n` (2=>9, never); its `o` (20=>1) is ignored with a
  warning. `m2` = `new(4, 3)` has `t` (4=>3, never), `o` (3=>1, hit at 8-9),
  `c` (hit) and `n` (4=>9, never). The union by name has `t`, `o`, `c`, `n`,
  with `t`, `o` and `c` hit: 3/4 = 75.00. This line is the 19.11.3 union
  that llg also applies to value bins (S45-D18); industry-standard behaviour
  reports the mean of the instances, 58.33, when an argument removes a bin
  in one instance, for value bins as for transition bins.

The run-time warnings come once per instance, in construction order.

`wide_points.sv` (industry-standard behaviour, probed; 71-bit `x`, signed
71-bit `s`, 128-bit `q`, signed 100-bit `t`). The ten samples of `x` are 0,
2^70, 2^69, 2, 2^66, 2^71 - 1, 5, 2^68 + 6, 2^66, 0; `s` is -1, -2^70, then
0; `n` 0 then 1; `q` 0, 2^128 - 1, 2^64, then 0x5555 · 2^112; `t` -2, 5,
then -3. Every coverpoint and the cross have weight 1.

- `p_val`: `zero`, `big`, `rng` hit, `miss` (7) never: 75.00. `p_arr`:
  `arr[2]`, `arr[2^70]` and `top` hit, `arr[1]` not: 75.00. `p_ign`: `all`
  ([0:15] without 5) by 0 and 2, `hi` by 2^66: 100.00.
- `p_auto`: 64 automatic bins of 2^65 values (19.5.2): bins 0, 32, 16, 2,
  63 and 8: 6/64 = 9.38. `p_sauto` (signed, numeric order): -2^70 in bin 0,
  -1 in bin 31, 0 in bin 32: 3/64 = 4.69. `p_qauto` (`auto_bin_max` 3):
  bins of floor(2^128 / 3) values; 0, 2^64 and 0x5555 · 2^112 fall in bin 0,
  2^128 - 1 in bin 2: 2/3 = 66.67.
- `p_cnt`: `c[4]` over 8 values, 2 each; 2^68 + 6 is in `c[3]`: 25.00.
- `p_tr`: `u` (0 => 2^70) at samples 1-2 and `v[2=>2^66]` at 4-5; `t` and
  `v[1=>2^66]` never: 2/4 = 50.00.
- `p_x`: `hi` is the `big` and `rng` rows (8 products), the other 8 products
  are automatic: 9 bins; (zero, 0), (big, 1) and (rng, 1) in `hi`, and
  (zero, 1) at the last sample: 3/9 = 33.33.
- `p_wild`: `w` is 2^71 - 16 .. 2^71 - 1, hit by 2^71 - 1; `o` never: 50.00.
- `p_signed`: `neg` by -1, `mn` (the 71-bit -2^70) by -2^70, `z` by 0, `pos`
  never: 75.00. `p_q`: all three hit: 100.00. `p_t`: `neg[-2]` and
  `neg[-3]` of three: 66.67. `p_n`: 2 of 4 automatic bins: 50.00.
- `all`: the mean of the 13 coverpoints and the cross: 780.73 / 14 = 55.77.

`wide_illegal.sv` (industry-standard behaviour for a 71-bit coverpoint,
probed; the fixture uses the longest value a coverpoint llg serves can
spell): 3 hits `ok` (100.00); -2^127, the most negative 128-bit value, is
illegal, a run-time error that ends the run (S45-D3) and names the whole
value (40 characters); `after` never prints and the final procedure shows
100.00. Exit status 1.

`wide_arguments.sv` is the plan's probe of a 71-bit coverpoint (`zero` and
`big` both hit) beside a transition bin reading the argument `low` (2 => 3,
hit at the last edge) and the value bin `[low:3]`: every value 100.00
(industry-standard behaviour, probed).

`cross_classes.sv` (llg limitation): each of `sa`..`sd` is 32 single-value
selects of one crossed coverpoint, so every coverpoint splits into 33 value
classes (its bins 0..31 one by one, and bins 32..63); 33^4 = 1185921
combinations exceed the 2^20 llg counts. Constructing the instance in the
`initial` procedure reports a run-time error located at `sa`, the first
bins of the cross; the run goes on, `ca` counts 1 of its 64 bins: 1.56, and
the cross counts nothing: 0.00. Exit status 1.

`overlap_budget.sv` (llg limitation): `a` and `b` share no complete
sequence (the final 2 must follow both the last 1 and the last 3), but the
lockstep search of option.detect_overlap explores every pair of goto counts,
about 10^6 pairs, past its budget of 2^20 steps: a located compile-time
warning names the pair where it stopped. The run reports no overlap; samples
0 and 2 complete neither bin: 0.00.

## Negative cases

| Fixture | Error |
| --- | --- |
| `neg_ignore_default` | `ignore_bins` with `default` (Slang: default coverage bins cannot be ignored) |
| `neg_bins_count` | a constant bins array size of 0 (S45-D5) |
| `neg_bins_with` | a bins `with` filter, 1800-2012 syntax |
| `neg_wildcard_wide` | a wildcard value that expands to more than 4096 ranges (llg limitation) |
| `neg_wide_argument` | a bins value read from an argument wider than 64 bits (llg limitation) |
| `neg_trans_length0` | `(3 [* 1:1])`, a transition of length 0: its repeat_range evaluates to 1 (S45-D11; `(3 [-> 1])` and the other forms of S45-D19 are legal) |
| `neg_trans_array_goto` | `[]` transition bins with a goto repetition (19.5.1) |
| `neg_trans_ignore_goto` | ignore_bins with a nonconsecutive repetition (19.5.4) |
| `neg_wide_auto_ignore` | ignore_bins in a 71-bit coverpoint with automatic bins (llg limitation) |
| `neg_wide_exact` | a `[4]` bins array over 2^32 + 1 values of a 71-bit coverpoint (llg limitation) |
| `neg_wide_point_argument` | a bins value of a 71-bit coverpoint read from a covergroup argument (llg limitation) |
| `neg_wide_intersect_auto` | `intersect` on a 71-bit coverpoint with automatic bins (llg limitation) |
| `neg_wide_enum` | an enum coverpoint of 70 bits (llg limitation: enum coverpoints of at most 64 bits) |
| `neg_trans_argument_array` | `[]` transition bins whose value reads a covergroup argument (llg limitation) |
| `neg_trans_argument_removal` | a transition value read from a covergroup argument in a coverpoint with ignore_bins transitions (llg limitation) |
| `neg_trans_argument_overlap` | a transition value read from a covergroup argument where option.detect_overlap may be set (llg limitation) |
| `neg_cross_of_cross` | a cross of a cross (not 1800-2009 syntax) |
| `neg_cross_transition` | a cross of a coverpoint with transition bins (llg limitation) |
| `neg_trans_states` | `2 [* 70000]` needs more than 65536 automaton states in one coverpoint (llg limitation) |
| `neg_trans_sequences` | `[]` transition bins whose `[* 1:65]` ranges expand to 4225 sequences, more than 4096 (llg limitation) |
| `neg_trans_ignore_sequences` | ignore_bins whose `[* 1:65]` ranges expand to 4225 sequences, more than 4096 (llg limitation) |
| `neg_trans_starts` | 1024 sequence shapes of `t[]` starting at more than 2200 value classes: more than 2^20 start entries (llg limitation) |
| `neg_trans_class_sequences` | counting the sequences of `t[]` over more than 2200 value classes needs more than 2^20 class sequences (llg limitation) |
| `neg_binsof_transition` | `binsof` of a transition bin; the cross of its coverpoint is rejected first (llg limitation) |
| `neg_cross_select_product` | a select `&&` that expands to 33 × 32 = 1056 terms, more than 1024 (llg limitation) |
| `neg_cross_select_union` | a select `\|\|` that expands to 1024 + 1 terms, more than 1024 (llg limitation) |
| `neg_cross_set` | a cross bin `with` filter, 1800-2012 syntax |
| `neg_type_weight` | `type_option.weight = -2` (Table 19-3) |
| `neg_merge_argument` | type_option.merge_instances from an argument (Slang: not a constant, 19.7.1) |
