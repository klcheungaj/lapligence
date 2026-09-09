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
Subroutine bodies are explicit arena references; consumers never infer a body
from the order of declarations or auxiliary statement children.
