# Floating-point contraction

Generated models are built with `-ffp-contract=off` on GCC and Clang
([cross-platform guide](../../../../../docs/cross_platform.md#memory-floating-point-and-pages)),
so `real` arithmetic rounds after every operator on every host.

`real_operators_round_separately.sv` relies on IEEE 1800-2009 6.12
(`SystemVerilog-1800-2009.txt` L5516): "The real10 data type is the same as a C
double." and note 10 (L5561): "The real and shortreal types are represented as
described by IEEE Std 754." The text does not mention fused operations; the
decision is that each operator is a separate IEEE 754 operation.

The oracle `real_operators_round_separately.out` is computed by hand: with
`a = 1 + 2**-30` and `b = 1 - 2**-30` the exact product `1 - 2**-60` is closer to
`1.0` than to the next double below it (`1 - 2**-53`), so the rounded product is `1.0`
(`3ff0000000000000`) and every `*`/`+`/`-` combination with `-1.0`/`1.0`
gives `0.0`. A fused multiply-add gives `+-2**-60` (`3c30000000000000` or
`bc30000000000000`).

The module `tests/sim_feature_completion/ki_fp_contract.rs` runs the case in
both optimizer modes on every value backend. The generated sources split each
operator into its own statement, so Clang's default contraction
(`-ffp-contract=on`, within one expression) cannot fuse them; the case fails
only with whole-function contraction (`--cflags "-march=haswell
-ffp-contract=fast"`, which GCC applies in GNU C mode). A standalone C
reproducer of the single-expression fusion lives in the task evidence, not in
the tree. The CMake flag itself is unit-tested in `src/sim/build.rs`.
