# Owned design database

- Purpose: capture UHDM once and expose an immutable design snapshot.
- Components:
  - `database.rs`: arena, shared builder state, and `Db::build`.
  - [`capture/`](capture/readme.md): private VPI traversal by domain.
  - `domain.rs`: owned enums for VPI properties, including unknown values.
  - `validate.rs`: root and metadata-reference validation.
- Consumers: model, lint, simulator, and language-server analysis.
- Output: read-only nodes, elaborated ranges, and declaration metadata.
