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
    -E 'binary(sim_net_partition) | test(electrical_net_partition_fixtures)'
```

Lowering unit tests prove group counts and driver-range/force boundaries; emitter
unit tests prove constant executable write counts for many heterogeneous groups
and one inertial handle per delayed contribution. Declared-view binding metadata
still scales per bit, independently of electrical group count.
