# ADV-019 fixtures: VPI typespecs, structure and union members, structure, expression and statement objects

Each `<name>.sv` has a C application `<name>.c` that registers the system task
the design calls. `tests/sim_feature_completion/adv_019.rs` links it as a
static archive with `--dpi-lib` (every host), declares the task with
`--define-system-task` and compares stdout with `<name>.out` in both optimizer
modes on the legacy, compact/portable and compact/GMP value backends. Clause
numbers are IEEE 1800-2009; line numbers refer to
`SystemVerilog-1800-2009.txt`.

The expected outputs were derived by hand from the rules below and then
checked against a run; no line was accepted only because the run printed it.
A status level printed as `level=3` is `vpiError`, read with `vpi_chk_error`
right after the routine it checks.

## typespecs (37.23)

`typespecs.c` prints, for each declaration, `vpiType` and the typespec tree:
the typespec type and `vpiName`, `vpiPacked` (structures and unions),
`vpiTagged` (unions), `vpiVector` (logic, bit and packed array typespecs),
the ranges of `vpi_iterate(vpiRange)`, then `vpiTypedefAlias`, the enum base
and constants, the typespec members and `vpiElemTypespec`, recursively.

- Names. Detail 1 (L59170-L59172): "If a typespec denotes a type that has a
  user-defined typedef, the vpiName property shall return the name of that
  type; otherwise, except in the case of a class typespec (see 37.28), the
  vpiName property shall return NULL. Consequently the vpiName property
  returns NULL for any SystemVerilog built-in type." Built-in and inline
  types print `name=(null)`.
- Aliases. Detail 1 (L59172-L59174): "If the typespec denotes a type with a
  typedef that creates an alias of another typedef, then the vpiTypedefAlias
  of the typespec shall return a non-null handle, which represents the
  handle to the aliased typedef." `colors` is the text's example
  (L59176-L59181): an enum typespec named `colors` whose alias is the enum
  typespec `primary_colors`. Detail 6 (L59200, L59204-L59206): "If a type
  is defined as an alias of another type, it inherits the vpiType of this
  other type. ... The vpiTypedefAlias applied to handle h1 shall return a
  typespec handle h2 to the predefined type "time"." So `my_time t` prints a
  `vpiTimeTypespec` named `my_time` with an unnamed `vpiTimeTypespec`
  alias, and `count_t` (a typedef of `int`) and the parameter `P` declared
  with it print the same shape with `vpiIntTypespec`. `p::nib_t` (a typedef
  of `logic [3:0]`) has no alias; see decision A19-D2.
- Members. Detail 3 (L59184-L59186): "it is possible to iterate over
  vpiTypespecMember to obtain the structure of the user-defined type. For
  each typespec member, the typespec relation indicates the type of the
  member." Detail 4 (L59187-L59188): "The property vpiName of a typespec
  member returns the name of the corresponding member, rather than the name
  (if any) of the associated typespec." The members come in declaration
  order. `C` is the example of detail 5 (L59192-L59199): "The typespec
  relation for B returns another struct typespec that has no name and has a
  single typespec member named "a". The typespec relation for "a" returns
  an int typespec." Its member `n` keeps the name of its typedef `nib_t`.
- Element typespecs. `arr` is the example of detail 8 (L59236-L59244): "The
  typespec of the object arr is an unpacked 4 x 3 array typespec with a NULL
  vpiName property. The typespec retrieved by applying vpiElemTypespec to
  this is a 3-element unpacked array typespec with a vpiName property of
  "unparrtype". The typespec retrieved by using vpiElemTypespec on this in
  turn yields a 2 x 3 packed array typespec (of packed struct objects) with
  a vpiName property of "parrtype2". Using vpiElemTypespec again in turn
  yields another packed array typespec (of 3 packed struct objects) with a
  vpiName property of "parrtype". One more application of vpiElemTypespec
  to this result yields a struct typespec". The packed array typespecs list
  only their own level's range (`[2:1]`, then `[1:3]`), as 37.17 detail 4
  (L58748-L58750) asks of packed array variables: "The ranges returned for a
  packed array shall not include the implicit range for packed struct or
  union var elements themselves". A bit or logic typespec unwinds one packed
  range at a time (the member `lo` of `pk_t`, `bit [1:0][2:3]`, prints
  `[1:0] [2:3]`, then `[2:3]`, then a scalar) and detail 8 (L59218-L59219)
  ends the chain: "When the vpiElemTypespec relation is applied to a
  vpiStructTypespec, vpiUnionTypespec, vpiEnumTypespec, or a vpiBitTypespec
  or vpiLogicTypespec with no ranges present, it shall return NULL."
