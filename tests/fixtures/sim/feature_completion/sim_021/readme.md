# SIM-021 runtime-native operator overloading

IEEE 1800-2009 §11.11 with §§6.24.1, 11.4.2, 13.4, 13.4.4, 16.6 and 23.3.3.2
supplies the oracles. Every `.out` file was derived by hand from the bound
functions' bodies, never captured from llg. Positive sources run through the
public CLI in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends (`tests/sim_feature_completion/sim_021.rs`).

Line numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt` (SV).

## Clauses relied on

- SV 11.11 L16473-16476: "The overload declaration allows the arithmetic
  operators to be applied to data types that are normally illegal for them,
  such as unpacked structures. It does not change the meaning of the operators
  for data types where it is legal to apply them. In other words, such code
  does not change behavior when operator overloading is used."
- SV 11.11 L16488-16492: "The overload declaration links an operator to a
  function prototype. The arguments are matched, and the data type of the
  result is then checked. Multiple functions can have the same arguments and
  different return data types. If no expected data type exists because the
  operator is in a self-determined context, then a cast shall be used to
  select the correct function. Similarly if more than one expected data type
  is possible, due to nested operators, and could match more than one
  function, a cast shall be used to select the correct function."
- SV 11.11 L16494-16500: "An expected result data type exists in any of the
  following contexts: — Right-hand side of an assignment or assignment
  expression — Actual input argument of a subroutine call — Input port
  connection of a module, interface, or program — Actual parameter to a
  module, interface, program, or class — Relational operator with unambiguous
  comparison — Inside a cast"
- SV 11.11 L16529-16532: "... which normally shall match exactly. The
  exception is if the actual argument is an integral type and there is only
  one prototype with a corresponding integral argument, in which case the
  normal implicit casting rules apply when calling the function."
- SV 11.11 L16554: "Overloading the assignment operator also serves to
  overload implicit assignments or casting."
- SV 11.11 L16562-16563: "The assignment operator from a float to a float
  cannot be overloaded above because it is already legal in the three
  preceding bind statements. Similarly, equality and inequality between floats
  cannot be overloaded."
- SV 11.11 L16569-16570: "An assignment operator such as += is automatically
  built from both the + and = operators successively, where the = has its
  normal meaning."
- SV 11.11 L16576-16579: "The scope and visibility of the overload
  declaration follows the same search rules as a data declaration. The
  overload declaration shall be defined before use in a scope that is
  visible. The function bound by the overload declaration uses the same scope
  search rules as a function call from the scope where the operator is
  invoked."
- SV 11.4.2 L14496-14498: "SystemVerilog includes the C increment and
  decrement assignment operators ++i, --i, i++, and i--. ... These increment
  and decrement assignment operators behave as blocking assignments."
- SV 6.24.1 L7276-7277: "If the expression is assignment compatible with the
  casting type, then the cast shall return the value that a variable of the
  casting type would hold after being assigned the expression."
- SV 23.3.3.2 L41468-41469: "An input port can be connected to any expression
  of a compatible data type. A continuous assignment shall be implied when a
  variable is connected to an input port declaration."
- SV 13.4 L18673-18675: "a) A function shall not contain any time-controlled
  statements. That is, any statements containing #, ##, @, fork, wait,
  wait_order, or expect. b) A function shall not enable tasks regardless of
  whether those tasks contain time-controlling statements."
- SV 13.4.4 L18989-18991: "Functions shall execute with no delay. ...
  Statements that do not block shall be allowed inside a function;
  specifically, nonblocking assignments, event triggers, clocking drives, and
  fork-join_none constructs shall be allowed inside a function."
- SV 4.4.2.9 L3210-3212: "$monitor, $strobe and other similar events
  are scheduled in the Postponed region. No new value changes are allowed to
  happen in the current time slot once the Postponed region is reached."
- SV 16.6 L21547-21549: "Functions that appear in expressions shall not
  contain output or ref arguments (const ref is allowed). — Functions shall be
  automatic (or preserve no state information) and have no side effects."

## Positive fixtures (SIM-021-A01, A02)

- `native_pipeline`: `+`, `==` and two `=` overloads on
  `struct { string s; int q[$]; int d[]; C h; }`. `x = word` and `y = 5`
  convert through `bfroms`/`bfromi` (L16554): `x` is `ab '{2} '{2} 2` and `y`
  is `<5> '{5} '{} 5`. `z = x + y` concatenates the string, queue and dynamic
  array and builds a new handle with `2 + 5`; changing every member of `z`
  afterwards (`!`, `push_back(9)`, `d[0] = 99`, `h.v = 70`) leaves `x` and `y`
  unchanged, and changing `x` afterwards leaves `z` unchanged. `x + y + x`
  (`AB<5>AB`, handle `2 + 5 + 2 = 9`) reuses intermediate results. A sum and a
  string pushed on a `bag_t` queue, a sum passed to a `bag_t` argument and a
  string converted for that argument (L16494-16496) follow. `z == upper`
  compares `AB<5>AB` with `AB` (0), `x == upper` gives 1, `x == word` 0.
- `native_contexts`: `mka`/`mkb` share formals and differ in result
  (`v = x + y` or `x * y`, with an `A:`/`B:` tag), selected by assignment, a
  `b_t` argument, a `b_t` input port that follows later operand changes
  (L16497, L41468-41469), named and positional assignment-pattern members, a
  cast to either type (a cast to the result's own type is the identity,
  L7276-7277), two continuous assignments and an `always_comb`. With `x = 2`,
  `y = 5`: `A` results are 7, `B` results 10; after `x = 3`, `X`: 8 and 15.
  The tb displays at time 2 and 6, the port at 1 and 5, so no two displays
  share a time slot.
- `operand_kinds`: an unpacked union sum (`3 + 4 = 7`) and `==` with an int;
  the clause's float example on `struct { real r; shortreal s; }` with real,
  shortreal and record operands, an `=` conversion from real (`s = 0.5`), a
  unary `-` and a `<` comparison; record/int, int/record and record/string
  operands, and a `byte` reaching the only prototype with an integral formal
  (L16529-16532).
- `native_update_values` (A02): on a record with string and queue members,
  `y = x++` yields the old value as an independent copy (changing `y.s`
  leaves `x.s`), `y = ++x` and `y = (x += 5)` yield the updated target,
  `y = arr[1]--` works on an array element, and an automatic task local takes
  `b = a++` and `b = --a`. On a 65,537-element `int` array (above the packed
  value limit), `by = (bx += 3)` yields the updated array. `calls` counts one
  bound-function call per update (7 in total).
- `nested_casts` (A02, L16490-16492): `a_t'(x + y) + x` selects `fa` inside;
  the outer sum takes `a_t` or `b_t` from the assignment; `b_t'(x + y) + x`
  selects `fb` inside and then the only `(b_t, a_t)` prototype `fba`; a doubly
  nested cast chain also resolves.
