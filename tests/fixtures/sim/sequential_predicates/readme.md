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

`syn_022_basic_patterns.sv` adds the SV-2009 primitive pattern subset: integral
constant patterns, `.*`, identifier bindings in later `&&&` clauses and true
arms, four-state constant inputs, ordered side effects, function locals and
combinational sensitivity. Its stdout is checked through the public CLI with
and without optimization. `bad_matches_*` retain legal structure-pattern
forms as fail-closed controls; `bad_matches_out_of_scope.sv` checks the
frontend binding boundary. These negatives must reject instead of becoming
Boolean tests of the matched source or exposing a binding in the else arm.

`syn_023_structure_patterns.sv` adds recursive fixed packed and unpacked
structure patterns. Positional and named fields, omitted members, nested
patterns, wildcard members, mixed signedness/state domains, and lexical
bindings are checked through the public CLI. Function-returning sources prove
that each matched value is captured once before ordered member checks.
