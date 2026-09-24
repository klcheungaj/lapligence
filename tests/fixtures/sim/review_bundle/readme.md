# Synthesizable review witnesses

The 18 R01–R11 HDL sources, two memory inputs, and two native C probes came
from the 2026-09-23 synthesizable review bundle. Additional R01–R09 and R13
sources exercise repaired features and their interactions. The R12 sources
provide concrete grammar/context witnesses for the maintained SYN-038 ledger.
`native_reversed_2d.hex` supplies the four-word input needed by the native
order control.

The positive cases print a success line only after checking their own result. Tests must
also assert exact stdout and run both optimizer modes through the public `llg`
CLI. R01 and R08 require a specific runtime diagnostic; a frontend or lowering
error does not satisfy them. R03 tests the project's read-only callback policy.
R10 records a conditional-policy choice in both editions. R11 is an optional
generation-only capacity probe, not a memory-stress simulation.

`tests/sim_review_bundle_patterns.rs` checks the R04–R06 type-key and
deconstruction cases in both optimizer modes. The R04 cases cover duplicate
type keys, recursive struct matching, and an unused type key covered by a
default. R05 covers arithmetic, negative, and constant-function index keys.
R06 covers packed and unpacked structures plus a packed array as positional
pattern-lvalue targets.

The native C programs exercise runtime helpers directly. `native_probe.c` is
run by `sim_memory_editions`; `native_controls.c` is run by `sim_memory_views`
with exact output and diagnostic checks. These are narrow controls and do not
substitute for generated-model tests.

The added R01/R02 cases cover a selected tagged receiver, a selected NBA
destination, and member reads/writes through a legal whole-union `ref` formal.
Inactive accesses require source-located runtime diagnostics and preserve the
active member payload. The focused suite asserts exact output and exit status.

`r09_readmem_slice_address_bounds.sv` checks that an explicit start/finish
address outside a selected slice produces a diagnostic and leaves the memory
unchanged. The other R09 cases cover a static slice, a runtime-selected row,
one-time selector evaluation, and reversed declaration bounds.

## R13 composed public-CLI witnesses

`r13_recursive_pattern_function_port.sv` and
`r13_record_pattern_selected_nba.sv` are additional composition cases authored
in this repository after the held-out bundle was assembled. The companion
`tests/sim_review_bundle_composition.rs` runs each through `llg` with and
without optimization and checks its exact stdout. The first carries recursive
type-key construction through an automatic fixed-array return, aggregate input
port, and `always_comb` reader. The second deconstructs a packed structure into
a runtime-selected destination through NBA scheduling, then changes both the
selector and source before the NBA commits.

`r07_fixed_rows.sv` is an additional fixed-array ordering case. It checks that
`reverse()`, `sort() with (...)`, and `rsort() with (...)` move complete rows,
including record fields outside the comparison key. The companion
`tests/sim_fixed_ordering_review.rs` runs it and the two held-out R07 record
witnesses through `llg` with and without optimization. It also checks that a
record sort without a legal integral comparison key is rejected.
