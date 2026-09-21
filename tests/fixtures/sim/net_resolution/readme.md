# Net resolution and true net-alias fixtures

The fixtures in this directory cover IEEE 1364-2001 §3.7.2, §6.1, §12.4 and
IEEE 1800-2009 §6.6, §10.3, §10.11 and §23.6: the two-driver resolution truth
matrix, multi-site structural drivers above the retired 16-slot runtime
ceiling, disjoint fixed-array net elements, plain-wire driver conflicts, and
whole/constant-selected/concatenated/ascending aliases with continuous and
primitive drivers, port links, force/release, and conflicting drivers.
Hierarchical continuous-driver fixtures cover parent-to-child, selected,
generated-name, upward-qualified and R05-connected targets; procedural net
writes remain negative controls. The owning suite runs every fixture through
`llg` and `llg --no-opt`.
