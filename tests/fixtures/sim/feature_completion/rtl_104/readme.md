# RTL-104 operator-overload leftovers

IEEE 1800-2009 §§11.4.2, 11.11, 26.3 and 26.6 supply the oracles; every `.out`
file was derived by hand from the bound functions' bodies. Positive sources run
through the public CLI in both optimizer modes and on the legacy,
compact/portable and compact/GMP value backends;
`incdec_values` and `single_evaluation` also run after snapshot/Db destruction.

An overloaded compound assignment or increment whose value is used, or whose
target has a side-effecting selector, binds its target once: the bound
function reads the target's current value and the result is stored through
the same resolved target. Statement forms with side-effect-free targets keep
the RTL-017 `A = f(A, B)` assignment.

- `incdec_values`: prefix and postfix `++`/`--` values on a two-member record,
  as a function argument, in a `while` condition compared through an
  overloaded `<`, on an array element, on a nested record member, on an
  automatic local returned from a function, on an unpacked `int [2]` array, and
  a compound assignment used as a value.
- `single_evaluation`: `arr[next()] += d`, `arr[next()]++`, `++arr[next()]`,
  `y = arr[next()]++`, `y = (arr[next()] += d)`, a nested member target
  `recs[next()].inner++` and a `for` step `arr[next()]++`; `next()` advances a
  counter, which shows each index call runs once.
- `rtl017_postfix_value` and `rtl017_target_side_effects`: the former RTL-017
  negatives `neg_postfix_value` and `neg_target_side_effects`, now legal forms
  with outputs.
- `relational_expected`: §11.11 lists "relational operator with unambiguous
  comparison" as an expected-type context. Operands of `<`, `<=`, `>`, `>=`,
  `==` and `!=` whose `+` or unary `-` prototypes differ only in their result
  type take the other operand's type: a record, another record type, `int`
  (selecting an `int` result compared by the built-in `==`), and an operand
  that is itself an overload resolved on its own; also in `if`, `while` and a
  conditional operator.
- `package_import` and `package_import_unit`: the project ruling on package
  visibility (`docs/sim_data_semantics.md`). Wildcard imports in a module, a
  package (`q` uses `p`'s `+`), a procedural block and the compilation unit
  make a package's overloads visible; a module's own declaration is found
  before an imported one.
- `composition`: a clocked `q <= cnt++` (old value scheduled, new value stored
  at once), a continuous assignment of the registered member, an
  `always_comb` comparison of overloaded sums, and a task incrementing an
  automatic copy.

Nearest-illegal negatives: `neg_relational_both_ambiguous` (both operands need
the other's type), `neg_relational_nested` (a nested operand has no expected
type) and `neg_relational_no_match` (`int` selects neither prototype) keep the
cast diagnostic; `neg_import_explicit` (explicit imports never carry an
unnamed overload), `neg_import_after_use` (the import follows the use) and
`neg_import_reexport` (exports never carry it) report invalid operands;
`neg_import_two_packages` reports two equally matching imported prototypes as
ambiguous. `limit_native_value` pins the documented limit: an overloaded
update used as a value needs a target within the packed value limit and
without native (string) members.
