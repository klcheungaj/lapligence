# SIM-040: DPI-C packed, sized and open-array imports

These fixtures cover imported DPI-C subroutines whose formals are packed
vectors, sized unpacked arrays and structures, or open arrays, together with
the `svdpi.h` routines that read and write them. Each positive `<name>.sv`
has a portable C11 companion `<name>.c`. The test
[`sim_040.rs`](../../../../sim_feature_completion/sim_040.rs) builds the
companion into a shared library and passes it with `--dpi-lib`. It runs on the
legacy, compact/portable and compact/GMP value backends, in both optimizer
modes. The companions build with `-std=c11 -Wall -Wextra -Werror` and avoid
constructs that MSVC `/W4 /WX` rejects (no `strcpy`/`strcat`, no shadowing,
explicit casts).

Line numbers refer to `SystemVerilog-1800-2009.txt` (a `pdftotext -layout`
extraction of IEEE 1800-2009).

## Clauses

35.5.1.2 (L55643-55644):

> The imported function shall not assume anything about the initial values of
> formal output arguments. The initial values of output arguments are
> undetermined and implementation dependent.

35.5.5 (L55985-55988):

> Function result types are restricted to small values. The following
> SystemVerilog data types are allowed for imported function results:
> — void, byte, shortint, int, longint, real, shortreal, chandle, and string
> — Scalar values of type bit and logic

35.5.6.1 (L56048-56050):

> Formal arguments of imported functions can be specified as open arrays.
> (Exported SystemVerilog functions cannot have formal arguments specified as
> open arrays.) A formal argument is an open array when a range of one or more
> of its dimensions is unspecified (denoted by using square brackets, []).

35.6.1 (L56128-56131):

> For the SystemVerilog side of the interface, the semantics of arguments
> passing is as if input arguments are passed by copy-in, output arguments are
> passed by copy-out, and inout arguments were passed by copy-in, copy-out.

H.6.4 (L72022-72023):

> The SystemVerilog simulator is responsible for handling value changes for
> output and inout arguments. Such changes shall be detected and handled after
> the control returns from C code to SystemVerilog code.

H.7.6 c) (L72287-72290):

> The natural order of elements for each dimension in the layout of an
> unpacked array shall be used, i.e., elements with lower indices go first.
> For SystemVerilog range [L:R], the element with SystemVerilog index
> min(L,R) has the C index 0 and the element with SystemVerilog index
> max(L,R) has the C index abs(L-R).

H.7.6 (L72275-72277):

> Each unsized, unpacked dimension has the same range as the corresponding
> dimension of the actual argument. An open array formal argument's unsized,
> packed dimension has the linearized, normalized range of all the actual's
> packed dimensions (see H.7.5).

H.7.7 (L72315-72319):

> A packed array is represented as an array of one or more elements (of type
> svBitVecVal for 2-state values and svLogicVecVal for 4-state values), each
> element representing a group of 32 bits. The first element of an array
> contains the 32 LSBs, next element contains the 32 more significant bits,
> and so on. The last element can contain a number of unused bits. The
> contents of these unused bits are undetermined [...]

H.7.8 (L72330-72332):

> In the case of an unpacked type that consists purely of unpacked elements
> (including subaggregates), the layout presented to the C programmer is
> guaranteed to be compatible with the C compiler's layout on the given
> operating system.

H.12.2 (L73284-73288):

> These functions are modeled upon the SystemVerilog array querying functions
> and use the same semantics (see 20.7). If the dimension is 0, then the query
> refers to the packed part (which is one-dimensional) of an array, and
> dimensions > 0 refer to the unpacked part of an array.

H.12.4 (L73337-73340):

> [...] the address and size of such an array shall be undefined (0, to be
> exact). Nonetheless, the addresses of individual elements of an array shall
> be always supported.

H.10.1.3 (L72914-72915):

> a tool that is based on IEEE Std 1800-2005, i.e., the VPI-based canonical
> value, shall return the string "1800-2005".

35.9 (L56272-56273, L56282-56284):

> b) When an imported task returns due to a disable, it shall return a value
> of 1. Otherwise, it shall return 0. [...] simulators shall implement checks
> to verify that item b), item c), and item d) are correctly followed [...]
> If any protocol item is not correctly followed, a fatal simulation error is
> issued.

## Decisions

Where the text leaves a result open, llg's choice has a portable case in
[`tests/fixtures/sim/lrm_decisions/`](../../lrm_decisions/) and a row in
[docs/lrm_decisions.md](../../../../../docs/lrm_decisions.md):

- S40-D1: output formals start at the type default (X for 4-state, 0 for
  2-state).
- S40-D2: copy-in of every input, then the call, then copy-out in declaration
  order, then the function result.
- S40-D3: `svDimensions` counts the integral packed part; absent dimensions
  query as 0.
- S40-D4: invalid element indices read the default, write nothing, and give a
  NULL pointer; svBit reads of X/Z give 0.
- S40-D5: a nonzero imported-task result with no disable is a fatal error.

## Fixtures and oracles

Every expected output was derived by hand from the clauses above and the
companion's arithmetic, then compared with llg.

| Fixture | Covers (acceptance) | Oracle notes |
| --- | --- | --- |
| `packed_widths` | Packed bit/logic formals of 2..129 bits across 32-bit chunk boundaries, signed and unsigned, `integer`, `time` and packed struct/union/enum types (A01). | H.7.7: chunk 0 holds the LSBs; for logic each chunk is `aval`/`bval` with 0=00, 1=10, Z=01, X=11. |
| `open_arrays` | Open unpacked dimensions (1..3), reversed and nonzero ranges, slices, packed-open `bit []`, open arrays of structures, sized dimensions inside an open formal (A01). | Ranges from H.7.6; element addresses in natural order. |
| `aggregates` | Sized unpacked structures and arrays nested in each other, with reversed ranges, logic members carrying X/Z, string input/output/inout and result (A01, A02). | H.7.8 C layout: lowest index first, members in declaration order. |
| `roundtrip_xz` | X and Z in every chunk of 70-bit vectors, output initial values, aliased input/output/inout actuals, a selected actual `mem[k][7:0]`, result assigned after copy-out, string result order, task outputs, 5000-bit vectors (heap staging) and logic scalars (A02). | Decisions S40-D1 and S40-D2. |
| `svdpi_access` | Every `svdpi.h` routine llg provides: `svDpiVersion`, bit and part selects, all array queries for dims 0..4 and invalid dims, `svGetArrayPtr`, `svSizeOfArray`, `svGetArrElemPtr{,1,2,3}`, `svGet/Put{Bit,Logic}ArrElem{,1,2,3}[VecVal]` with in-range, out-of-range and wrong-count indices (A03). | H.11, H.12; decisions S40-D3 and S40-D4. |
| `scopes_types` | One C function imported from two scopes, including an open-array import whose actual shape differs per scope; `pure` imports; enum, packed-struct and `integer` formals. | 35.5.4: one C function per C name. |
| `neg_*` | Explicit errors: dynamic actuals of open formals, string or real elements in aggregates, aggregates over 1048575 payload bits, packed results, `ref`/`event`/class/unpacked-union formals, open arrays in exports, conflicting signatures, a missing import symbol (with `neg_missing_symbol.c`) and an unprovided routine `svGetScope` (with `neg_unprovided_routine.c`) (A03). | 35.5.5, 35.5.6, 35.5.6.1; llg limits in docs/known_issues.md. |
