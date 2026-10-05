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
  selector keeps each immediate member that is equal in both arms and gives
  the others their uninitialized value (SV 11.4.11): member `a` differs, so
  it becomes all zero (`bit`), while `tag` and `n` are kept. The NBA is
  visible only after `#1`. The named pattern fills every element of `a` with
  `7`; `'{default: 0}` clears every member.
- `oversized_values` (a 2,097,160-bit record): `f` returns a copy with
  `t = 9`, so `r == s` has a known mismatch in `t` (`0`) despite X cells in
  `w`. The ambiguous conditional resets the unequal members to their
  uninitialized values (`x`, and `0` for the two-state `t`). `g` copies `r`
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
- Negative cases: binding a whole value beyond the packed limit to a pattern
  variable and comparing a record function result (no packed value exists)
  are rejected; a procedural write to a record driven by a continuous
  assignment or an output port violates the single-writer rule (SV 6.5).
