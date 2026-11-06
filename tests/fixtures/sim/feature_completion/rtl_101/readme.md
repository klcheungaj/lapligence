# RTL-101 column-layout records and tagged unions

IEEE 1800-2009 §§7.2, 7.3.2, 7.4, 10.10, 11.4.5, 11.4.11, 12.6 and 23.3.3
supply the oracles. Records whose member arrays exceed the 4,096-cell dense
threshold, and records or tagged unions wider than the 1,048,575-bit packed
limit, keep each member array and each scalar leaf in its own descriptor
column. Expected values are derived by hand below; none were captured from
`llg`.

- `member_columns`: `pr.a` is filled by `foreach` with `k[7:0]`, copied to
  `pr.b`, and the pattern lvalue `'{d2, d3} = pr` copies both members, so
  `d2[3] = 03`, `d3[65536] = 00` and `d3[257] = 01`. `x` follows `r.a[i]`
  through `always_comb` (`5a`, then `5b` after the write). Clearing bit 1 of
  `4'h3` gives `0001`. The two-state member array equals its copy `arr`
  (`1 0`). After `s = r` the records are equal (`==` and `===` are `1`);
  writing `s.a[9]` makes `==` `0`. A known `0` selector copies `s`; an `x`
  selector keeps each leaf that is equal in both arms and gives the others
  their uninitialized value (decision R05-D1, SV 11.4.11): only `a[9]`
  differs, so it becomes `00` (`bit`), while `a[5]` (`5b`), `tag` and `n` are
  kept. The NBA is
  visible only after `#1`. The named pattern fills every element of `a` with
  `7`; `'{default: 0}` clears every member.
- `oversized_values` (a 2,097,160-bit record): `f` returns a copy with
  `t = 9`, so `r == s` has a known mismatch in `t` (`0`) despite X cells in
  `w`. The ambiguous conditional merges leaf by leaf (decision R05-D1):
  `w[3]` is 5 in both arms and is kept, while the unequal two-state `t`
  becomes its uninitialized value 0 (SV2009 11.4.11 L15215-15216). Other
  simulators combine the bits of the differing `t` (1 and 9) as for a packed
  conditional, which gives 1 once its X bit is stored in a two-state member. `g` copies `r`
  into `u`, sets `u.w[1] = 77` and increments the inout `s.t` from 9 to 10;
  `h` writes through a `ref` formal; the static `st` adds 3; `depth` recurses
  five times adding 1 to `t`. Ports and the continuous assignment copy `r`
  into `q` and `z`; the `ref` port of `stamp` writes `r` at time 5, which
  then propagates. `mk(8'd6)` fills every `w` element and `t` with 6.
- `tagged_columns` (a 2-bit tag above a 2,097,152-bit payload): a tagged
  expression resets the inactive members to their uninitialized value, as
  the payload padding of a packed tagged union does, so comparing two `t`
  values with X inactive members yields `x`; different tags compare unequal.
  `bump` matches `tagged t .n` and returns `n + 1`. Pattern cases select the
  active member's item; `void` members match by tag alone.
- `tagged_inactive`: reading `v.w[0]` while `t` is active reports a runtime
  error and yields X; the write `v.w[1] = 3` reports an error and leaves `t`
  unchanged, so the run ends with status 1.
- `record_patterns`: structure patterns test each named column; `.k` binds
  the scalar member, a mismatching constant fails, `.*` matches the whole
  record, and `casex` treats the X tag bit as a wildcard.
- `scale_65537` and `scale_1048576`: the same design at two extents; the
  generated models differ only in the spelled bounds.
- `whole_binding` and `record_result_operand` were limits of RTL-101 and are
  legal since RTL-101b: the bound copy of `r` has tag `3`; `f(r) == r` has an
  equal `t` and X cells in every `w`, so the equality is `x`.
- Negative cases: a procedural write to a record driven by a continuous
  assignment or an output port violates the single-writer rule (SV 6.5).

- `chosen_columns`: records below the dense threshold whose member arrays the
  design selects (`record_columns/choice.rs`): a run-time index into `n.a` of
  the record with a `string` member, and every select of the fixed record `f`.
  The expected output is the per-cell form's; a commercial simulator gives the
  same lines, in another same-time order for the two `always` blocks at times
  1 and 9 (SV 4.7, a nondeterministic order), with `name` declared packed because it
  rejects `n.a` with a `string` member in the NBA and the continuous assignments.
  `always_comb` runs at 0 and for each change of `f.a[3]` or `f.a[4]`, not for
  `f.a[5]` at time 2. SV2009 9.2.2.2.1 L11184-11185: "The implicit sensitivity
  list of an always_comb includes the expansions of the longest static prefix
  of each variable or select expression that is read within the block or
  within any function called within the block with the following exceptions:".
  `k = 40` and an X index write nothing and read 0. SV2009 7.4.6 L8235-8237:
  "If an index expression is out of the address bounds or if any bit in the
  address is X or Z, then the index shall be invalid. The result of reading
  from an array with an invalid index shall return the default uninitialized
  value for the array element type. Writing to an array with an invalid index
  shall perform no operation."
- `chosen_fallback`: `%p` of the whole record and a copy from a directly
  addressed array are outside column layout, so the record keeps its leaves.
- `chosen_whole_values`: whole copies and comparisons of chosen records, which
  read whole operands in place and write only differing cells. A copy that
  changes nothing wakes nothing (`q = p` at time 2). `q.a[7]`, a cell only `q`
  holds, returns to `p`'s default 0. `b` starts X, so `==` is `x` and `===` is 1
  on equal records; a known mismatch gives 0 from both. SV2009 11.4.5 L14795:
  "the relation is ambiguous, then the result shall be a 1-bit unknown value
  (x).". `q = q`
  and `swap_in(f, f)` have one array on both sides. The NBA copy takes `p`
  before `p.a[3] = 8`, so `q.a[3]` stays 6. Writes that change `g.b[5]` are in
  separate time steps because simulators differ on `@` over a value that returns
  to its old one within a process; this case is not about that. Expected lines
  match industry-standard behaviour with `name` packed: an NBA of a record with a
  `string` member is not portable, so that form is checked by llg's own storage
  forms only.
