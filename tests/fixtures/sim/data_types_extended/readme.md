# Extended datatype conformance fixtures

- Purpose: black-box extended datatype fixtures.
- Coverage: wide arithmetic, signed operations, packed layouts, net resolution,
  and the model-width boundary.
- Execution: each fixture runs in optimized and unoptimized models.
- Result: exact `PASS` output is required.
- Limits: width-boundary and oracle details are maintained with the fixture
  contracts.

Run serially:

```sh
cargo test --test sim_data_types_extended -- --test-threads=1
```
