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
- `chosen_constant_cells`: constant selects of `string`, handle and record
  cells go through their position, and a record element written whole from a
  record of its type stores leaf by leaf; the invalid-index rule quoted above
  holds at any constant, and an unknown index keeps the decoding path. Whole
  writes change only the leaves that differ, and `-0.0` differs from `0.0` by
  bits as a change of a variable does. Industry-standard behaviour stops at a
  constant select beyond a dynamic-data record's member with an out-of-bound
  error, against 7.4.6, and it does not wake on such a record's members at all
  (9.4.2); without those lines and with the plain record `w`, lines agree.
- `chosen_element_compare`: whole elements of records with `string` and `real`
  member arrays compare from fixed, dynamic, queue and associative containers;
  the lines match `-D N=8`, `-D N=5000` and industry-standard behaviour.
- `chosen_real_copy`: a whole `real` column copy writes only the cells whose
  bits differ, so an observer wakes once per change; `-0.0` against `0.0` is a
  change and a repeated NaN is not. Identical to industry-standard behaviour.
- `chosen_dense_source`: a whole member takes a copy from a directly addressed
  array and gives one back; `%p` prints a whole record and a whole member array
  in one image each, in the form of the record's leaves.
- `chosen_vpi`: an application reads, watches and writes the scalar members of a
  chosen record and walks its members. Expected lines are the per-cell form's
  output, compared line by line: `tb.r.s`, `tb.r.a` and `tb.r.a[3]` have no object
  in either form.
- `chosen_toggle`: toggle coverage entities are packed variables (S46-D14), so a
  chosen record changes neither count.
- `chosen_observers`: writes to cells of a chosen record reach run-time index
  readers (`always_comb`, `wait`, `@`, `$monitor`) in both instances of a shared
  body, while `quiet`, which nothing reads, gives up its contents marker. Expected
  lines match industry-standard behaviour. SV2009 9.4.2 L11836-11837: "Changing the value
  of object data members, aggregate elements, or the size of a dynamically sized
  array referenced by a method or function shall cause the event expression to be
  reevaluated."
- `chosen_sampled`: `$sampled`, `$rose`, `$fell`, `$changed`, `$stable`, `$past`
  and concurrent assertions on cells of a chosen record, at constant and run-time
  indices and with bit and part selects, including writes and a whole-array change
  in the slot of the clock edge. `N=8` gives the same lines through per-cell
  leaves. Expected lines match industry-standard behaviour. SV2009 16.9.3
  L22669-22670: "The $sampled function returns the value of the expression
  sampled in the Preponed region of the simulation time step in which the
  function is called."
- `chosen_native`: a chosen record with a string member through a queue, a class
  property, a mailbox and function results, with one runtime copy per column.
  `N=8` gives the same lines through per-cell leaves.
- `chosen_ref`: `ref` formals of a chosen record with a string member are bound
  by specializing the subroutine for the actual, so a reader sees the writes of
  a suspended task; a subroutine local passes its native value by address.
- `chosen_elements`: a record element of a member array is one column cell.
  Its members select from that cell at constant and run-time indices, and it
  moves as a value to and from record variables, arrays of records, arguments
  and streams. Expected lines equal the per-cell form's at `N = 15`, with the
  three run-time selected whole elements written as constants there, because a
  whole record with a member array has no run-time selection below 16 cells.
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
- `chosen_structure`: records of the column form as a port connected to an
  element of an array of records, a member array driving a net array and a
  dense array through one pattern, an initializer whose cells differ, a mailbox,
  a queue, an array of records and `matches` on an element. Rebuilt with
  `-D N=8` it takes the per-cell paths and prints the same lines.
- `chosen_part_writers`: separate continuous assignments and `always_comb`
  blocks write separate bits of one member of a column cell. SV2009 9.2.2.2:
  "multiple assignments made to independent elements of a variable are allowed
  as long as their longest static prefixes do not overlap".
- `chosen_cell_ref`: a `ref` formal of the column form bound to one image in a
  column cell or an array of records. One body serves every cell at any index,
  forwarding reaches the same cell, a wait sees the cell's writes. `-D N=8`
  prints the same lines through per-cell storage.
- `chosen_block_records`: the same members in static and automatic block
  records, a static class property, a fork, above 4,096 cells and beside a
  member above that size, with
  whole real columns compared by value and by bit pattern. An automatic record
  restarts from defaults at the declared size. Expected output is checked by
  hand against the rules of §§6.8 and 7.2, not against another simulator.
- `chosen_native_members`: member arrays of 16 cells or more with string, handle,
  real, shortreal and record elements beside packed arrays, multidimensional
  arrays and records nested in small arrays: selects, copies, comparisons, calls,
  `ref` and `output` formals, ports, class properties, queues, patterns, events
  and nonblocking writes. Its lines were taken from the per-cell form of the
  previous compiler.
- `chosen_nested`: a member array as an `inside` set and `case inside` item, an
  element pattern, a fill of structured elements (`default` reaches each leaf,
  SV 10.9.2), a record nested in a record, `foreach` on an element member.
  `-D N=8` prints the same lines per cell.
- `chosen_element_ref`: lrm_decisions S13-D4 with a 16-cell member array: the
  formal is the column form's, the actuals are elements with native members, and
  the lines are that fixture's.
- `chosen_event_nba`, `chosen_event_nba_native`: whole records, elements and
  cells with columns, per-cell members, strings, handles and containers written
  through `@`, `repeat` and `#`. §4.9.4: "The values in effect when the update is
  placed in the event region are used to compute both the right-hand value and
  the left-hand target."; §9.4.5: "An intra-assignment delay or event control
  shall delay the assignment of the new value to the left-hand side, but the
  right-hand expression shall be evaluated before the delay, instead of after the
  delay." The target of `arr[k].d[k] <= @(e) 6.5` is the one selected at issue;
  industry-standard behaviour leaves that cell unwritten, against this text. The
  other lines without a `#` write and without strings, handles or containers match
  industry-standard behaviour, which refuses a delay on an unpacked struct and
  a nonblocking write of a record with strings.
- `chosen_ref_wait`: tasks waiting on `ref` formals bound to record elements at a
  run-time index, in records with member arrays held in containers and in dynamic,
  queue and associative arrays. §13.5.2: "both the caller and the subroutine
  share the same representation", so each task wakes for its own element: writes
  of other elements change nothing it reads. Forwarding, recursion, a record
  beside an element, `fork ... join` in the task, virtual methods. The lines match
  `-D N=4` and `-D N=5000`. Checked against the text alone: industry-standard
  behaviour has this binding as a not-yet-implemented error for any array select.
- `chosen_call_source`: a call returning a record of the column form, and an automatic
  record of that form, assigned to and from elements, members and persistent records,
  directly, nested, inside a task and with `<=`; records of packed cells held in one
  member column; member arrays in slots merged under an unknown predicate. The lines
  match `-D N=4`, and industry-standard behaviour without the `<=` lines, refused for a
  record with a `string`, and with two differences that follow the text. `co.e[0] =
  kq[0]` copies the member array too, where that behaviour leaves it at its default
  (§7.2.2, "A structure can be assigned as a whole"). `l = unknown ? wq[0] : l` keeps the element `[p]`
  that matches: §11.4.11, "If the elements match, the element is returned", where that
  behaviour gives every string of the member array its default.
- `chosen_nested_assoc`: elements of nested associative arrays holding records with
  member arrays in containers (§§7.8, 7.9): writes create them, reads of missing
  entries give defaults, whole copies, `foreach`, `delete` and `ref` formals. The
  lines match `-D N=6`.
