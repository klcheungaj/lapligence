# RTL-017 fixed operator overloading

IEEE 1800-2009 §11.11 and Annex A.2.8 supply the oracles; every expected value
was derived by hand from the bound functions' bodies. Positive sources run
through the public CLI in both optimizer modes and on the legacy and compact
value backends; `saturating_records` and `scopes` also run after snapshot/Db
destruction.

An overload is consulted only where the built-in operator is illegal for the
operand types, so each use below calls its bound function exactly once and a
legal built-in operation never changes meaning.

- `saturating_records` covers every overload operator on a saturating 8-bit
  record: binary `+ - * / % **`, unary `+ -`, relational `< <= > >=`, `==`
  and `!=` against `int`, prefix and postfix `++`/`--` statements (also on an
  array element and as a `for` step), compound `+= -= *= /= %=` built from the
  arithmetic overload plus a normal assignment, and `=` conversions from `int`
  and `real` in assignments, a declaration initializer, an argument, a return
  and a cast. A 4-bit and a 40-bit operand use the integral exception (the
  normal implicit cast to the single integral formal).
- `result_casts` binds `+` twice with the same formals and different result
  types; the assignment, argument, port connection and return contexts select
  one, and a cast selects one inside a nested expression.
- `scopes` covers declaration-before-use, a compilation-unit declaration, a
  block and a function-body declaration shadowing it, a later module-level
  declaration, and a bound function name resolved from the scope of the use
  (`other` has its own `add1`).
- `fixed_point_pipeline` composes Q8 fixed-point records with continuous
  assignments, `always_ff` nonblocking assignments, input ports converted by
  an `=` overload and overloads used inside another function.
- `descriptor_operands` overloads `+`, unary `-` and `=` on a 65,537-element
  `int` array (wider than the packed value capacity), so operands, results and
  the compound target cross the bound functions as descriptors.
- `builtin_preserved` declares overloads for operations that are already
  legal (packed and real arithmetic, same-type copy and equality, an increment,
  a legal implicit conversion and a comparison); their sentinel results never
  appear.
- `operator_overload_witness` is the adopted FND-002 witness for L-F07-17-01.

Nearest-illegal negatives: `neg_self_determined_ambiguous` and
`neg_nested_ambiguous` (no single expected result type without a cast),
`neg_use_before_declaration` and `neg_nonvisible_block` (missing or
nonvisible prototypes), `neg_missing_function`, `neg_prototype_mismatch`,
`neg_overload_arity`, `neg_not_overloadable` (`&` is not an overload
operator) and `neg_v2001_overload.v` (IEEE 1364-2001 has no overload
declarations). The former `neg_postfix_value` and `neg_target_side_effects`
forms are legal and now execute: see `rtl_104/` (`rtl017_postfix_value`,
`rtl017_target_side_effects`).
