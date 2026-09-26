# Datatype edge fixtures

self-checking SystemVerilog datatype-edge cases.

Coverage: selected boundaries, including packed-state conversion and wide
  indices.
- Runs: applicable cases run in both optimizer modes.
- Oracle: self-checking output must match the expected fixture result.
- Limits: LRM-derived boundary behavior is intentionally narrower than a full
  datatype-conformance claim.
