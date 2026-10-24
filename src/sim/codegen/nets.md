# Lowering: nets and procedural drivers

Inout collapse: `lowering.rs` module docs.
Input/output links feeding a group retain their own structural driver sites.
Resolve net-type/delay selection through `net_collapse`/`port_net_types` before
storage union; same-net-type alias checks precede more permissive port rules.
Parent connections precede descendants, and each hierarchy depth collapses as
one `port_batch` against pre-batch types, so instance/port order never selects
a type: winners reduce to the non-dominated set, a warning-only tie takes the
first Table 23-1 column with a located warning, and every winning declaration
of that type owns the delay. A uwire in a collapsed network keeps one
structural driver per bit network (`note_uwire_drivers`, conservative for
unmapped targets); the frontend patch stops counting the inout connection
itself as a uwire driver, for each uwire operand of a concatenated actual and
for the internal net of an `inout uwire` formal.
Whole connections keep declaration-level metadata; selected connections map
physical electrical bits. Reject incompatible/non-net members and unsupported
resolution/sensitivity, never recover an unresolved driver from source text.

Give continuous/declaration/gate sites deterministic independent contribution slots,
sized at elaboration without a fixed driver ceiling. Selected contributions restart
from Z each evaluation. Synthetic slots have no waveform names; reads/dependencies
observe resolved cells only. Wire/wired/pull/supply resolution retains both
strength endpoints of X; a known drive wins only when stronger than every opposite
possibility. High-Z adds no drive. Wired-AND 0 and wired-OR 1 dominate X; all-Z
sources resolve Z unless a default source applies. Pull/supply defaults are
strength-bearing, not unconditional values. Preserve scalar strengths; explicit
vector (including selected-bit) and supply-net continuous strengths reject; gate
instance-array strengths stay legal. Undelayed enable gates split into a
strength0-only and a strength1-only slot so an unknown enable drives L/H; delayed
ones keep one slot (X) so each transition keeps a single delay. An omitted input
under `unconnected_drive` is a pull-strength slot of its port on every net or
net-array formal (variables receive the value only); input net-array links and
gate outputs on net-array cells publish through their port/gate cell slots.
`%v` on a net, its constant projections, aliases or net-array cells reads a lazily
created strength-view signal (`collection/strength_views.rs`); monitors depend on
that view, so strength-only changes reach them without waking value readers. Reject trireg before storage collection,
including undriven arrays, and dynamic net selectors. Keep disjoint constant
uwire selections legal; overlapping drivers reject.

Combinational UDP instances lower scalar input expressions directly into
`UdpEval` (no per-evaluation input locals) and intern each definition once per
model through a hash of its typed rows. Terminals select from larger objects
through the ordinary expression/lvalue paths; a bit select of a computed value
(operator, conditional, cast or slice) is numbered by that value's own packed
type, as Slang's instance-array slicing requires. Vector or aggregate terminals
of a single instance reject.

Pass switches (`collection/switches.rs`) lower each terminal to one net-group
bit (through inout collapse and true aliases; `Db::primitive_array_position`
selects an array element's bit, since Slang binds bidirectional terminals
whole). An undelayed control that is one bit of a packed net or variable
(`path_terminal`) becomes an `IrSwitchControl` row the runtime watches, so
arrays of switches emit no process per switch; any other controlled switch
gets a hidden process issuing `IrStmt::SwitchControl` with its normalized
control (a delayed control goes through an inertial shadow signal that starts
X). Terminals keep their
never-written structural driver slot. `finish_switch_network` numbers the
nodes of each statically connected network contiguously; wired-logic member
nets, wide terminals and SDF-annotated switches reject with their location.

Sequential UDP instances lower to one `UdpStep` at the start of the
continuous gate body followed by the shared publication of `UdpState`; each
instance owns one `IrModel::udp_states` record, so instance-array elements
keep independent states. The definition's typed rows come from the Db
(`core::db::UdpTable`, located rows); a row the IR cannot represent fails
with its source location. The initial value is published at time zero
without the instance delay: lowering sets `delayed_driver_start` so the
delayed driver starts at the initial value instead of X (S2-D4), and the
first step runs at time zero with every prior input X (decision S3-D4).

