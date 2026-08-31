# tests — repository validation

## Scope

Tests cover both the shared Surelog/UHDM processing layer and the simulator;
the LSP has an additional process-level stdio acceptance suite.

## LSP acceptance tests

- `lsp_stdio.rs` must speak only framed standard LSP JSON-RPC over the `llg`
  process's stdio.  Never parse or depend on debug output from stdout.
- Cover the new contract: per-root `llg.toml` config (default and
  client-supplied overrides via `llg.configFiles`), config reload without
  restart, `.v`/`.sv` compilation-unit discovery with `.vh`/`.svh`
  include-only behavior, include/exclude precedence, independent multi-root
  ownership, longest-root ownership transfer on workspace folder add/remove,
  standard watched-file notifications (config files, `.v`/`.sv` units, and
  resolved arbitrary-extension include dependencies) re-registered after
  successful analyses, dep events routed to every dependent root,
  shared-file aggregation (identical findings once; conflicting findings
  labeled `[<root-name>]`; owner-wins semantic tokens/hover labeling),
  per-root lint configuration, unsaved source/header buffers, project-wide
  diagnostics published for never-opened files (and refreshed when such a
  file is fixed on disk via a watched-file event), include
  authorization under configured directories, last-good navigation during
  failed compiles, read-only shadow staging, and no Surelog artifacts
  (`slpp_all/`, logs) inside a workspace used as the server CWD.
- Fixtures live under `tests/fixtures/lsp/`, use schema
  `llg.lsp.fixture/v1`, ship an effective `llg.toml` per root, and source
  files retain the `// llg-lsp-fixture:` header.

## Surelog integration

Tests that invoke Surelog must run from a fresh temporary working directory,
because Surelog writes `slpp_all/`. Clean up temporary trees after each test.
Prefer the shared Rust compile/session APIs and keep assertions deterministic.
