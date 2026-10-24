# Random-stream fixtures

- `sim_random_streams.rs` runs `random_streams.sv` in both optimizer modes.
- It checks `$urandom` seed input, `$urandom_range`, process seed/state replay
  and fork isolation through relationships and range invariants, never an
  unspecified numeric sequence.
- Anchors: local IEEE 1800-2009 §§18.13–18.14 (H07).

```sh
cargo nextest run -E 'test(/^sim_random_streams::/)' --locked
```
