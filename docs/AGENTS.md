# Project documentation

Keep documentation aligned with implementation and tested behavior. Distinguish
simulator, shared-core and LSP contracts; simulator behavior is not an LSP guarantee.

## Ownership

- Module READMEs explain purpose, components, requirements and interactions in
  concise, human-facing prose, short bullets or a small hierarchy.
- Keep detailed invariants and maintenance instructions in the owning `AGENTS.md`.
  Preserve unique details when shortening a README. READMEs must not link to
  `AGENTS.md`; link to source, other READMEs or product documentation instead.
- Update simulator feature status only in `sim_features.md`. Put testing methods,
  limits and commands in `../tests/readme.md`, using concise hierarchical bullets.
- Keep dated investigations, plans and run evidence in ignored `../persistence/`,
  not transient reports in `docs/`.

## Product accuracy

LSP documentation must retain stdio transport; independent multi-root workspaces;
per-root `llg.toml` v1 and client config-file overrides; `.v`/`.sv` units and
include-only `.vh`/`.svh`; root-relative include/exclude precedence; include
authorization under source/include directories; read-only shadow buffers in a
private process temp tree; lint configuration; dynamic config/source/include
watchers; and configurable stdout-safe logging. State unsupported macro-generated
or dynamic include paths rather than implying complete preprocessor resolution.

Keep roadmap/security claims evidence-based. Include validation commands and
residual risks when material to users. [Validation](../tests/AGENTS.md) owns CI,
release caveats, serialized tests, generated-runtime sanitizers and dependency
audits; fmt/check/clippy alone are insufficient. Untested Windows/macOS matrix
legs do not establish platform support.
