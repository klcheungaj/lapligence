# RTL-018 library, configuration and bind fixtures

`compose.sv` (with `compose_config.sv`, `compose.map` and `lib/lane_*.sv`) is
the A01 composition: the configuration overrides `SCALE`, binds one lane in a
generate-if scope to the `gate` cell with its own `GAIN`, and leaves the
generate-for lanes on `rtl`. Instance binds inject `rtl018_tap` into one of
each, an interface bind sums every generate-for interface memory, and a
131072 x 9-bit memory (above the packed-value limit) crosses the configured and
bound ports. `compose.out` is derived by hand from those values (x = 2).

`units.sv` with `units_root.map` (which includes `units/child.map`) covers
compilation-unit modes for library sources, map-local macro isolation,
command-line defines and include precedence (`global/mark.vh` versus the
library's `units/headers/`). `binding.sv` covers the library search order, a
configuration `use` clause, `%l`, and missing/unreachable/ambiguous bindings.

The `bind_*.sv` files are single-fault negatives (package, class, generate-block
and program targets, module into interface, repeated bound names) plus the
legal interface-into-module bind. `include_escape.sv` names an existing file
outside every admitted root. `witness_*.v` and `witness/` are adopted FND-002
witnesses; `witness/empty/` intentionally holds no headers.