- Ranges. Detail 10 (L59249-L59253): "For an array typespec,
  vpi_iterate(vpiRange, handle) shall return the set of array range
  declarations beginning with the leftmost unpacked range and iterating
  through the rightmost unpacked range. ... For a logic typespec or bit
  typespec that has an associated range, the iteration shall return the set
  of ranges beginning with the leftmost packed range". The bounds are read
  with `vpiLeftRange` and `vpiRightRange` of each range.
- Vectors. 37.17 detail 20 (L58809-L58812): a bit or logic var "with one or
  more packed dimensions defined, is a vector ... A packed struct var, a
  packed union var, and packed array var are vectors"; the typespecs report
  the same `vpiVector`.
- Enums. The base typespec of `primary_colors` is `bit [0:2]`, and its
  constants print their name, `vpiBinStrVal` (the base width, left bit
  first), `vpiIntVal` and `vpiType` `vpiEnumConst`: `red` 0, `yellow` 1,
  `blue` 2 (6.19, L6165: "If the first name is not assigned a value, it is given the
  initial value of 0."; later names count up by one).
- Variable types. `count_t n` is a `vpiIntVar` like `int n`, and the other
  integer atoms print their own variable types (`vpiByteVar`,
  `vpiShortIntVar`, `vpiLongIntVar`, `vpiIntegerVar`); a scalar `logic` is a
  `vpiReg` (37.17 detail 19, L58783-L58787: "logic var == reg") and `arr` a
  `vpiRegArray` ("array var == reg array").
- Unsupported relations. `vpiIndexTypespec` exists only on associative array
  typespecs (detail 2, L59182: "vpiIndexTypespec relation is present
  only on associative array typespecs"); asking an enum typespec for it
  returns NULL with `vpiError`.

## members (37.17, 37.24)

`members.c` prints each structure or union, then iterates `vpiMember` (37.24)
recursively. Each member line shows `vpiType`, `vpiFullName`, `vpiName`,
`vpiSize`, `vpiStructUnionMember`, `vpiPacked` for structures and unions,
the value (`vpiHexStrVal`, or `vpiRealVal` for a real member), the
`vpiName` of `vpiParent` and the `vpiType` of the member's typespec.

- Values. `ns` is `{inner, s, st}` = `{8'ha0, 8'hfd, 2'b10}`, the 18-bit
  value `283f6`. 37.17 detail 9 (L58760-L58763): "for enum vars, and for
  packed struct and union variables, vpiSize shall return the size of the
  variable in bits. ... For unpacked structures and unions, the size
  returned indicates the number of fields in the structure or union." So
  `ns` has size 18, `up` (four fields) size 4 and each `uarr` element size
  2. A packed union's members overlay the same bits: `pu = 8'h96` reads
  `w = 96`, `p = 96`, `p.hi = 9`, `p.lo = 6`.
- Whole unpacked structures. 37.24 detail 1 (L59271-L59272):
  "vpi_get_value()/vpi_put_value() cannot be used to access values of
  entire unpacked structures and unpacked unions." `up`, `uarr[0]` and
  `uarr[1]` print `value: level=3`, and `vpi_put_value` of `tb.uarr[0]`
  fails with level 3 (decision A19-D3 covers the array elements). Their
  members have values: `up.b = 8'h5x` reads `5x`.
- Member flags and parents. 37.17 detail 17 (L58778-L58779): "When the
  vpiStructUnionMember property is TRUE, it indicates that the variable is a
  member of a parent struct or union variable." Detail 26 (L58838-L58842):
  vpiParent returns a prefix object, "the object obtained from successively
  removing the rightmost index or identifier from a compound or
  indexed/multidimensional object name"; the parent of `tb.ns.inner.lo` is
  `tb.ns.inner` and that of `tb.uarr[1].b` is `tb.uarr[1]`.
- Names. `vpi_handle_by_name` takes a member path (`tb.ns.inner.lo`,
  `tb.up.p.hi`) and an element followed by a member (`tb.uarr[1].b`). A
  member that does not exist returns NULL with no error, as decision S42-D11
  does for every name that denotes no object. `vpiMember` of a plain logic
  variable returns NULL with `vpiError`.
- Callback. A `cbValueChange` on `tb.ns.inner.hi` (`vpiHexStrVal`) runs when
  the design assigns `ns.inner.hi = 4'h1` at time 1 and prints `1`; the VPI
  writes before it touch other members and do not run it.
- Writes. With `vpiNoDelay`: `ns.inner.lo = 9` and `ns.s = -1` make `ns`
  `{8'ha9, 8'hff, 2'b10}` = `2a7fe`; `pu.p.hi = 0` makes `pu` `06`;
  `up.a = -4`; `up.p.lo = 1` makes `up.p` `31`; `uarr[1].b = 8'h5a`. The
  design prints the written values at time 1.

## structure (37.5-37.15, 37.25, 37.31, 37.37, 37.43, 37.58)

`structure.c` is an independent walker. `$structure` (at time 1, after the
time-0 evaluation of the gates and assignments) starts from
`vpi_iterate(vpiModule, NULL)` and prints, for every module and interface
instance, its modports, ports (with both connections and the first port
bit), named events, processes, continuous assignments, tasks and functions,
primitives (with their terminals) and instance arrays (with their ranges and
elements), then recurses into `vpiInterface` and `vpiModule` children. Each
relation is checked back against its origin with `vpi_compare_objects`
(parents, `vpiModule`/`vpiScope`, lookup by name, `vpi_handle_by_index` of
array elements, both `vpiActual` of an interface port); an exhausted port
iterator must be released. A failed check prints `CHECK FAILED` and
`failures=0` closes the walk. A connection prints as
`type:full name=binary value`, or without the value when `vpi_get_value`
fails; `NULL` is a NULL handle. Children come in catalog order (by name;
unnamed processes and assignments in source order, decision A19-D7).

- Top modules. 37.5 detail 1 (L57768): "Top-level modules shall be accessed
  using vpi_iterate() with a NULL reference object." Only `tb` is a top;
  `bus_if`, `leaf` and `sink` are instantiated.
- Ports. 37.14 details 3-4 (L58185-L58186): "vpiHighConn shall indicate the
  hierarchically higher (closer to the top module) port connection." and
  "vpiLowConn shall indicate the lower (further from the top module) port
  connection." Detail 9 (L58193): "The first port has a port index of
  zero." Detail 10 (L58195-L58196): "vpiHighConn shall return NULL if the
  instance of the module, interface, or program does not have a connection
  to the port." So `u[0].y` and `u[1].y` (`.y()`) print `high=NULL`;
  `single.a` is connected to the literal `4'b1010`, a `vpiConstant` with no
  name whose value is `1010`. Detail 6 (L58188): vpiScalar tells whether the
  port is one bit. The port bits (`vpiPortBit`) iterate from the most
  significant bit (decision A19-D7); bit 3 of `single.y` connects
  `single.y[3]` low and `r2[3]` high, and a constant has no bits, so the
  high connection of a bit of `single.a` is NULL.
- Interface and modport ports. Detail 1 (L58182): "vpiPortType shall be one
  of the following three types: vpiPort, vpiInterfacePort, and
  vpiModportPort." Detail 5 (L58187): "vpiLowConn of a vpiInterfacePort
  shall always be vpiRefObj." 37.15 (L58306-L58313): "for the port of i2 ,
  the vpiHighConn relationship returns a handle of type vpiRefObj. The
  vpiActual relationship applied to the ref obj handle returns a handle of
  type vpiModport." `sk.b` (`bus_if.dst b`) is a modport port: both
  connections are reference objects whose `vpiActual` is the modport
  `tb.bi.dst`; detail 7 of 37.15 (L58288-L58289) gives the low one the
  `vpiDefName` `dst`. Its `vpiDirection` is `vpiNoDirection` (5).
- Modports. 37.7 lists modports under an interface; `tb.bi` has `dst` and
  `src` (by name) and each modport's `vpiParent` is the interface.
- Named events. 37.25 (L59284-L59310) gives a named event a name and only
  `vpi_put_value()` as value access, so `vpi_get_value` fails
  (`value=error`); it is not an array member.
- Processes. 37.58 detail 1 (L61096): "vpiAlwaysType can be one of
  vpiAlways, vpiAlwaysComb, vpiAlwaysFF, or vpiAlwaysLatch." `always_latch`
  prints 4, `always @(ev)` 1 (`vpiAlways`), `sk`'s `always_ff` 3; the
  `initial` and `final` procedures print their types. Lines are the source
  lines of the procedures in `structure.sv`.
- Continuous assignments. 37.43 (L60275-L60304) relates a cont assign to
  `vpiLhs` and `vpiRhs` and lists its value; decision A19-D5 makes the
  value the right-hand side converted to the left-hand side size. `assign
  w1 = r0[0]` has lhs `tb.w1`, rhs the bit select `tb.r0[0]` and value `1`
  (`r0 = 4'h3`). In `leaf`, `assign y = ~a` has an operation on its right:
  the rhs is a `vpiOperation` (family 3) whose value is `~a`, `1100` in
  `u[0]` and `u[1]` and `0101` in `single`, and so is the assignment's.
- Tasks and functions. 37.37 lists `vpiAutomatic`, `vpiFuncType`,
  `vpiSize` and `vpiSigned`; detail 12 (L60024-L60026) makes `vpiSize` the
  return size. `inc` (an automatic function returning `int`) prints
  `func_type=5` (`vpiSizedSignedFunc`, "returns sized signed value", Annex
  K L75421-L75422), size 32, signed 1; `fire` is a static task and `ping` an
  automatic interface task. Each is found by its full name and its
  `vpiScope` is its instance.
- Primitives. 37.31 detail 1 (L59782): "vpiSize shall return the number of
  inputs."; detail 3 (L59784): "The first terminal has a term index of
  zero."; detail 4 (L59785-L59786) gives array elements their `vpiIndex`.
  37.32 detail 2 (L59825): "vpiPrimType returns vpiSeqPrim for sequential UDPs and
  vpiCombPrim for combinational UDPs." `g1` is `and` (`vpiAndPrim` 1) of
  `r0[0] = 1` and `r0[1] = 1`, value 1; `ud` is the combinational UDP
  `udp_and` (28) of `r0[2] = 0` and `r0[3] = 0`, value 0. The `nand` array
  `ng [1:0]` slices its connections (28.3.6): `ng[0]` owns the right-hand
  slices `ga[0]`, `r0[0]` and `r0[2]` (`nand(1, 0) = 1`), `ng[1]` owns
  `ga[1]`, `r0[1]` and `r0[3]` (also 1). A primitive's value is its output
  terminal's; a gate's `vpiDefName` is its keyword and a UDP's its name
  (decision A19-D6).
- Instance arrays. 37.11 detail 2 (L58019-L58021): "vpi_iterate(vpiRange,
  instance_array_handle) shall return the set of instance array ranges
  beginning with the leftmost range of the array declaration". `tb.u` is a
  `vpiModuleArray` and `tb.ng` a `vpiGateArray`, each of size 2 with range
  `[1:0]`; their elements iterate from the leftmost index (decision A19-D7),
  each with its `vpiIndex` (37.5 detail 2, L57769-L57770) and
  `vpiArrayMember` 1, and `vpi_handle_by_index` returns the same element.
- Leaf values. `u[0]` and `u[1]` receive `r0 = 0011` and drive
  `y = ~a = 1100`; `single` receives `1010` and drives `0101` onto `r2`.
  `sk.seen` is `x`: `bi.data` has no driver.
- The event callback. After the walk a `cbValueChange` is placed on
  `tb.ev`. 38.36.1 (L64747-L64749): "When the event statement is executed,
  the callback routine will be called. Because event statements do not
  have a value, when the callback routine is called, the value field of the
  s_cb_data structure will be NULL." `fire` (`-> ev`) at time 5 runs it
  before the woken `always @(ev)` prints `ev at 5`. 38.34 (L64239): "Calling
  vpi_put_value() on an object of type vpiNamedEvent shall cause the named
  event to toggle." `$fire` at time 10 does so: the callback runs inside the
  put, then `fire level=0`, then the woken process (decision A19-D8). `inc
  2` and the `final` line close the run.

## expressions (37.54, 37.55, 37.19, 37.38)

`expressions.c` walks expression trees. `$exprs` (at time 1) prints its own
three arguments, the right-hand side of every continuous assignment of `tb`
(in source order) and the high connection of `u.a`. Each line shows
`vpiType`, `vpiOpType` or `vpiConstType`, `vpiName` when there is one,
`vpiSize`, `vpiConstantSelect` and `vpiIndexedPartSelectType` of selects,
`vpiDecompile` and the value (`vpiObjTypeVal`: a real prints with `%g`,
everything else as `vpiBinStrVal`; `error` when `vpi_get_value` fails).
Below it come the operands, the parent and index or range expressions of a
select, and the arguments of a call. After each assignment its own value is
printed. `$again` (time 2, after `a = 1` and `i = 1`) re-reads the saved
right-hand sides: expression handles stay valid and evaluate the current
values. `failures=0` closes the walk when `vpiParent` of `w[i * 4 +: 8]` is
`tb.w`.

- Operands. 37.55 detail 1 (L60881-L60882): "For an operator whose type is
  vpiMultiConcatOp, the first operand shall be the multiplier expression.
  The remaining operands shall be the expressions within the concatenation."
  `{2{n[1:0]}}` has the operands `2` and `n[1:0]`. Operands are the
  expressions as written; the conversions the design applies to them are
  not objects (decision A19-D12), so `a` in `a + b * 2` is the 8-bit `tb.a`
  while the sum is 32 bits wide (`2` is an `int`, 11.6.1). A reference to a
  net or variable is that object (`vpiReg`, `vpiIntVar`, `vpiRealVar`).
- Decompilation. 37.55 detail 2 (L60883-L60885): "The property
  vpiDecompile shall return a string with a functionally equivalent
  expression to the original expression within the source code. Parentheses
  shall be added only to preserve precedence. Each operand and operator
  shall be separated by a single space character. No additional white space
  shall be added due to parentheses." So `(a + b) * (a - b)` keeps its
  parentheses and `a + b * 2` needs none; the spelling of unary operators,
  literals and casts is decision A19-D11 (`~ a`, `8'shf0`, `1.0`,
  `int'(r)`). A variable decompiles to its name.
- Selects. 37.55 detail 10 (L60961-L60962): "For a part-select or indexed
  part-select, the vpiParent object shall correspond to the expression
  formed by removing the part-select range from the expression represented
  by the part-select or indexed part-select itself." `mem[i][3:0]` has the
  parent `mem[i]`, a `vpiVarSelect` (37.19) whose parent is the array
  `tb.mem` and whose index is `i`. 37.54 detail 3 (L60813-L60815): a
  bit-select is a constant select when "every associated index expression is
  an elaboration time constant expression, and — vpiConstantSelect returns
  TRUE for the parent of the bit-select"; `w[17]` and `a[3:0]` are, `w[i *
  4 +: 8]` and `mem[i]` are not (detail 9, L60941-L60944, and 37.19 detail
  1, L58988-L58991, for part and var selects). `pk[1]` selects one element of
  the packed dimension `[0:3]` of `pk`, a `vpiBitSelect` of 8 bits, and
  `pk[i -: 2]` two elements of 16 bits (`pk[1:2]` for `i = 2`, 11.5.1).
  `w[17]` is outside `w[15:0]` and reads `x` (11.5.1).
- Values. 38.15 (L62541-L62542): "The VPI routine vpi_get_value() shall
  retrieve the simulation value of VPI objects." With `a = 8'h5a`,
  `b = 8'h0f`, `n = 4'b10x1`, `s = -3`, `i = 2`, `r = 1.5`, `w = 16'hbeef`
  and `pk = {1, 2, 3, 4}`: `a + b * 2 = 0x78`, `(a + b) * (a - b) = 0x69 *
  0x4b = 0x1ec3`, `a > b` so the conditional is `a - b = 0x4b`, `-s >>> 1 =
  1`, `mem[2][3:0] = 3`, `a ==? 8'b0101xxxx = 1` (the x bits of the right
  operand match anything, 11.4.6), `!n = 0` (`n` has a 1 bit), `r * 2.0 =
  3`, `int'(r) = 2` (rounded, 6.24.1) plus `8'shf0 = -16` gives `-14`. A
  function call (`twice(3)`) and a system function call (`$clog2(i)`) have no
  value in llg (decision A19-D12): their lines and the assignments that hold
  them print `value=error`. After `a = 1` and `i = 1`, `$again` reads `0x1f`,
  `16 * -14 = 0xff20`, `0001x1x1`, `b - a = 0x0e`, `w[4 +: 8] = 0xee`,
  `mem[1][3:0] = 2`, `pk[0:1] = 0x0102` and `a ==? ... = 0`.
