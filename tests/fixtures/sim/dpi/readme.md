# DPI-C scalar imports

These fixtures cover the bounded H27 import ABI: scalar `bit`/`logic`, the
two-state integral atoms, `real`/`shortreal`, `chandle`, and `string`. The
foreign C identifiers are explicit aliases so the generated prototypes can be
checked independently from the HDL names. Packed vectors, unpacked aggregates
and open arrays are covered by the SIM-040 fixtures in
`../feature_completion/sim_040/`. `unsupported_native_member.sv` keeps the
remaining explicit rejection of an unpacked aggregate with a `real` member;
`ref`, exports, and suspending/context callbacks remain outside these
fixtures.
