# Electrical range partitioning

`ranges.sv` exercises widths 1, 7, 64, 65 and 129 with single/full competing
sources, overlapping/disjoint partial sources, ordinary and dissimilar wired
inout collapse, aliases, bit/range force and release, scalar strengths and delayed
contributions. Scalar strength sites aliased to vector bits keep distinct
connectivity; a captured delayed vector driver exercises the descriptor loop over
those remaining bit groups. Force through the scalar/vector peer observes the
same array cell without broadening the existing array-force admission.

`sim_net_partition.rs` computes expected bit strings independently from driver
coverage and explicit four-state truth rules. Every positive and negative CLI
case runs with normal optimization and `--no-opt`. `invalid_alias_width.sv`
keeps the nearest illegal width mismatch rejected. `runtime.sv` checks 64
128-bit array elements across 100 toggling-driver steps, with Z neutral and
opposite known drives resolving to X.

`shared_runs.sv` splits two vectors bit by bit: a coerced input formal on
each bit of `bus`, which has one wide driver, and a selected inout with a weak
driver on each bit of `sb`. Each vector stays one shared group with a narrow
contribution per bit (`collection/net_partition.rs`). The expected lines follow
from the LRM, independently of the group layout:

- SystemVerilog-1800-2009.txt L41461-41462 (23.3.3.1): "A port that is
  declared as input (output) but used as an output (input) or inout may be
  coerced to inout." Each `g[i].u.a` drives `bus[i]` as an inout would.
- L41596-41597 (23.3.3.7): "It is permissible to merge the dominating and
  dominated nets into a single net, whose type shall be that of the dominating
  net. The resulting net is called the simulated net". `bus[i]` and
  `g[i].u.a` show one value (`t1`: the wide 1 against the leaf's 0 on bit 3
  is `x` on both names; `t2`: the leaves alone give `zzzz0110`).
- L47609-47610 (28.12.1): "If two or more signals of unequal strength combine
  in a wired net configuration, the stronger signal shall dominate all the
  weaker drivers and determine the result." `sb` shows the strong `0011` over
  the weak `0101` (`t0`, `St0`), then the weak `0101` alone (`t5`, `We1`,
  `We0`), and `z` where no leaf drives (`t6`, `HiZ`).
- L13397-13399 (10.6.2): "A force procedural statement on a net shall
  override all drivers of the net ... When released, the net shall
  immediately be assigned the value determined by the drivers of the net."
  Forcing `g[1].u.a` shows 0 on `bus[1]` (`t3`); release restores 1 (`t4`).
- `db` has one wide strong 0 and a driver with a 2 ns delay on bits 1 and 0.
  A delayed driver drives X on the bits it drives until its first delayed
  update, other bits stay Z (decision S2-D4, `docs/lrm_decisions.md`; 6.7
  L5199-5200: "Nets with drivers shall assume the output value of their
  drivers."), so `t1d db=00xx`; then it turns off (`t7 db=0000`).
- `bit2_changes` counts the changes of `g[2].u.a` after time 0: `x` at `t1`
  and 1 at `t2`; the last write leaves it 1.

`waveform/partitioned_nets.sv` separately checks declared 129-bit array views,
escaped declared-index VCD names, and force/release values. The generated-model
VPI plugin checks the existing array element-width metadata and a resolved peer
value in both optimizer modes; indexed VPI array lookup remains outside the
current bridge's API.

Run the HDL cases and focused frame/C11 checks with:

```sh
CARGO_BUILD_JOBS=8 CMAKE_BUILD_PARALLEL_LEVEL=8 scripts/run-tests.sh \
    --test-work-dir /build --cargo-profile quick --test-threads 8 \
    --test sim_net_partition --test generated_c_frame_lint \
    -E 'test(/^sim_net_partition::/) | test(electrical_net_partition_fixtures)'
```

Lowering unit tests prove group counts and driver-range/force boundaries; emitter
unit tests prove constant executable write counts for many heterogeneous groups
and one inertial handle per delayed contribution. Declared-view binding metadata
still scales per bit, independently of electrical group count.
