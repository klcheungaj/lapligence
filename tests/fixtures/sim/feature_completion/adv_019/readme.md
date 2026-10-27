# ADV-019 fixtures: VPI typespecs, structure and union members

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
