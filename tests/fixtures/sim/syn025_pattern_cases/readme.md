# SYN-025 pattern case acceptance

IEEE 1800-2009 §12.6.1 defines ordered `case ... matches` items, optional
`&&&` filters and item-local bindings. §12.5.3 defines `unique`, `unique0`
and `priority` diagnostics: `unique` reports zero or multiple matches,
`unique0` only multiple matches, and `priority` zero matches; a default
suppresses zero-match reports. `qualifiers.sv` checks these counts, first-body
selection and one side-effecting selector call. The existing
`../sequential_predicates/syn_025_pattern_case.sv` checks fixed structure,
primitive, tagged and X/Z pattern modes.

`ordinary_controls.sv` checks selector-side Z and X under pattern `casez`/
`casex`, and that ordinary exact `case`, selector-side Z in `casez`, and
`case inside` item wildcards retain their separate §12.5 rules.
`runtime_modes.sv` repeats selector-side Z/X comparisons for each pattern
`case` mode using a procedural variable and a side-effecting selector function
whose calls are counted. `$value$plusargs` and `$test$plusargs` make the source
values runtime-dependent. It checks item-side `?`/Z in `casez`, item-side X
in `casex`, exact-mode negative controls, and a runtime packed-structure
payload under `casez` and exact `case`. All outcomes follow the §12.5.1
wildcard table applied to §12.6.1 pattern cases.
`ordinary_controls.sv` also supplies the 2001 edition rejection, since
`matches` was introduced after IEEE 1364-2001. `bad_binding_scope.sv`
rejects use of an item binding after `endcase`; `string_pattern.sv`
binds a whole string selector (SIM-007): the filtered first item fails on
`"hello" == "bye"`, the second binds `hello` (length 5) and sets the result. The Rust suite runs each source in
both optimizer modes.