- Calls. 37.38 relates a function call to its `vpiFunction` and its
  `vpiArgument` expressions; `twice` is `tb.twice` with `vpiFuncType`
  `vpiSizedSignedFunc` (5). `$clog2` is built in (`vpiUserDefn` 0).
- Arguments and connections. `$exprs` declares typed formals
  (`logic [7:0]`, `real`, `string`); its arguments are the operation `~a &
  b`, the real division `r / 4 = 0.375` and the string constant `"hi"`
  (`vpiStringConst`, 6), each decompiled. The connection of `u.a` is the
  operation `a ^ b = 01010101`.

## statements (37.56-37.74, 37.12)

`statements.c` walks the statements of every process (source order, 37.58),
task and function of `tb` through `vpiStmt`. Each line shows the statement
type, `vpiLineNo`, the file name of `vpiFile` and `vpiName` when it has one,
then by type: `vpiBlocking` and `vpiOpType` of assignments with the
decompiled `vpiLhs`, `vpiRhs` and timing control; `vpiQualifier`,
`vpiCaseType` and `vpiLocalVarDecls`; the decompiled `vpiCondition`; the
`vpiDelay` of a delay control; the event of an event statement; the target
of `disable`; the task and the arguments of calls. Sub-statements follow
indented (`vpiStmt`, `vpiElseStmt`, case items with their `vpiExpr`, `for`
init and increment statements). It ends with `tb.main` found by name, a
`cbStmt` registration and `vpiStmt` of a variable.

