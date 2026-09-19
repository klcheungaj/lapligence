# Net resolution and true net-alias fixtures

The fixtures in this directory cover IEEE 1800-2009 §6.6-6.7 and §10.11: the
two-driver resolution truth matrix, multi-site structural drivers above the
retired 16-slot runtime ceiling, disjoint fixed-array net elements, plain-wire
driver conflicts, and whole/constant-selected/concatenated/ascending aliases
with continuous and primitive drivers, port links, force/release, and
conflicting drivers. The owning suite runs every fixture through `llg` and
`llg --no-opt`.
