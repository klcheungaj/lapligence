# Agent guide: docs

Human and cross-cutting documentation. Index: [source layout](source_layout.md#documentation).

## Placement

- Each fact lives in one place; other places link to it.
- Code comments hold facts about that code; docs and guides point to the file
  or item instead of restating it.
- `AGENTS.md` files: local ownership, rules, pitfalls, commands and pointers only.
- Module `readme.md` beside the source: that module's purpose, components and
  contracts, in concise prose or short bullets. Extend it rather than adding a
  parallel file; keep unique details when shortening it.
- `docs/*.md`: user guides and cross-cutting contracts (for example
  [cross-platform handling](cross_platform.md)).
- `tests/readme.md`, `tests/ci.md`: test methods, limits, commands and CI rules.
- Simulator feature status: only `sim_features.md`.
- Dated investigations, plans and run evidence: ignored `../persistence/`, never
  `docs/`.
- READMEs and docs never link to `AGENTS.md`; link to source, other readmes or
  docs.
- `docs/` is ignored by default: whitelist a new tracked doc in `.gitignore` and
  add it to the index.

## Accuracy

- Keep docs aligned with implementation and tested behavior. Distinguish
  simulator, shared-core and LSP contracts; simulator behavior is not an LSP
  guarantee.
- LSP docs keep: stdio transport; independent multi-root workspaces; per-root
  `llg.toml` v1 and client config-file overrides; `.v`/`.sv` units and
  include-only `.vh`/`.svh`; root-relative include/exclude precedence; include
  authorization under source/include directories; read-only shadow buffers in a
  private process temp tree; lint configuration; dynamic config/source/include
  watchers; configurable stdout-safe logging.
- State that macro-generated or dynamic include paths are unsupported rather than
  implying complete preprocessor resolution.
- Roadmap and security claims are evidence-based, with validation commands and
  residual risks when material to users. fmt/check/clippy alone are not
  validation, and untested Windows/macOS matrix legs do not establish platform
  support ([CI](../tests/ci.md)).
