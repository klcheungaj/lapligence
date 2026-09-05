# sim/ir

Validation of the typed simulator IR at lowering and optimization boundaries.
The validator owns no lowering or emission policy: it checks table references,
widths, constants, array shapes, and process registrations before the backend
indexes those structures.

Keep new cross-structure invariants here and cover every new IR variant in the
recursive validation walk.
