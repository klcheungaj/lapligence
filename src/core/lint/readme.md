# Shared linter

`core::lint` runs the same owned-design checks for the language server and
the `llg` simulator driver. Rules consume `core::db::Db` and
`core::model::DesignModel`; they do not invoke VPI, reparse source text, or
depend on LSP types.

## Default rules

All 24 rules are enabled by default. Each finding has a stable rule ID and a
default severity that can be overridden by either frontend.

| Rule | Detects |
|---|---|
| `unused-signal` | signals that are never read or used |
| `width-mismatch` | assignments and port links with different known widths |
| `incomplete-case` | exact cases without a default in combinational/latch logic |
| `combinational-loop` | combinational feedback paths |
| `multi-driver` | signals driven by multiple processes |
| `casez-misuse` | overlapping wildcard case items and constant selectors |
| `if-latch` | incomplete combinational `if` assignments that may infer a latch |
| `naming-style` | configured naming-convention violations |
| `blocking-in-always_ff` | blocking assignments in clocked processes |
| `nba-in-always_comb` | nonblocking assignments in combinational processes |
| `unused-parameter` | parameters that are never referenced |
| `implicit-net` | nets auto-created from undeclared identifiers |
| `case-default-missing` | cases without a default outside `incomplete-case`'s domain |
| `comparison-width-mismatch` | comparisons whose known operand widths differ |
| `unconnected-port` | omitted and explicitly open instance ports |
| `mixed-assignments` | blocking and nonblocking assignments in one process |
| `undriven-signal` | read signals with no known source |
| `incomplete-sensitivity-list` | explicit level-sensitive blocks missing a body input |
| `out-of-range-select` | statically provable packed or unpacked select overflow |
| `xz-logical-equality` | `==`/`!=` used directly with an X/Z literal |
| `duplicate-case-item` | repeated literal labels in an exact case |
| `empty-implicit-sensitivity` | plain `always @*` blocks that write but have no resolved body signal reads |
| `assignment-in-condition` | assignment expressions consumed as truth predicates |
| `casex-statement` | `casex` statements in process and function/task bodies |

The registry in `rules/mod.rs` is authoritative. Rules deliberately skip
uncertain cases instead of guessing; see `AGENTS.md` in this directory for
the exact analysis boundaries and known false negatives.

## Simulator configuration

`llg --lint` runs the rules before code generation. `--lint-json [path]`
emits a report without running the simulation. A `llg-lint.toml` file passed
with `--lint-config` uses one section per stable rule ID:

```toml
[rules.undriven-signal]
enabled = false

[rules.incomplete-sensitivity-list]
severity = "error" # error | warning | info
```

Missing entries remain enabled at their rule-defined severity. Unknown rule
IDs and unsupported keys are configuration errors.

## Language-server configuration

Each workspace root configures the same registry through its schema-v1
`llg.toml`:

```toml
[lint]
enabled = true

[lint.rules.duplicate-case-item]
severity = "info"
```

LSP findings use source `llg-lint` and carry the stable rule ID as the
diagnostic code. Configuration edits are applied by the existing per-root
hot-reload path.