- `legal_effects` (A03 boundary, L18989-18991): the bound function performs a
  nonblocking assignment (`late` is still 0 after the call, 3 one time unit
  later) and a fork-join_none (`spawned` becomes 1 after the call returns). An
  overloaded `<` without side effects runs in `$monitor`, `@(...)` and
  `wait (...)`; the monitor prints 1 at time 0 and 0 after `px.v = 3`.

## Nearest-illegal negatives (SIM-021-A02, A03)

- `neg_builtin_add`, `neg_builtin_copy`, `neg_builtin_equality`,
  `neg_builtin_conversion` and `neg_builtin_string_compare` (L16473-16476,
  L16562-16563): an `int + int` prototype with an `int` result, a same-type
  record `=`, a same-type record `!=`, an `=` from `real` to `int`, and a
  `string < string` prototype are rejected at the declaration. The decision is
  register entry S21-D1 (`lrm_decisions/S21-D1_builtin_legal_overload`).
  Prototypes whose result type the built-in result could not stand for stay
  admitted and never change a legal operation (S21-D2 and
  `rtl_017/builtin_preserved`).
- `neg_timed_function` (`#`) and `neg_event_wait_function` (`@`) violate 13.4
  a); `neg_task_binding` binds a task and `neg_output_formal` a function with
  an output formal, neither of which matches the prototype.
- `neg_invisible_declaration`: a declaration in another module is not visible
  (L16576-16579), so `+` stays invalid for the records.
- `neg_nested_ambiguous_native` and `neg_self_determined_native`: a nested sum
  and a `$display` argument have no single expected type (L16490-16492).
- `neg_monitor_effect`: a bound function that writes module storage is
  rejected as a `$monitor` argument, exactly as the same call would be
  (L3210-3212).
- `neg_assertion_effect`: a side-effecting bound function in a concurrent
  assertion (16.6 L21547-21549) is not admitted. llg rejects every function
  call in a concurrent assertion expression today, so the message is that
  general limit, not a side-effect diagnosis.
- `limit_native_value_argument`: the documented limit
  (`docs/known_issues.md`): an overloaded update on a native record yields its
  value only as the right-hand side of an assignment, not as a call argument.
