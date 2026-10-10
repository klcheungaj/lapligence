# SIM-020 native and resizable bit-stream operations

IEEE 1800-2009 §§6.24.3 and 11.4.14 (with 11.4.14.1-4), 6.16, 6.21 and
10.4.2 are the oracles. Line numbers refer to the `pdftotext -layout`
extraction `SystemVerilog-1800-2009.txt`. Expected outputs were derived from
the clauses by hand, not captured from llg. The suite
`tests/sim_feature_completion/sim_020.rs` runs every positive fixture in both
optimizer modes on the legacy, compact/portable and compact/GMP value
backends.

## Clauses relied on

- 6.24.3 L7514-7515: "When a dynamic array, queue, or string type is converted
  to the packed representation, the item at index 0 occupies the MSBs."
- 6.24.3 L7516-7517: "An associative array type or class shall be illegal as a
  destination type."
- 6.24.3 L7522-7526: "If the destination type, dest_t, includes unbounded
  dynamically sized types, the conversion process is greedy ... any remaining
  dynamically sized items are left empty."
- 6.24.3 L7528: "For the purposes of a bit-stream cast, a string type is
  considered a dynamic array of bytes."
- 6.24.3 L7534-7537: "If both source_t and dest_t are fixed-size types of
  different sizes and either type is unpacked, then a cast generates a
  compile-time error. If source_t or dest_t contain dynamically sized types,
  then a difference in their sizes will issue an error either at compile time
  or at run time, as soon as it is possible to determine the size mismatch."
- 6.16 L5641-5642: "A string variable shall not contain the special character
  "\0". Assigning the value 0 to a string character shall be ignored."
- 11.4.14 L15472-15474: "If the target represents a dynamically sized
  variable, such as a queue or dynamic array, the variable is resized to
  accommodate the entire stream. If, after resizing, the variable is larger
  than the stream, the stream is left-aligned and zero-filled on the right."
- 11.4.14 L15476-15478: "the intermediate result between the two steps is
  never visible and therefore tools are free to implement it in any way that
  yields the same overall result."
- 11.4.14.1 L15494-15497: an unpacked array is streamed element by element,
  "Other unpacked arrays are processed in the order in which they would be
  traversed by a foreach loop"; L15523: anything else "shall be skipped (not
  streamed), and an error shall be issued."
- 11.4.14.2 L15544-15546: "If as a result of slicing the last (left-most)
  block has fewer bits than the block size, the last block has the size of the
  remaining bits; there is no padding or truncation."
- 11.4.14.3 L15563-15566: "If the source expression contains more bits than
  are needed, the appropriate number of bits shall be consumed from its left
  (most significant) end. However, if more bits are needed than are provided by
  the source expression, an error shall be generated."
- 11.4.14.4 L15600-15602: "the first dynamically sized item is resized to
  accept all the available data (excluding subsequent fixed-size items) in the
  stream; any remaining dynamically sized items are left empty."
- 11.4.14.4 L15623-15625: "The expression within the with is evaluated
  immediately before its corresponding array is streamed (i.e., packed or
  unpacked). Thus, the expression can refer to data that are unpacked by the
  same operator but before the array."
- 11.4.14.4 L15629-15630 and L15643-15645: a variable-size array "shall be
  resized to accommodate the range expression"; outside a smaller range "the
  remainder of the array is unmodified."
- 6.21 L7007-7008: "Automatic variables and members or elements of dynamic
  variables ... shall not be written with nonblocking, continuous, or
  procedural continuous assignments."
- 10.4.2 L13048: "It shall be illegal to make nonblocking assignments to
  automatic variables."

## Fixtures

Acceptance A01 (round trips):

- `roundtrip`: a byte string to a queue and back with `>>` and `<<8`,
  `<<byte`, a dynamic array, a reversed-bounds array `[3:0]`, a nested
  `[0:1][0:2]` array, slice sizes 3 and 5 (5 on 16 bits leaves a short left
  block), and `<<shortint` over a queue.
- `dynamic_extents`: greedy unselected dynamic targets, `with [i +: n]`,
  `[i:j]` and `[i]` ranges, left-aligned zero fill of a dynamic target, a
  cast to a dynamic type, zero bytes dropped from strings, and a struct with a
  dynamic member as a source and as selected member targets.

Acceptance A02 (capture and order, clean size errors):

- `capture_order`: overlapping source and destination (`q` on both sides,
  with and without `with`), swapped scalars, a self-reversed fixed array, and
  side-effecting `with` selectors on fixed, dynamic and `<<` targets that
  evaluate once.
- `err_short_source`, `err_cast_elements`, `err_cast_fixed`,
  `err_oversized_short`: size errors found at run time. Each prints its
  `before` line and stops with the runtime diagnostic; no target is written.

Acceptance A03 (illegal forms stay negative, no flattening):

- `large`: 1.6M-bit and 3.2M-bit streams of queues, fixed arrays, strings and
  their concatenation, beyond the 1,048,575-bit packed limit.
- `oversized_mixed`: queue operands, plain and `with`-selected, of oversized
  descriptor streams, and oversized whole fixed unpack targets, blocking and
  nonblocking. This form was the RTL-103 negative `neg_descriptor_container`.
- `neg_assoc_target`, `neg_assoc_cast`, `neg_real_member`, `neg_chandle`,
  `neg_event`, `neg_real_queue`, `neg_fixed_cast_size`, `neg_automatic_nba`,
  `neg_dynamic_element_nba`: illegal forms the frontend rejects.
- `unsupported_class_stream`, `unsupported_dynamic_record_cast`,
  `unsupported_dynamic_nba`: legal forms llg rejects with a diagnostic (see
  `docs/known_issues.md`).

Portable decisions S20-1 to S20-6 are in `tests/fixtures/sim/lrm_decisions/`
and `docs/lrm_decisions.md`.