- Processes. 37.58 diagram (L61076-L61092): a process has one `stmt`. The
  `always #5 clk = ~clk` statement is a delay control (37.63) whose
  statement is the assignment; `always @(posedge clk or negedge clk)` an
  event control (37.60) whose condition is one `vpiEventOrOp` (decision
  A19-D13). The nonblocking `q <= #1 a + b` is an assignment with
  `vpiBlocking` 0 and its delay control; 37.63 detail 1 (L61206): "For delay
  control associated with assignment, the statement shall always be NULL."
- Assignments. 37.59 detail 1 (L61124-L61126): "vpiOpType shall return
  vpiAssignmentOp for normal assignments (both blocking ‘=’ and nonblocking
  ‘<=’). For assignment operators, vpiOpType shall return a value that
  corresponds to the operator that is combined with the assignment". `a +=
  2` and `count += by` print `op=24` (`vpiAddOp`), plain assignments
  `op=82`; `i++` and `k++` are assignments with `vpiPostIncOp` (62) and no
  right-hand side (decision A19-D13). `a = repeat (2) @(posedge clk) b`
  has a `vpiRepeatControl` (37.64) with its count and event.
- Blocks. 37.12 detail 1 (L58069-L58070): "A named begin or named fork shall
  always be a scope." `main` is the catalog `vpiNamedBegin` `tb.main` (also
  found by name, with the same first statement); its declaration `int k`
  is not a statement. The `fork` holds its two branches.
- Tasks and functions. 37.12 detail 5 (L58098-L58100): "A task func can
  have zero or more statements (see 13.3 and 13.4). If the number of
  statements is greater than 1, the vpiStmt relation shall return an unnamed
  begin that contains the statements of the task or function. If the number
  of statements is zero, the vpiStmt relation shall return NULL." `bump` and
  `clamp` print a `vpiBegin` at their first statement's line; `nothing`
  prints `stmt NULL` with no error. `return v` is a `vpiReturnStmt` whose
  `vpiCondition` is `v` (37.73).
- Conditions and loops. `if`/`else if` chains nest `vpiIfElse` objects
  (37.66) with `vpiQualifier` 0, `priority if` has `vpiPriorityQualifier`
  (2) and `unique case` `vpiUniqueQualifier` (1) with `vpiCaseType`
  `vpiCaseExact` (1). 37.67 detail 1 (L61292): "The case item shall group
  all case conditions that branch to the same statement."; detail 2
  (L61293): "vpi_iterate() shall return NULL for the default case item".
  `for (int i = 0; ...)` declares its variable (37.12 detail 2, L58081:
  "A for statement shall be a scope if and only if the vpiLocalVarDecls
  property returns TRUE"), so it prints `local_vars=1` and an init
  assignment to `i`; `repeat`, `while` and `do ... while` print their
  conditions, `foreach (arr[j])` its array and loop variable `j` (37.70
  details 1-2, L61358-L61361), `forever` its begin, and `break`/`continue`
  are `vpiBreak`/`vpiContinue`.
- Waits, events and the rest. `wait (count > 0)` is a `vpiWait` (37.62);
  `@(ev or posedge clk);` and `#2;` control a `vpiNullStmt` (decision
  A19-D13); `-> ev` is a `vpiEventStmt` whose `vpiNamedEvent` is `tb.ev`
  (37.57); `disable main` names `tb.main` through `vpiExpr` (37.72);
  `force` has `vpiLhs` and `vpiRhs`, `release` only `vpiLhs` (37.74). A
  system task call prints its name and decompiled arguments.
