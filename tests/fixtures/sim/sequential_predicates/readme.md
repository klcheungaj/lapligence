# Sequential Boolean conditional predicates (R06)

Reference: IEEE 1800-2009 12.6.2-12.6.3, with result selection/merging from
11.4.11. The source oracle treats only definite true as permission to evaluate
another clause; X/Z ends the sequence before any later false. `if` takes else
on ambiguity, while `?:` evaluates both arms. Real results become zero; packed
and unpacked-array results retain their different merge rules.

All files use finite checked-in test stimuli. `truth_table.sv` checks the 64
three-clause combinations with an independent source oracle: 1 true, 21 false,
42 ambiguous. Effects are checked by counts/prefix traces, not by imposing an
order between the two alternatives evaluated for an ambiguous mux. Signed/wide
truth, qualifiers, nested branches, sensitivity, fixed-array values and lexical
reduction contexts have separate fixtures. The Rust suite owns exact stdout
oracles and runs both optimizer modes; no source transformation to `&&` is used.

`bad_matches_*` are legal language forms but intentionally unsupported simulator
cases. They must reject with a pattern diagnostic, not silently ignore matching
or prune it away. The fixture inventory is not evidence that Rust compilation or
HDL execution has passed; those steps were unavailable for this delivery.
