# Net resolution and true net-alias fixtures

The fixtures in this directory cover IEEE 1364-2001 §3.7.2, §6.1, §12.4 and
IEEE 1800-2009 §6.6, §10.3, §10.11 and §23.6: the two-driver resolution truth
matrix, multi-site structural drivers above the retired 16-slot runtime
ceiling, disjoint fixed-array net elements, plain-wire driver conflicts, and
whole/constant-selected/concatenated/ascending aliases with continuous and
primitive drivers, port links, force/release, and conflicting drivers. SYN-010
adds fully indexed fixed net-array elements, reversed ranges, multidimensional
packed projections, and aliases crossing an admitted inout link; its negative
fixtures keep self, duplicate, incompatible-type, and variable aliases as
single-fault frontend controls.
Hierarchical continuous-driver fixtures cover parent-to-child, selected,
generated-name, upward-qualified and R05-connected targets; procedural net
writes remain negative controls. The owning suite runs every fixture through
`llg` and `llg --no-opt`.

`generate_offset_drivers` drives parts of vector nets from loop generate
blocks at offset positions (`buf (y[6 + i], a)`, `assign w[2 + i] = ...`,
`drv dr (.o(v[4 + 2 * i +: 2]), ...)`), on `wire`, `tri` and `wand` nets, a
net with a net delay and next to delayed gates. IEEE 1800-2009 §6.6
(`SystemVerilog-1800-2009.txt` L4740-4741): "If no driver is connected to a
net, its value shall be high-impedance (z) unless the net is a trireg"; §6.7
(L5198-5199): "The default initialization value for a net shall be the value
z. Nets with drivers shall assume the output value of their drivers." So every
bit no generated driver writes prints `z`, and each driven bit prints its
driver's value (`y` bits 9:6 are 1, `u` bit 10 is 1 and bits 4:3 are 0). At
time 0 the driven bits of the delayed gates and of the delayed net hold X
until a driver writes them (decisions T0-D3 and S2-D10), while the undriven
bits of `e` are z; the net delay of `d` makes its undriven bits z at time 2
with the driven ones (S2-D3). Output agrees with industry-standard behaviour.
