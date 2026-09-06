# Datatype edge fixtures

- Purpose: self-checking SystemVerilog datatype-edge cases.
- Coverage: selected boundaries, including packed-state conversion and wide
  indices.
- Execution: applicable cases run with optimization disabled and enabled.
- Result: self-checking output must match the expected fixture result.
- Limits: LRM-derived boundary behavior is intentionally narrower than a full
  datatype-conformance claim.
