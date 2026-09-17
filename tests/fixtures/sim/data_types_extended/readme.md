# Extended datatype conformance fixtures

- Purpose: black-box extended datatype fixtures.
- Coverage: wide arithmetic, signed operations, packed layouts, net resolution,
  and the supported-width boundary.
- Execution: each fixture runs in optimized and unoptimized models.
- Result: exact `PASS` output is required.
- Limits: width-boundary and oracle details are maintained with the fixture
  contracts.

Run serially:

```sh
cargo nextest run --locked --test sim_data_types_extended
```
