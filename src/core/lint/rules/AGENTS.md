# Agent guide: src/core/lint/rules

One module per rule; the rule's behavior is in its module docs and
[readme](readme.md). API and configuration: [lint guide](../AGENTS.md).

## Directory map

- `mod.rs`: `default_rules`, the authoritative stable registry order.
- `analysis.rs`: shared owned-db helpers (read/write collection, widths,
  scope/instance iteration, port links, `port_unconnected`).
- `<rule>.rs`: one rule each (`unused.rs`, `width.rs`, `undriven.rs`, ...).

## Conventions

- Keep shared graph/data-flow helpers in `analysis.rs`; never duplicate them
  across rules.
- A new rule needs a `default_rules` entry plus focused behavior and
  configuration tests, with triggering and nearby quiet cases.
- Rules consume owned data through `LintCtx`; no live native traversal or I/O.
- Width rules skip operands whose width is unknown.
- When two rules could report one location, document the split in both modules
  (`case_default.rs` vs `latch.rs`; `mixed_assign.rs` vs `blocking_in_ff.rs` and
  `nba_in_comb.rs`).
- Identical findings from cloned instances are source-deduplicated where the
  rule says so; keep that when refactoring.

## Pitfalls

- `naming-style` uses one built-in convention; there is no naming configuration.
