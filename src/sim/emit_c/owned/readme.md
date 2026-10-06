# Structured owned-value emission

The renderer replaces nested allocating expressions with ordered C11 statements.
`Value` records result code, shape and ownership; registered expression scopes and
lexical cells support cleanup on normal exit, suspension and cancellation.
Non-escaping packed, real and native local cells use caller-owned scope nodes
and payloads; escaping or unproven cells retain stable heap owners. Feature guards reject unrepresented ownership paths.

## Responsibility map

| Components | Responsibility |
| --- | --- |
| `owned.rs` | Result records, reusable slots, binding/assignment casts. |
| `expressions.rs`, `control.rs`, `system.rs` | Expressions, branches and system operations. |
| `repeated_values.rs` | Typed repeated concatenation appends with per-iteration evaluation and cleanup. |
| `stores.rs`, `statements.rs` | Captured lvalues/masks, writes and lexical/loop cleanup. |
| `pca_batches.rs` | Adjacent typed PCA assignment runs, registered helper temporaries and ordered table loops; contracts in the [emitter guide](../AGENTS.md#procedural-continuous-assignment-batches). |
| `calls.rs`, `pure_calls.rs`, `events.rs`, `formatting.rs` | Call/result/address ownership, callback inlining, events and output. |
| `cached_fields.rs` | C-local mirrors of resume-stable frame fields, reloaded after every suspension. |
| `captures.rs`, `event_waits.rs`, `model/callbacks.rs` | Activation frames, wait contexts and evaluator callbacks. |
| `model.rs`, `model/` | Storage, zero-time initialization, procedures and host lifecycle API. |
| `assertions/`, `assertions.rs`, `assertion_tasks.rs`, `clocking.rs`, `qualifiers.rs` | Sampled/local assertion values, controls, clocking operands and branch diagnostics. |
| `native.rs`, `strings.rs`, `objects.rs` | Native payloads, byte strings and typed handles. |
| `native_values.rs` | Descriptor tables, registered roots and leaf addresses of native record values (SIM-003). |
| `containers.rs`, `containers/` | Ordered operands, keys, item snapshots and mutations. |
| `input.rs`, `native_tasks.rs` | File/plusarg targets, text, queue/random calls and VPI arguments. |
| `native_access.rs`, `references.rs`, `mailboxes.rs` | Member/ref resolution, retained identities and message transfers. |
| `streaming.rs` | RHS snapshots, sequential selectors, bounds checks and publication. |
| `tests.rs`, `tests/` | Structural, ownership-boundary and actual IR-to-C regressions. |

Packed scanner targets through ref formals use synchronous borrowed views of the
canonical descriptor and captured numeric selection plan. Typed declarations
keep the view alive through argument setup; scanning does not retain the view
beyond the call, and failed conversions leave the selected destination unchanged.

## Verification

[Ownership validation](../../../../tests/readme.md#dynamic-ownership-validation)
distinguishes handwritten C probes from Rust-emitted models and public HDL runs.
Scope/callback tests check every retained descriptor field; array and stream tests
check index cleanup, invalid handles and source-size checks before publication.
Component success is not full HDL or platform acceptance.

```sh
cargo test --lib --no-default-features sim::emit_c
cargo test --lib --no-default-features structured_owned_model_
cargo test --locked --no-default-features --test sim_a_m sim_dynamic_ownership::
```

See [runtime ownership](../../rt/value/ownership.md) and
[feature boundaries](../../../../docs/sim_features.md#dynamic-value-migration-acceptance-boundary).

`udp.rs` emits shared immutable combinational truth tables and one borrowed runtime
evaluation per gate. Row count grows data, while executable body size stays fixed.
