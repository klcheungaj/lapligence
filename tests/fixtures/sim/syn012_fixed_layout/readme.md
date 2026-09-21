# SYN-012 fixed aggregate matrix

- `matrix.sv` exercises packed structs, equal-width packed unions, fixed
  unpacked structs, untagged unpacked union views, and nested arrays of
  records through declaration, local, member, row, equality, conditional,
  assignment, and return contexts.
- The matrix covers signed `logic`, `bit`, reversed bounds, and leaf widths 1, 7,
  8, 31, 32, 33, 64, 65, and 129. The Rust suite runs every width through
  `llg` and `llg --no-opt` with exact output.
- The rejection fixtures keep unequal packed union widths, distinct unpacked
  record identities, native conditional members, and an invalid unpacked-union
  bit-stream cast as single-fault controls.
- Normative references are IEEE 1800-2009 §§6.22, 6.24.3, 7.2–7.4, 10.8,
  and 11.2.2; the local section index is
  `docs/specification/spec-reference-sv.md`.

This is finite execution evidence for SYN-012, not a claim of complete
aggregate or bit-stream conformance.
