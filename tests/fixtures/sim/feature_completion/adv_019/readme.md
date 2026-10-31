# ADV-019 fixtures: VPI typespecs, structure and union members, structure, expression, statement, dynamic, class and constraint objects

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
fails; `NULL` is a NULL handle. Children come in declaration order (S42-D12;
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
  `w[17]` is outside `w[15:0]` and reads `x` (11.5.1). A select is named by
  its text (`name=mem[i][3:0]`, decision A19-D24).
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
  `op=82`; `i++` and `k++` are `vpiAddOp` assignments whose right-hand side
  is the constant 1 (decision A19-D13). `a = repeat (2) @(posedge clk) b`
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
  `clamp` print a `vpiBegin` at the line of the token after their name (the
  `;` ending the header or the port list), and a written `begin` reports the
  line of its keyword (the `default` item, line 35; decision A19-D13); `nothing`
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
  assignment to `i`, and as a scope it is named `unnamed$$_0` (decision
  A19-D27; the `foreach`, detail 3 L58083: "The scope of each loop control
  variable in a foreach stmt shall be the foreach stmt.", is `unnamed$$_1`
  of the block `main`); `repeat`, `while` and `do ... while` print their
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
  `vpiModPathHasIfNone`. A path is named after its instance, first input
  and first output (`tb.u.a$y`) and is not found by that name (decision
  A19-D9). `(a +=> y)` is a parallel path (`type=2`) with
  polarity `vpiPositive`, which is also its data polarity (no data source);
  `(posedge clk => (q +: d))` has edge `vpiPosedge` (`0x0d`) on its input,
  `d` as data path input and data polarity `vpiPositive`. `if (en) (b *> y)`
  is a full path (`type=1`) whose condition is `en` (`ifnone=0`); only
  `ifnone (b *> y)` reports `ifnone=1` (decision A19-D9).
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
  `notifier` only for `$setup`; `vpiExpr` gives the notifier right after
  the terms (decision A19-D10).
- Refusals. `vpi_iterate(vpiExpr)` on a path and `vpi_get(vpiTchkType)` on a
  path are `vpiError` (`level=3`).

## dynamic (37.3.7, 37.17, 37.27-37.29)

`dynamic.c` prints, for each module variable, `vpiType`, `vpiName`,
`vpiFullName` (or the status level), and then by kind: for an array var
`vpiSize`, `vpiArrayType` and every element of `vpi_iterate(vpiMemoryWord)`
with its `vpiIndex`, `vpiArrayMember` and value (class var elements print
`vpiObjId` and whether `vpiClassObj` is set); for a class var its value
status, `vpiObjId`, the `vpiName` of its `vpiClassTypespec` and the class obj
it refers to with every property of `vpi_iterate(vpiVariables)`; otherwise
`vpiSize` where it is defined and the value (`vpiIntVal`, `vpiRealVal` or
`vpiStringVal` by type). It then walks the class typespecs and definitions,
reads by name, writes values that the design prints, deletes `q[0]` and reads
the element handles it held.

- Values. 37.17 detail 11 (L58766): "Variables of type vpiArrayVar or
  vpiClassVar do not have a value property." Class vars and array vars
  print `value: level=3`. A chandle var reads as its 64-bit pointer image
  (`tb.ch` is null: `size=64 value=0`, decision A19-D21); a string var reads as `vpiStringVal` and,
  in other formats, as its packed image (`tb.s hex=6869`, decision A19-D21).
- Sizes. 37.17 detail 9 (L58760-L58762): "vpiSize for a variable array shall
  return the number of variables in the array. ... For a string var, it shall
  return the number of characters that the variable currently contains."
  `tb.s` has 2 characters, `name` 3, `da` 3 elements, `q` 2, `aa` 2, `sa`
  1; `vpiSize` of a real property is not defined and is not printed.
- Array types. 37.17 detail 21 (L58819): "vpiArrayType can be one of
  vpiStaticArray, vpiDynamicArray, vpiAssocArray, or vpiQueue." The
  `sv_vpi_user.h` codes are `vpiDynamicArray` 2, `vpiAssocArray` 3 and
  `vpiQueueArray` 4.
- Elements. 37.17 detail 18 (L58780-L58781): "If a variable is an element of
  an array (the vpiArrayMember property is TRUE), the vpiIndex iterator shall
  return the indexing expressions that select that specific variable out of
  the array." Elements are named `<array>[<index>]`; associative elements come
  in key order (`aa[-2]` before `aa[5]`), and a string key prints quoted; its
  index is a `vpiStringConst` constant holding the key, whose `vpiIntVal` is
  the key's packed image (`sa["k"]`, `index=107`, decision A19-D23).
- Object identity. 37.29 detail 1 (L59610-L59612): "The property vpiObjId is
  a class object's identifier. It is a property of a live object and
  guaranteed to be unique with respect to all other dynamic objects that
  support this property for as long as the object is alive." Detail 2
  (L59613-L59614): "For a class var, its vpiObjId is the identifier of the
  object it references or 0, indicating it is not referencing any object."
  llg numbers objects from 1 in the order VPI first sees them (decision
  A19-D22): `p` refers to object 1, `p.next` to 2, `none` and `ring[1]` are 0,
  and `ring[0]` refers to object 1 (`same object=1`).
- Declared and created class. 37.29 detail 5 (L59619-L59622): "For a class
  var, vpiClassTypespec shall return the class typespec with which the class
  var was declared in the SystemVerilog source text. If the class var has the
  value of NULL, the vpiClassObj relationship applied to the class var shall
  return a null handle. vpiClassTypespec when applied to a class obj handle
  shall return the class typespec with which the class obj was created." `p`
  is declared `Packet` and refers to a `LinkedPacket` (the clause's own
  example); `none` prints `obj=null`.
- Properties. 37.29 detail 6 (L59648-L59649): "From a class obj, the
  iterations over vpiVariables ... shall return both static and automatic
  properties". The object's properties come in declaration order, inherited
  ones first (`id`, `name`, `weight`, `next`, `data`, then `tag`). 37.17
  detail 25 (L58825): "A non-static data member of a class var does not have
  a vpiFullName property." Every property prints `full: level=3`.
- Lifetime. 37.3.7 (L57563-L57564): "vpiDynamicScheme indicates the object
  was allocated in dynamic memory and may be a class object or part
  thereof." The class obj prints `alloc=2` (`vpiDynamicScheme`) and
  `automatic=1`; L57544-L57545: "vpiAutomatic is also a property of a class
  defn or class typespec, indicating the default lifetime for variables of
  any of its declared tasks/functions", which is automatic for class
  methods.
- Definitions. 37.28 detail 9 (L59561): "The vpiExtends relation shall return
  the base class typespec, if any, from which a given class typespec is
  derived." 37.27 detail 6 (L59478-L59479): "The relation from extends to class
  typespec provides the base class." 37.27 detail 5 (L59477): "The
  vpiDerivedClasses iterator shall return all the class defns derived from
  the given class defn." `LinkedPacket` extends `Packet`; `Packet` has no
  `vpiExtends` (NULL, no error) and derives `LinkedPacket`. `vpiClassType` is
  `vpiUserDefinedClass` (3). A definition's `vpiVariables` are the properties
  it declares itself (inherited ones are reached through `vpiExtends`), and
  37.27 detail 2 (L59481-L59482): "vpi_get_value() and vpi_put_value() are not
  allowed for variable and event handles obtained from class defn handles."
  Each prints `value: level=3`.
- Names. 37.29 detail 9 (L59657-L59658): "vpi_handle_by_name() shall accept a
  full name to a non-static data member, even though it does not have a
  vpiFullName property." `tb.p.id` finds the property; `tb.p.next.id`,
  `tb.q[0]` and `tb.aa[5]` find a property and elements. A name past the
  current size, a missing key or an unknown property is NULL.
- Writes. Integral elements and properties take `vpiIntVal`; a string takes
  `vpiStringVal` and `vpiIntVal`, cast as 6.16 casts an integral value
  (`put int to tb.s level=0`, decisions A19-D21, A19-D25); a class
  obj has no value (`level=3`). The design prints the written values. After
  `q.delete(0)` the handle held for `tb.q[1]` reports `level=3` and the one
  held for `tb.q[0]` reads the element now at index 0 (`2`): element handles
  select by index or key at each access (decision A19-D23).

## classes (37.10, 37.13, 37.17, 37.26, 37.27, 37.37)

`classes.c` lists the class definitions of the design (`vpi_iterate(vpiClassDefn,
NULL)`), of `tb`, and of each package, and then prints for every definition its
full name, qualifiers and scopes, its extends object, derived classes,
methods with their arguments, parameters with their values, properties, and
the counts of its internal scopes, named events and property declarations. It
then prints the properties, method and parameter counts of the object `obj`
refers to and the qualifiers of three module variables. A refused integer
query prints `-99`; `level=` is the status of the call before it.

- Listing. The text does not say what `vpi_iterate(vpiClassDefn, NULL)` lists;
  llg lists every class definition of the design, like another simulator
  (decision A19-D36): the package class, the `$unit` class and the two
  module classes; `tb`, `$unit` and `cls_pkg` list their own. A variable
  has no class definitions (`level=3`). `Base #(W = 4)` is specialized once
  (`Base #(8)`) and is listed once as `Base`, with that specialization's
  parameter values (37.28 detail 4, L59545: the name of a specialization is
  "valid, though tool-dependent").
- Full names. 37.10 detail 5 (L57952-L57954): "vpiFullName for objects that
  exist in a package shall begin with the name of the package followed by
  "::". ... The . separator shall be used in all cases except package and
  class defn." Hence `cls_pkg::PkgItem`, `$unit::UnitItem`, `tb.Base`.
  37.37 detail 5 (L60004-L60005): a method's full name is the class defn's
  full name, `::` and the method name (`tb.Base::vf`); parameters follow
  the same form (`tb.Base::W`); io declarations have no full name.
- Scopes. 37.10 details 2 and 3 (L57934-L57936): `vpiModule` is the module
  the object is inside, otherwise NULL (`module=null` for the package and
  `$unit` classes); `vpiInstance` is the immediate instance (`cls_pkg`,
  `$unit`, `tb`). A method's and a parameter's `vpiScope` is its class defn,
  which iterates its methods as `vpiInternalScope` (37.27 diagram,
  L59513-L59514): 1, 0, 5 and 2 scopes (decision A19-D36). Classes declare
  no named events (llg rejects event properties) and no property
  declarations (A.1.9 `class_item` has none): both iterations are NULL
  without an error.
- Qualifiers. 8.20 makes `Base` virtual (`virtual=1`); classes and their
  methods have automatic lifetime (8.6), so `vpiAutomatic` is 1. 37.37
  detail 4 (L60001-L60003): `vpiVisibility` is `vpiPublicVis` (1),
  `vpiProtectedVis` (2) or `vpiLocalVis` (3): `lf` is local, `pt`
  protected. `vf` is virtual in `Base` (pure virtual) and in `Derived`
  (8.20: a method that overrides a virtual method is virtual). Built-in
  methods are not listed (37.27 detail 1, L59458-L59460), so `Base` lists
  `vf`, `new`, `lf`, `pt` and `sf` in declaration order.
- Function types. Annex M: `vpiSizedSignedFunc` 5 for `int` results; `new`
  and the void function `lf` have no function type (refused) and size 0
  (37.37 detail 12, L60024-L60026: "For a void function, vpiSize shall
  return 0"). A method's statements are not VPI objects in llg
  (`stmt level=3`).
- Arguments. A method iterates its declared arguments (`vpiIODecl`, without
  the implicit `this`): `vpiDirection` `vpiInput` 1, `vpiOutput` 2 and
  `vpiRef` 6 (decision A19-D42), with `vpiTypespec`, `vpiTaskFunc` the
  method and no `vpiExpr` (decision A19-D42).
- Extends. 37.27 detail 6 (L59478-L59479): "The relation from extends to
  class typespec provides the base class": `Derived` has an extends object
  whose class typespec belongs to `tb.Base`; `Base` lists `tb.Derived` as
  derived (detail 5).
- Parameters. 37.27 detail 8 (L59482-L59484): `vpiParameter` returns the
  parameter port list and the body parameters; `vpiLocalParam` is TRUE for
  `LP`. `W` is 8 in the only specialization and `LP` is `W * 2` = 16;
  both are `vpiBinaryConst` (3) and 32 bits wide. The object's class `Derived`
  declares no parameters (`object parameters=0`) and two methods.
- Properties. 37.17 details 15, 16 and 24 (L58775-L58777, L58822-L58824):
  `vpiRandType` is `vpiNotRand` 1, `vpiRand` 2 or `vpiRandC` 3; `cv` is
  `const` (`vpiConstantVariable` 1). A property obtained from a class defn
  belongs to no object and is not randomized (decision A19-D36); in `obj`,
  `a` and `d` are active and `b` is off after `obj.b.rand_mode(0)` (18.8).
  Module variables are not class members: `vpiNotRand`, not randomized,
  `vpiPublicVis`; `limit` is const.
- `plain=8` is `vf(3)` (3 + `d` 0) + `getx()` 5 + `u` 0.

## constraints (37.30, 37.34, 18.5, 18.9)

`constraints.c` prints, for the constraints of class `Packet` obtained from
its class defn, their name, full name, `vpiAutomatic`, `vpiVirtual`,
`vpiAllocScheme`, `vpiIsConstraintEnabled`, `vpiParent` type and line, and
the tree of their constraint items: each object's type and line, the
relations of 37.30 and 37.34 and, for an expression, its name, decompiled
text and operands. It then prints the constraints obtained from the object
`p` refers to. `$modes` prints both lists again after the design switches
the static constraint off and `c_small` back on. `-99` marks a refused
integer query.

- Constraints. 37.30 (L59681-L59723): a constraint has the properties above
  and a class obj as `vpiParent`; from a class defn its parent is the defn
  (decision A19-D37). Detail 1 (L59722-L59723): "vpiAutomatic == 0 implies
  the constraint was declared static" (`c_shared`). 37.10 detail 5
  (L57952-L57954): the full name is the class defn's, `::` and the name.
  2009 declares no virtual constraints: `vpiVirtual` 0.
- Allocation. 37.3.7 (L57548-L57564): `vpiDynamicScheme` (2) for an object
  "allocated in dynamic memory ... or part thereof", `vpiOtherScheme` (3)
  otherwise: the object's non-static constraints are 2, the static one and
  every constraint from the class defn 3.
- State. 18.9: `constraint_mode(0)` turns a constraint off in one object, and
  for a static constraint in every object. After `randomize()` and
  `p.c_small.constraint_mode(0)`, the object reports `c_small` 0 and the
  others 1; a non-static constraint from the class defn belongs to no object
  and reports `vpiUndefined` (-1) without an error, as another simulator
  does where the text is silent; the static one reports its class-wide
  state (decision A19-D37).
  `$modes` sees `c_shared` 0 from both and `c_small` 1 in the object.
- Items. 37.30: a constraint item is a constraint ordering, a distribution or
  a constraint expression; 37.34 (L59855-L59872): a constraint expression is
  an implication, a constr if, a constr if else or an expression. A braced
  set lists its items in place. `c_mix` holds one of each: the distribution
  `d dist {...}` with `vpiExpr` `d` and three `vpiDistItem`s (`vpiDistType`
  `vpiEqualDist` 1 for `:=` and for `7`, whose weight 18.5.4 defaults to
  `:= 1` and which has no `vpiWeight`; `vpiDivDist` 2 for `:/`; the range
  `[1:3]` a `vpiRange` with its bounds), the ordering `solve a before d`
  (`vpiSolveBefore` `a`, `vpiSolveAfter` `d`), the implication with its
  `vpiCondition` and two `vpiConstraintExpr`s, the `if`-`else` with
  `vpiElseConst` and the `if` whose expression `d inside {[0:20], 30}` has a
  `vpiRange` operand. Class properties in constraints are references that
  name the property (`a`, `len`, `d`).
- `ok=1`: the randomization succeeds.

## scopes (37.10, 37.12, 37.17, 37.29)

`scopes.c` finds by name the class variable, string and queue that package
`dp` declares, an element of the queue, the static string of the named
block `blk` and the static class variable of the static task `remember`,
printing each object's type, full name and value, object id or size; it
then lists the variables of `dp` and of `tb.blk`.

- Static dynamic objects. 37.17 does not restrict string, class and
  container variables to instances: a package, a named block and a static
  task declare them too, and they are found by name and listed by
  `vpiVariables` like the other variables of their scope (37.12; declaration
  order, S42-D12). 37.10 detail 5 (L57952-L57954): package members are named
  `dp::...`. Object ids follow the order VPI first sees the objects
  (decision A19-D22): the package's object is 1, the object `held` and
  `keep` refer to is 2.
- Virtual interfaces. `tb.keep.vif` is a virtual interface property, which
  llg does not expose (docs/known_issues.md): it is not found (`null`).
- The design prints `blk 5 pk 2`.

## gc (37.3.7, 37.29, 38.36.1)

`gc.c` registers `cbCreateObj` and `cbReclaimObj` on the `Node` and `Leaf`
class typespecs, `cbValueChange` on `tb.tmp`, takes handles to the object
`tb.tmp` refers to after `make(2)` (the object, `tb.tmp.v`,
`tb.tmp.items[0]` and the object's constraint `c_r`) with `cbReclaimObj` and `cbEndOfObject` on that object,
and prints at each `$check` the sorted values of the objects reclaimed since
the last one. The test runs with `LLG_GC_STRESS=1 LLG_GC_VERIFY=1`: a
collection runs at every safe point after an allocation, so an object is
reclaimed in the time step it becomes unreachable, and an access to a
reclaimed object would be a fatal error.

- Registration. 38.36.1 (L64692-L64695): "For cbCreateObj, this field shall be
  assigned a handle to a class typespec object. For a cbReclaimObj, this
  field shall be assigned either a handle to a class typespec or a class obj.
  With a class typespec, any class object of that type shall generate a
  callback." `cbCreateObj` (700) on a class var is refused (`level=3`).
- Creation. 38.36.1 (L64650-L64652): cbCreateObj runs "After the class
  constructor call has completed and the internal state of a class object
  has been initialized, or for shallow copy, after the copy operation has
  completed." `keep` (v=1), `make(2)`, the shallow copy `new keep` (v=1),
  `cycle()` (10, 11) and `leaf()` (20, a `Leaf`, reported only by the
  `Leaf` callback: an object's type is the class it was created with,
  decision A19-D22). Objects receive `vpiObjId` 1 to 6 in that order.
- Reclamation. 38.36.1 (L64653-L64656): cbReclaimObj runs "Before the class
  object has been reclaimed by the automatic memory management, when it has
  been marked as no longer being used. When control is returned from this
  callback, any handles to this class object, its properties or their
  subelements, and any associated callbacks should be considered invalid."
  L64775-L64778: "The time field of the s_cb_data structure will be NULL. ...
  All VPI properties of the class obj are accessible. Using this handle as a
  reference for purposes of navigation or registering callbacks is
  undefined." In the held object's callback its properties read (`v=2`),
  while its constraint's state, navigation through `link` and a write to
  `v` are refused (`level=3`, decision A19-D22). The `cbEndOfObject` callback (L64672-L64676) runs for
  the same object with a NULL time. Object 2 dies when `copy()` replaces
  it (check 1), the cycle 10-11 when `cycle()` returns (check 2), the leaf
  when `leaf()` returns (check 3) and the copy when `tmp` becomes null
  (check 4); `keep` stays.
- Stale handles. After check 1's collection the held object, property,
  element and constraint handles report `level=3` and `vpiObjId` reads `vpiUndefined` (-1);
  `tb.tmp.v` resolves again to the property of the object `tmp` now refers
  to.
- Class var changes. 38.36.1 (L64766-L64767): "The cbValueChange callback may
  be placed on a class var and will be called when its value changes, which
  indicates that it is referring to a new dynamic object (including a newly
  constructed one) or no object." `tmp` changes in `make(2)`, `copy()` and
  `tmp = null`: 2 changes at checks 1-3, 3 at check 4.

## declarations (37.10, 33.7)

`declarations.c` prints, at time 1, the definition and configuration
properties of `tb` and `tb.u`; the test elaborates configuration `cfg`
(`--top cfg`), whose design statement selects `work.tb` (33.4.1). File
names print without their directory; a refused query prints `error=<level>`.

- Configuration. 33.7 (SystemVerilog-1800-2009.txt L54034-L54039):
  "vpiLibrary—the library name into which the module was compiled",
  "vpiCell—the name of the cell bound to the module instance",
  "vpiConfig—the library.cell name of the config controlling the binding of
  the module instance". `cfg` binds both instances, so both report
  `work.cfg`; 33.3.1 (L53381-L53382): a file that matches no library "shall
  by default be compiled into a library named work", so `vpiLibrary` is
  `work`; the cells are `tb` and `leaf`.
- Definition. 37.10 (diagram, L57912-L57915): "definition location int:
  vpiDefLineNo str: vpiDefFile": `tb` is declared on line 8 and `leaf` on
  line 5 of `declarations.sv`. `vpiTop` (L57922-L57923) is TRUE for `tb`
  only. No envelope is protected (34), so `vpiProtected` is FALSE.
- Directives. Without `` `celldefine``, `` `default_nettype`` or
  `` `unconnected_drive`` the instances report `vpiCellInstance` 0,
  `vpiDefNetType` `vpiWire` (1; 22.8 L39771-L39772: "When no
  `default_nettype directive is present ... implicit nets are of type
  wire"), `vpiUnconnDrive` `vpiHighZ` (1; Annex K L75188: "No default drive
  given") and `vpiDefDelayMode` `vpiDelayModeNone` (1; L75194: "no delay mode
  specified"); both definitions are static, so `vpiAutomatic` is 0 (37.3.7,
  decision A19-D41).
- `y=1` is `~a` with `a` 0.

## runtime (37.32, 37.38, 37.54, 37.75, 37.76, 37.79)

`runtime.sv` instantiates a combinational and a sequential UDP, drives a
vector `r` through a continuous assignment and a UDP, and calls
`$runtime(1)` at 1 ns and `$runtime(2)` at 3 ns (`timescale 1ns/1ns`);
`en` changes at 5 ns. `runtime.c` registers `$runtime` and the unused
function `$runtime_unused`. `adv_019::runtime_structures_list_systfs_callbacks_time_queues_uses_and_udps`
runs it in both optimizer modes with both value backends. Every line of
`runtime.out` follows from these clauses (SystemVerilog-1800-2009.txt):

- `vpiUserSystf $runtime`, `vpiUserSystf $runtime_unused`: 37.38 detail 6
  (L60096-L60097) "All user-defined system tasks or functions shall be
  retrieved using vpi_iterate(), with vpiUserSystf as the type argument,
  and a NULL reference argument." (registration order, decision A19-D39).
- The callbacks of NULL list only `eos`: 37.75 detail 2 (L61475) "To get
  callback objects not related to the above objects, the second argument to
  vpi_iterate() shall be NULL." The value change callback belongs to `tb.r`
  (an expression), the delay callbacks to their time queue, `removed` was
  removed and `eot` (cbAtEndOfSimTime) is a time callback of the current
  slot. The callbacks of `tb.r` list `vc`, whose iterated handle compares
  equal to the registered one and whose vpi_get_cb_info (37.75 detail 1,
  L61474) reports its reason 1 (cbValueChange) and object. `tb` (a scope,
  hence a stmt) has none; `tb.u1` (a primitive) is no callback owner, so the
  query fails (decision A19-D26).
- Time queue at 1 ns: 37.76 detail 1 (L61491) increasing order; detail 3
  (L61493) "The current time queue shall only be returned as part of the
  iteration if there are events that precede read only sync": at 1 ns only
  the end-of-time callback remains, so the queues are 4 ns (`ad`, cbAfterDelay
  3 from 1 ns, listed under it) and 5 ns (`en`). The `#2` of the calling
  process is not armed yet. At 3 ns the same two queues remain.
- UDP definitions: 37.32 (L59798-L59825): `vpiSize` the number of inputs,
  `vpiPrimType` 28 (vpiCombPrim) and 27 (vpiSeqPrim, detail 2 L59825),
  `vpiProtected` 0, the line of each `primitive`; vpiName is not a udp defn
  property (error); io declarations in port order with vpiDirection 2
  (output) and 1 (input), scalar, their vpiUdpDefn the same definition; the
  table entries decompiled as in decision A19-D38 with vpiSize 3 (two inputs
  and the output) or 4 (with the state); vpiInitial fails (llg keeps the
  initial value only). `tb.u1` reaches `rt_or`.
- Uses of `tb.r` (37.54, L60774-L60812, decision A19-D43): the continuous
  assignment, the two UDP terminals, and the assignment `r = 1`; the first
  use of `tb.r[0]` is its UDP terminal (the continuous assignment reads bit
  1 only). 37.79 details 1 and 2 (L61647-L61649): the net iterator of `tb`
  reports `tb`, the module iterator from NULL reports NULL without an error,
  and once the bit-select reference is released vpiUse fails.

## threads (37.25, 37.39, 37.40, 38.36.1)

`threads.sv` runs an initial procedure that calls the automatic task `t1`
(which waits `#1`), the recursive function `fact`, a `fork`/`join` of two
branches and a trigger of `ev`, and an always procedure waiting on `ev`.
`threads.c` prints, at each `$threads(n)`, the frame chain from
`vpi_handle(vpiFrame, NULL)` and every thread of `vpi_iterate(vpiThread,
NULL)`, places the frame and thread callbacks and queries a frame and a
thread it held after they ended.
`adv_019::threads_and_frames_follow_processes_and_subprogram_calls` runs it
in both optimizer modes with both value backends. Every line of
`threads.out` follows from these clauses (SystemVerilog-1800-2009.txt) and
decision A19-D44:

- Two threads (the procedures) at step 0 and no frame: 37.40 detail 1
  (L60195-L60197) "A thread is a SystemVerilog process such as an always
  procedure or a branch of a fork construct. ... a new frame is activated as
  each new task or function is entered." Threads are listed in creation
  order, active, with no parent or origin.
- Placements: cbStartOfFrame needs a task or function, cbEndOfFrame a
  frame, cbEndOfThread and cbEnterThread a thread, and cbStartOfThread no
  object; the others fail (38.36.1, L64658-L64671, A19-D44).
- `tb.t1` at steps 1 and 2 is the active frame (37.39 detail 4,
  L60163-L60165) of the main thread (`vpiThread`, compared equal), and its
  statement is the running system task call. cbStartOfFrame ran when `t1`
  was entered (`frame=tb.t1`), cbEnterThread when the main thread resumed
  after `#1`, cbEndOfFrame when `t1` returned.
- Step 2: the always procedure is the one thread waiting on `ev` (37.25
  detail 1, L59303-L59304).
- Step 3: three `tb.fact` frames, only the innermost active, each the
  vpiParent of the one it called; the frame held from `t1` has ended, so
  querying it fails (37.3.8, L57568).
- Step 4: cbStartOfThread ran for both fork branches; the main thread has
  two children whose vpiParent is that thread. A thread has no assertion
  iteration (the 37.40 diagram lists none).
- Step 5: the second branch resumed (cbEnterThread) and its handle is still
  valid; cbEndOfThread ran when it ended. Step 6: the held branch thread has
  ended, so querying it fails (38.36.1 cbEndOfThread, L64666-L64670).

## catalog (A03; 37.3, 38.6, 38.18, 38.23, 40.5.3)

`catalog.sv` holds an object of most kinds the VPI catalog
(`src/sim/rt/vpi_catalog.txt`) lists: instances and instance arrays,
interfaces with a modport, ports, nets and net arrays, every variable kind,
class, string and container variables, parameters, gates, a UDP and a
switch, a module path, a timing check and a specparam, continuous assignments, a generate
loop with its genvar (decision A19-D42), sequences, properties and concurrent assertions, tasks and functions
with most statement and expression kinds, an FSM declared with the
state-vector pragma (decision A17-D11), a package and a `$unit` variable,
a program and a program array (decision A19-D29), and clocking blocks in
the top module, an interface and a program with io declarations, a
default and a global clocking and a `default disable iff` (decision
A19-D30). The walk runs inside the task `catalog_walk`, so it also
reaches a frame, the threads and the system task call through the frame. `catalog.c` has no expected output
file: starting from a NULL reference, it calls `vpi_get` and `vpi_get_str`
with every property name and `vpi_handle` and `vpi_iterate` with every
relation name the catalog or the runtime dispatch knows, on up to 40 objects
of each type it reaches, and `vpi_get_value` with `vpiObjTypeVal`. It prints
one `+` line per query that raised no error. The probe tables are generated:
`tests/runtime_vpi_catalog.rs` checks that they hold exactly the catalog and
dispatch names.

`adv_019::catalog_walk_matches_the_vpi_catalog` runs it in both optimizer
modes and checks, per object type reached:

- each answer is a row the catalog marks `implemented` or `extension` for
  that type (directly or through a class that contains it); vpiType,
  vpiLineNo and vpiFile apply to every object (37.3.2, 37.3.3);
- each `implemented` or `extension` row the catalog gives to this test is
  answered;
- every other query raised a `vpiError` error and returned `vpiUndefined` or
  NULL. 38.6 (L62051-L62052): "Should an error occur, vpi_get() shall return
  vpiUndefined."; one-to-one relations are the single arrows of the
  diagrams (38.18, L63241-L63242) and iterations the double arrows (38.23,
  L63447-L63448). A query outside the diagrams is an error (decision
  A19-D26); a line `!` (an error that still returned a value, handle or
  iterator) fails the test.

The check is per object type: an object type that answers a row on one
object and refuses it on another (for example a statement reached through
different relations) counts as answering it.
