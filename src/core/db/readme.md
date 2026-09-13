# Owned semantic database

- `database.rs`: the `NodeId` arena, Slang snapshot projection, normalized
  instance hierarchy, and read-only consumer API.
- `slang_types.rs`: validated type, array, packed-range, and aggregate
  projection from typed ABI tables.
- `domain.rs`: frontend-independent semantic enums.
- `validate.rs`: arena, root, side-table, embedded-reference, and cycle checks.

`Db::from_slang` copies no native data and performs no filesystem reads.
Native semantic kind/detail metadata is retained beside the frontend-neutral
node kind so simulator coverage can reject an unknown reachable executable
record with its source span instead of silently treating it as `Other`.
Unsupported facts stay explicit. Simulator, model, lint, and language-server
analysis share this database rather than querying Slang independently.
Variable metadata keeps Slang's resolved static or automatic lifetime separate
from the explicit source qualifier used for override diagnostics.
Packed ranges retain declaration identity, so same-named locals in unnamed
blocks and differently parameterized instances keep their own bounds.
Subroutine bodies are explicit arena references; consumers never infer a body
from the order of declarations or auxiliary statement children.

Event controls retain their expression, edge and optional `iff` condition as
validated owned node references, including mixed named-event lists and fixed
unpacked event-array selects. Named-event declarations retain identity-bearing
array metadata for hierarchy and runtime-indexed lowering; nonblocking
named-event triggers retain their mode and supported delay/event/repeat timing.
Unsupported timing or resizable event-storage nodes remain explicit owned
references for source-located lowering rejection.
Driver delays preserve single or separate rise/fall/turn-off expressions.