`collection/net_partition.rs` forms maximal affine runs of canonical electrical
roots for fixed arrays and selected/aliased scalar or vector nets. Adjacent
positions must have the same declared members, structural source/terminal sites,
force/release membership and resolved net type/delay owners; each member and
source offset advances by one. Reversed/permuted or opaque projections retain
bit groups. Include driver ranges before partitioning, and extend that source
inventory for new topology forms. Ordinary whole vector groups already resolve
word-parallel and retain their existing path. Keep per-bit declared-view bindings
with a group-bit offset (the emitter renders consecutive bits as one run-length
alias part); electrical width must never change an HDL storage shape.
Whole net-array inout ports, rows and slices union each formal cell with its
left-to-left actual cell (`net_array_inout_pairs`) in both storage and type plans.
`collection/net_cells.rs` classifies cells before that: a cell joins the
bit-level path only when a structural source, true alias, selected inout
connection or force/release target reaches it or a whole-array inout peer of
it; a new topology form must be added to its inventory too, and publication
rejects a driver reaching an unclassified cell.
Contribution projection gathers contiguous RHS runs with part selects and Z runs,
then concatenates only disconnected runs. Conversion precedes projection. Array
publication binds every bit to this partition and rejects a missing root.
Record net-array member selections map declared cell coordinates and recursive
member offsets through `array_net_selection` before this same partitioning;
member boundaries split runs when their structural driver sites differ.

Fixed net arrays, selected ports and interfaces share canonical bits; array
publication notifies array dependencies. Keep the signal reverse index aligned
with every insertion and rebuild it after port canonicalization. Alias whole-net
views resolve selected positions lazily; retain eager first-error behavior when
endpoint validation cannot prove the complete affine range valid. True aliases
flatten MSB-to-LSB, with
ascending indexed-minus starts checked as base-width+1 before materialization.
Retain self/duplicate/net-kind/cross-scope restrictions. Hierarchical/concatenated
continuous targets use resolved sites, not function/task-output or procedural-net
write paths.

