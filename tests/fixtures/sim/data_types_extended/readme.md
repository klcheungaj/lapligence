# Extended datatype conformance fixtures

black-box extended datatype fixtures.

Coverage: wide arithmetic, signed operations, packed layouts, net resolution,
  and the supported-width boundary.
- Runs: each fixture runs in both optimizer modes.
- Oracle: exact `PASS` output.
- Limits: width-boundary and oracle details are maintained with the fixture
  contracts.

Run serially:

```sh
cargo nextest run -E 'test(/^sim_data_types_extended::/)' --locked
```
