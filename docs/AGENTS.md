# docs — project documentation

Keep documentation synchronized with the implementation and its tested
behavior.  The repository contains separate simulator, shared-core, and LSP
contracts; do not describe simulator-only behavior as an LSP guarantee.

## Module documentation

- Keep module READMEs human-facing, clear, and concise: use short bullets or
  a small hierarchy for purpose, components, and interactions.
- Put detailed implementation contracts, invariants, and maintenance
  instructions in the owning `AGENTS.md`, preserving unique details when
  shortening a README.
- Do not reference or link to `AGENTS.md` from READMEs. Human-facing links may
  point to source files, other READMEs, or product documentation in `docs/`.

## Product accuracy

For LSP documentation, record the standard stdio protocol, independent
multi-root workspaces, per-root `llg.toml` v1 configuration (with
client-supplied config-file overrides), `.v`/`.sv` compilation-unit discovery
with `.vh`/`.svh` include-only behavior, root-relative include/exclude
precedence, include authorization under configured source/include
directories, read-only shadow-buffer behavior (private per-process temp
tree), `llg.toml` lint configuration, dynamic config/source/include
watchers, and configurable stdout-safe logging.  Explicitly call out
unsupported macro-generated or dynamic include paths rather than implying
that all preprocessor behavior is resolved.

Keep roadmap and security-audit status evidence-based. Mention validation
commands and residual risks when they materially affect users. Use
[../tests/AGENTS.md](../tests/AGENTS.md) for CI jobs, validation commands and
release-platform caveats; the gate includes serialized tests, generated-runtime
sanitizers and dependency audit, not just fmt/check/clippy. Do not imply that
untested Windows/macOS release legs establish platform support.

## Simulator documentation ownership

- Maintain all simulator feature-status updates only in `sim_features.md`.
- Put testing methodology, limitations and commands in `../tests/readme.md`,
  using concise hierarchical bullets.
- Keep dated investigations, plans and validation evidence in ignored
  `../persistence/`; do not publish separate transient status reports in docs.
