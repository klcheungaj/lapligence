# SIM-045 fixtures: coverpoint bins, transitions, crosses and options

Run by [`tests/sim_feature_completion/sim_045.rs`](../../../../sim_feature_completion/sim_045.rs).
Positive cases run in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends; negative cases run in both optimizer modes. Line
numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt` (SV). The interpretations that needed a choice
are the register decisions
[S45-D1 and later](../../../../../docs/lrm_decisions.md#coverpoint-bins-transitions-crosses-and-options-sim-045).

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
- 19.5.6, L32862-32863: "For wildcard bins, x and z values in b shall be
  treated as all possible 0 and 1 values prior to applying these rules."
  L32876-32884: "If a warning is issued for a bins element, the following
  rules shall apply: ... If an element of a bins open_range_list is a range
  [b1 : b2] and there exists at least one value in the range for which a
  warning would not be issued then the range shall be treated as containing
  the intersection of the values in the range and the values expressible by
  type(e)."
- 19.11.1, L33858-33860: "If there is no value or transition associated with
  a bin, the bin is ignored and shall not contribute to the coverage
  computation. That is, the bin is excluded from both the numerator and the
  denominator of the coverage equation."
- 19.11.3, L33937-33938: "When type_option.merge_instances is false, type
  coverage is computed as the weighted average of all instances."

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
  Sample 5 is a run-time error (19.5.5, S45-D3) and counts nothing, also not
  in `lo`; 12 hits the illegal default: another error. 0/1 = 0.00. Sample 3:
  100.00.
- `g_count` with `n` = 0: `b[n]` has no bins, a run-time error when the
  instance is constructed (S45-D5); `other` = {9} is the only bin: sample 9 is
  100.00. The run exits with status 1.

## Negative cases

| Fixture | Error |
| --- | --- |
| `neg_ignore_default` | `ignore_bins` with `default` (Slang: default coverage bins cannot be ignored) |
| `neg_bins_count` | a constant bins array size of 0 (S45-D5) |
| `neg_bins_with` | a bins `with` filter, 1800-2012 syntax |
| `neg_wildcard_wide` | a wildcard value that expands to more than 4096 ranges (llg limitation) |
| `neg_wide_argument` | a bins value read from an argument wider than 64 bits (llg limitation) |
