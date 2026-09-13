# DPI-C scalar imports

These fixtures cover the bounded H27 import ABI: scalar `bit`/`logic`, the
two-state integral atoms, `real`/`shortreal`, `chandle`, and `string`. The
foreign C identifiers are explicit aliases so the generated prototypes can be
checked independently from the HDL names. `ref`, packed vectors, open arrays,
exports, and suspending/context callbacks remain outside H27 and are rejected
or deferred to later tasks.
