# net_interval fixtures

Undriven net-array cells and large, sparsely driven net arrays
(`tests/sim_net_interval.rs`). Expected values are hand-computed from the
net type rules: an undriven `wire` cell reads `z`, `tri0`/`tri1` read their
pull value, `supply1` reads `1`, and a whole-array inout peer takes the
collapsed type of IEEE 1800-2009 Table 23-1.

- `undriven_cells.sv`: undriven cells of several net types, a collapsed
  `tri1` inout formal, `%v` strengths, dynamic reads and force/release of an
  otherwise undriven cell.
- `inout_65537.sv`: whole net-array inout port, 65,537 cells, two drivers.
- `udp_200000.sv`: one UDP output bit in a 200,000-cell `wire [7:0]` array.
- `procedural_cell.sv`: nearest illegal form, a procedural write to a net cell.
