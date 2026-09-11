# Owned semantic database

- `database.rs`: the `NodeId` arena, Slang snapshot projection, normalized
  instance hierarchy, and read-only consumer API.
- `slang_types.rs`: validated type, array, packed-range, and aggregate
  projection from typed ABI tables.
- `domain.rs`: frontend-independent semantic enums.
- `validate.rs`: arena, root, side-table, embedded-reference, and cycle checks.

`Db::from_slang` copies no native data and performs no filesystem reads.
Unsupported facts stay explicit. Simulator, model, lint, and language-server
analysis share this database rather than querying Slang independently.
Variable metadata keeps Slang's resolved static or automatic lifetime separate
from the explicit source qualifier used for override diagnostics.
Packed ranges retain declaration identity, so same-named locals in unnamed
blocks and differently parameterized instances keep their own bounds.
Subroutine bodies are explicit arena references; consumers never infer a body
from the order of declarations or auxiliary statement children.

Event controls retain their expression, edge and optional `iff` condition as
validated owned node references, including mixed named-event lists.
Driver delays preserve single or separate rise/fall/turn-off expressions.