- Refusals. llg has no per-statement callbacks: `vpi_register_cb` with
  `cbStmt` returns NULL with `vpiError` (level 3), and `vpiStmt` of a
  variable is not a relation (`vpiError`).
- The run. The design prints `count 11 a 7 b 7 q 14` at time 201: `count =
  clamp(0 + 20) = 10`, then `bump(1)`; `a` doubles from 5 to 10, so `b = 2`,
  then the fork sets `b = 9` (the wait is already satisfied) and `b = 7`
  one unit later, and `a = b` after two clock edges; `q` is released after
  its force and the next clock edge drives `a + b = 14`.

## timing (37.35, 37.36)

`timing.c` walks every module instance from `vpi_iterate(vpiModule, NULL)`
and prints its `vpiModPath` and `vpiTchk` objects in source order with
`vpiLineNo`, then checks two refusals and prints `failures=0` when every
relation pointed back at its origin (`vpiParent` of a term is its path or
check, `vpiModule` is the instance).

- Paths. The 37.35 diagram (L59873-L59900) gives the path terms
  `vpiModPathIn`, `vpiModPathOut` and `vpiModDataPathIn` with
  `vpiDirection`, `vpiEdge` and `vpiExpr`, the path's `vpiCondition`,
  `vpiPathType`, `vpiPolarity`, `vpiDataPolarity` and
  `vpiModPathHasIfNone`. `(a +=> y)` is a parallel path (`type=2`) with
  polarity `vpiPositive`; `(posedge clk => (q +: d))` has edge
  `vpiPosedge` (`0x0d`) on its input, `d` as data path input and data
  polarity `vpiPositive`. `if (en) (b *> y)` is a full path (`type=1`)
  whose condition is `en`; it and `ifnone (b *> y)` report `ifnone=1`
  (decision A19-D9).
- Checks. 37.36 details 1-2 (L59948-L59951): "the relationship
  vpiTchkRefTerm shall denote the reference_event or
  controlled_reference_event, while vpiTchkDataTerm shall denote the
  data_event, if any" and "When iterating over vpiExpr from a tchk, the
  handles returned for a reference_event, a controlled_reference_event, or a
  data_event shall have the type vpiTchkTerm." Every timing check task
  appears once with its `vpiTchkType` (`$setup` 1 to `$timeskew` 12); for
  `$setup` the reference term is `clk` (second argument), the `&&& en`
  condition sits on the data term of `$hold`, `negedge` is `0x32`,
  `$period` and `$width` have no data term, and `vpiTchkNotifier` is
  `notifier` only for `$setup` (decision A19-D10).
- Refusals. `vpi_iterate(vpiExpr)` on a path and `vpi_get(vpiTchkType)` on a
  path are `vpiError` (`level=3`).
