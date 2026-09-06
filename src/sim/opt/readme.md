# IR optimization

- **Facade:** `opt.rs` exposes optimization configuration and entry points.
- **Passes:** `passes.rs` walks and transforms typed IR conservatively.
- **Invariants:** passes preserve table indices and lowering-time sensitivity
  sets; they do not recompute wake behavior.
- **Validation:** callers validate the model after enabled passes complete.

See [the simulator README](../readme.md) for this stage's place in the model
pipeline.
