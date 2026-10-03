# RTL-008 fixed initialization, package state and let expansion

IEEE 1364-2001 §§6.2.1, 10.3.5 and 12.4.1 and IEEE 1800-2009 §§3.12.1, 3.13,
6.8, 6.20-6.21, 7.2, 7.4, 10.5, 11.2.1, 11.13, 13.3-13.5 and 26.2-26.6 supply
the oracles. Positive single-file sources run through the public CLI in both
optimizer modes and on the legacy and compact value backends; representative
sources also run after snapshot/Db destruction at native O0/O3. Multi-file
sources run in both optimizer modes and in the stated compilation-unit modes.

Owner policy for static initialization order: SystemVerilog runs every static
declaration initializer before any process (§10.5). Within one scope a
declaration precedes its simple references; across scopes the order is left
open (the frontend warns). The simulator runs a static initializer after every
static declaration its value reads, directly or through called functions,
and otherwise keeps declaration order. Fixtures that cross scopes either
follow a declaration dependency or use order-independent results.

- `package_order` chains package constants and variables through zero-time
  calls, a typed package parameter and an importing package; a module reads
  them through imported and qualified names.
- `call_order` mixes call results with later scalar, unpacked-array and record
  initializers in one module.
- `static_locals` covers explicit `static` variables in automatic functions,
  tasks and loop blocks (the §6.21 `top_legal` example), first called from a
  declaration initializer, with per-instance module, interface, generate and
  package state; automatic variables reinitialize per entry.
- `member_defaults` covers nested record member defaults, two-state and
  four-state uninitialized members, arrays of records, a record declaration
  assignment and nested default arguments in declaration initializers.
- `aggregate_lets` covers let declaration-scope binding (package and module),
  named and default actuals, typed formals, nested lets, template
  re-evaluation of effectful and conditional operands, and packed/unpacked
  aggregate results in assignments, continuous assignments, comparisons and
  declaration initializers.
- `descriptor_initializers` initializes 65,537-cell arrays (beyond one packed
  payload) in a package, a module, a let-copied declaration, a function-static
  and a block-static local before any process, without change events, and
  automatic function/block arrays on every entry, including across a delay.
- `package_constants` reads typedef, packed, unpacked-record and array
  parameters of a package and of `$unit` from module initializers, and a
  runtime `const` initialized by a call.
- `shared_pkg`, `shared_user` and `shared_top` share one package counter
  between three files in separate and merged compilation-unit modes; the
  oracle is order-independent.
- `unit_decls` with `unit_merged_top` shares merged `$unit` variables,
  functions, types and parameters across files, with a package re-export.
- `unit_separate_helper` with `unit_separate_top` keeps same-named `$unit`
  declarations of separate units distinct.
- `edition_race.v` pairs editions: Verilog-2001 declaration assignments race
  with an `always` (two allowed results, and the generated model keeps an
  active-region initialization process); SystemVerilog-2009 initializes
  before processes without an event (one result, no such process).
- Adopted FND-002 witnesses: `package_string_call_witness` (L-F03-03-02),
  `timed_function_witness` (L-F02-05-01), `let_recursive_witness`
  (L-F07-16-01), `hierarchical_constant_witness` (L-F03-03-04) and
  `soft_constraint_witness` (L-F03-03-03).
- Negatives (language): delays and event controls in functions called by
  declaration initializers, `let` in Verilog-2001, a variable array bound, a
  write to a `const` variable, `const` and package variables in parameter
  values, and redeclared names in a compilation-unit scope or a package (the
  frontend only warns; the simulator rejects instead of aliasing storage).

All `.out` files are hand-derived from the clauses above.