Delayed contributions start X; genuinely driverless nets start Z. Inertial updates
own one Active event per site/array element without suspending their evaluator.
Compare pending contribution values, not resolved nets; unchanged values keep
original deadlines, changed values cancel, return to current value cancels without
replacement. Preserve full single/rise-fall/three-way delays. A one-bit target
follows the from/to table (a change to X takes the smallest delay); a wider
target takes one delay for its whole value (nonzero to zero falls, all-Z turns
off, else rises; V §6.1.3). A net delay schedules the whole resolved vector,
or each separately driven part, with the delay selected by the new least
significant bit (S2-D3); `net_delay_layout` derives the parts from every
driver site after initialization steps exist and marks driven delayed nets and
module-path destinations to start X (S2-D10, A7-D6); every other driven wire,
wand or wor group records the bit runs its sites write (`driven_runs`), which
start X until the first write of a driver of those bits (T0-D3); a constant driver's z bits are
left out (T0-D4). A net
declaration assignment's delay is that driver's delay, never a net delay (the
capture gives it only to the assignment). Selected delayed drivers and
net-array cells start X on their own bits (`initialize_delayed_lhs`). Delayed
concatenation or alias-split targets: `delayed_shadow_driver` docs (`delays.rs`);
pattern members are separate drivers. A SystemVerilog runtime-valued driver delay
(`driver_delay`, `IrInertialDelay::Runtime`) is evaluated by the driver when it
schedules; a runtime net delay registers one `SetNetDelay` updater process,
emitted before every Comb process and sensitive to its operands. Verilog-2001
rejects both (V §7.14.1). Every min:typ:max expression, folded
or lowered, selects through `min_typ_max_operand`.
Module paths lower after the net groups and their structural driver
sites exist (`collection/module_paths.rs`): one `IrModulePathSet` per
instance, terminals resolved to storage-bit ranges, delays folded through
`path_delay_ticks` (negative is zero) and expanded to twelve transitions,
one hidden `ModulePathStamp` SensLoop process on the sources. A destination
must be a whole net group driven only inside the instance subtree (Net-member
slots of collapsed inout groups are storage, not drivers); IEEE 1364-2001
also allows one driver site. Variable destinations fail with their location. A condition is lowered once per declaration in the
instance scope (`IrModulePathSet::conditions`); edges map to `IrPathEdge`;
paths with exactly an `ifnone` path's terminals share a group, an
unconditional duplicate of an `ifnone` path and all but the last duplicate
`ifnone` are dropped (A8-D3). `PATHPULSE$` limits (first input and output
terminal, else module-wide; error below reject lowers reject; ticks through
`path_limit_ticks`) attach per declaration, and pulse-style declarations set
per-bit destination flags, first declaration of each kind winning.
`CodegenOptions::path_pulse` (the driver's `--pulse-*`/`--show-cancelled`)
becomes `IrModel::path_pulse`. Timing checks lower after the module paths
(`collection/timing_checks.rs`): one `IrTimingCheckSet` per instance,
terminals through the module-path terminal resolver, roles per Tables
31-1..31-12 (`$removal` by Table 31-4 in both editions, A9-D2), the derived
`$period`/`$width` data event, limits through `timing_limit_ticks` (module
unit, min:typ:max, negative is zero), conditions lowered once per set and
flagged nondeterministic for a top-level `==`/`!=`, duplicates dropped with a
warning (A9-D6), and one hidden `TimingCheckStep` SensLoop process per
design on the hidden doorbell variable (`IrModel::timing_doorbell`); the
runtime toggles notifiers itself. `$setuphold`/`$recrem` lower to two rows
(before/after the reference, with the timestamp and timecheck conditions as
`condition2`); with `IrTimingCheckOptions::negative` the instance's limits
are settled (negative sum to 0, sum 0 or 1 dropped), the delays solved per
terminal and edge class (`solve_signal_delays`, ADV-010 A10-D3/D5), delayed
signals lowered as `IrTimingLine`s into explicit nets (a structural driver
slot) or hidden variables, and every non-skew row moved onto the delayed
terminals. Invalid checks fail with their location whatever the options;
`CodegenOptions::timing_checks` (`--no-timing-checks`, `--no-notifiers`,
`--no-timing-check-messages`, `--neg-timing-checks`) then omits the sets,
the toggles or the reports, or enables negative limits; with checks off an
instance keeps only its explicit delayed signals as copies. Both
collections record, per declaration, the model rows it became
(`sdf_path_rows`, `sdf_check_rows`, `sdf_delayed_checks`); `sdf_targets`
runs the same metadata lowering and builds the SDF annotation catalog from
them (`collection/sdf_targets.rs`, `sim::sdf::SdfTargets`, including each
net's load ports for NETDELAY). The side tables never reach the IR.
`collection/sdf_annotations.rs` reads every `$sdf_annotate` call (constant
arguments only) and `--sdf` file before net groups are built: annotated
gates get writable delays (`IrInertialDelay::Annotated`), annotated
combined checks get runtime pair tables, interconnect loads get an
interconnect path set on their port net, and after the timing checks are
lowered the valued ops become `IrSdfAnnotation`s applied by
`IrStmt::SdfAnnotate` or, for `--sdf` and calls that lead an initial
procedure (A12-D1), at startup. Log files are returned in
`GeneratedModel::sdf_logs`. A design without SDF use is byte-identical.
Specify-block specparams are collected with the instance's
parameters. A design without specify blocks costs one emptiness test.
Force overlays visible storage while ordinary writes/driver slots remain live;
release retains variable value or recomputes current net resolution. Releasing a
variable toggles its hidden release marker; `attach_release_reevaluation` makes
the variable's synthetic continuous drivers (assignments, port links) sensitive
to it after all lowering, so they rerun (SV 10.6.2). Re-force replaces a matching
entry per bit. Keep typed constant-selected/concat targets; constant indexed,
member and element selects of nets become fixed parts (`normalize_force_lhs`).
Reject unsupported variable selects, automatic/local captures, arrays, and
string/handle/event or whole net-array targets with located errors.
Procedural `assign`/`deassign`: rejected by design (`statements/drivers.rs`
docs).
No subroutine/captured RHS admission without an owned activation environment.
