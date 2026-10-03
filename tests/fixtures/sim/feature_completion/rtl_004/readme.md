# RTL-004 fixed patterns and positional scatter

IEEE 1800-2009 §§7.6, 10.4.2 and 10.9.1–10.9.2 supply the value and
snapshot oracles. All sources run through the public CLI in both optimizer modes.
Representative sources additionally run after snapshot/Db destruction at O0/O3.

- `vector_keys` adopts FND-002's `vector_type_key` positive and extends it to
  ascending bounds, two-state conversion and packed immediate row types.
- `record_keys` checks mixed state, duplicate type-key last-match precedence,
  explicit keys and 65-bit boundaries. Outputs follow the declared member order.
- `nested_rows` and `scatter_capture` adopt the checked-in continuation witnesses;
  assertions derive left-to-left row mapping and frozen source/target coordinates.
- `effect_values` uses idempotent effects and invariant return values; it asserts
  values and the allowed idempotent flag set `{0, 1}`, never an invocation count or
  cross-operand order for type/default/replicated operands.
- `descriptor_patterns` exercises sparse exceptions, overlap, issue-time NBA,
  state conversion, recursive all-one defaults and alternating repetitions over
  negative coordinates. `descriptor_scale` executes 65,537 and 16M 17-bit cells;
  the source-size check compares generated models at those extents.
- `scatter_records` checks record destinations, shared overlapping nibbles, static
  locals outliving NBA issue and selected-cell notification. Both overlapping
  stores supply `aa`, so their common nibble is independent of publication order.
- `net_pattern_driver` adopts FND-002's static net scatter witness; packed `12`
  decomposes in declaration order into hexadecimal `1 2`.
- Duplicate semantic indices, keyed/default/replicated or constant lvalues,
  uncovered elements, wrong shapes, negative replication and automatic/ref NBA
  targets retain specific diagnostic tests. FND-002 negatives are adopted by name.
  The inherited real automatic-NBA witness is supplemented by fixed scatter NBA.

All `.out` files are hand-derived. No evaluator output was used as an oracle.
Zero-count rejection is only claimed for the admitted nonempty fixed slice;
empty/native container replication remains outside this task.
Oversized array-valued items and scatter across large selected rows/records depend
on the separate aggregate-view transport work; these are not language negatives.
