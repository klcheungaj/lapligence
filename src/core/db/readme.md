# core/db

Validation for the fully owned UHDM arena produced by the canonical VPI walk.
It checks arena parent/child links, typed root collections, and side-table
references before the database crosses into model, lint, LSP, or simulator
processing.

Capture policy remains in `core::db`; keep this submodule limited to owned-data
invariants and free of VPI access.
