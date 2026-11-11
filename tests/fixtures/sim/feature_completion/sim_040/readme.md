# SIM-040: DPI-C packed, sized and open-array imports

These fixtures cover imported DPI-C subroutines whose formals are packed
vectors, sized unpacked arrays and structures (with packed, real, shortreal,
string and chandle leaves), or open arrays, together with the `svdpi.h` routines that read and write them and
the time services. Each positive `<name>.sv`
has a portable C11 companion `<name>.c`. The test
[`sim_040.rs`](../../../../sim_feature_completion/sim_040.rs) builds the
companion into a shared library and passes it with `--dpi-lib`. It runs on the
legacy, compact/portable and compact/GMP value backends, in both optimizer
modes. The companions build with `-std=c11 -Wall -Wextra -Werror` and avoid
constructs that MSVC `/W4 /WX` rejects (no `strcpy`/`strcat`, no shadowing,
explicit casts); on Windows they build with `/W4 /WX`.

Each DPI-C import definition carries `DPI_DLLESPEC`, the Annex I `svdpi.h`
macro that exports it from a Windows DLL and is empty elsewhere (Annex I,
`SystemVerilog-1800-2009.txt` L74027-74031: "/* Use to import a symbol into
application */ / #if defined (_MSC_VER) / #define DPI_DLLESPEC
__declspec(dllexport) / #else / #define DPI_DLLESPEC"). On Windows the
companion is a DLL linked against the design's import library
(`llg --dpi-import-lib`), so its `svdpi.h` calls bind to `sim.exe`. A
prototype of an import definition carries `DPI_DLLESPEC` too, since MSVC
rejects a definition whose `__declspec(dllexport)` an earlier declaration
lacks (C2375); prototypes of the design's exports stay unmarked.

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

H.7.4 Table H.1 (L72189-72193, L72201-72207; the extraction prints the `*`
of `void*` and `const char*` as `a`):

> Table H.1—Mapping data types
> SystemVerilog type C type
> byte char
> real double
> shortreal float
> chandle voida
> string const chara

H.8.10.1 (L72520-72522):

> When strings are contained in aggregate arguments, those string members
> shall also be represented by const char* variables. All the same
> stipulations apply to string members of aggregate arguments as apply to
> standalone string arguments.

H.8.10 (L72486-72488):

> If the pointer value is modified by the C application, SystemVerilog copies
> the indicated string contents into its memory space and undertakes any
> actions sensitive to this change.

20.3.1 (L34168-34169) and 20.3.3 (L34207-34208), for the time services:

> The $time system function returns an integer that is a 64-bit time, scaled
> to the time unit of the module that invoked it. [...] The $realtime system
> function returns a real number time that, like $time, is scaled to the time
> unit of the module that invoked it.

Plain `char` is unsigned on some ABIs (AArch64 Linux, PowerPC, RISC-V), so the
companions read `byte` members through an explicit sign extension (`sv_byte`)
and print the signed SV value on every host; `sim_040.rs` also runs
`aggregates` with `-funsigned-char`.

I.2 (L73970-73971), for the `svdpi.h` the
companions include (llg's copy defines the types with `<inttypes.h>`):

> Implementations shall define the types uint8_t and uint32_t, but the exact
> method of doing so is not prescribed by this standard.

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
  2-state); a structure output, or an array of structures, starts with every
  byte zero.
- S40-D2: copy-in of every input, then the call, then copy-out in declaration
  order, then the function result.
- S40-D3: `svDimensions` counts the integral packed part; absent dimensions
  query as 0.
- S40-D4: out-of-range element indices read the default, write nothing, and
  give a NULL pointer (an out-of-range svBit read gives 0); an svBit read of
  a valid logic element returns its value (aval) bits, so X reads 1 and Z
  reads 0 (`svdpi_access` line `bit view of logic=10`: `l1[0]` is X,
  `l2[0][1]` is Z).
- S40-D5: a nonzero imported-task result with no disable is a fatal error.
- S40-D6: an element access with a wrong number of indices is a fatal error;
  the import's C code is abandoned at the access, nothing is copied out and
  the run ends.
- S40-D8: native leaves cross in the C layout; an output aggregate starts
  with every byte zero and its string leaves at an empty string; changed
  string pointers are all read before any leaf is replaced, so in llg a
  pointer the C function received stays valid until it returns.
- S40-D7: `svGetTime` fills `high`/`low` with the scope's `$time` and `real`
  with its `$realtime` and leaves `type` alone; `svGetTimeUnit` and
  `svGetTimePrecision` store powers of ten in seconds; a NULL scope is the
  simulation time unit; an invalid scope or a NULL result pointer returns -1
  and stores nothing.

## Fixtures and oracles

Every expected output was derived by hand from the clauses above and the
companion's arithmetic, then compared with llg.

| Fixture | Covers (acceptance) | Oracle notes |
| --- | --- | --- |
| `packed_widths` | Packed bit/logic formals of 2..129 bits across 32-bit chunk boundaries, signed and unsigned, `integer`, `time` and packed struct/union/enum types (A01). | H.7.7: chunk 0 holds the LSBs; for logic each chunk is `aval`/`bval` with 0=00, 1=10, Z=01, X=11. |
| `open_arrays` | Open unpacked dimensions (1..3), reversed and nonzero ranges, slices, packed-open `bit []`, open arrays of structures, sized dimensions inside an open formal (A01). | Ranges from H.7.6; element addresses in natural order. |
| `aggregates` | Sized unpacked structures and arrays nested in each other, with reversed ranges, logic members carrying X/Z, string input/output/inout and result (A01, A02). | H.7.8 C layout: lowest index first, members in declaration order. |
| `roundtrip_xz` | X and Z in every chunk of 70-bit vectors, output initial values, aliased input/output/inout actuals, a selected actual `mem[k][7:0]`, result assigned after copy-out, string result order, task outputs, 5000-bit vectors (heap staging) and logic scalars (A02). | Decisions S40-D1 and S40-D2. |
| `svdpi_access` | Every `svdpi.h` routine llg provides: `svDpiVersion`, bit and part selects, all array queries for dims 0..4 and invalid dims, `svGetArrayPtr`, `svSizeOfArray`, `svGetArrElemPtr{,1,2,3}`, `svGet/Put{Bit,Logic}ArrElem{,1,2,3}[VecVal]` with in-range and out-of-range indices (A03). | H.11, H.12; decisions S40-D3 and S40-D4. |
| `svdpi_index_count` | Element pointer, get and put accesses with a wrong number of indices: the first reports one error and abandons the C function, so the later accesses and its last `printf` do not run, `after the call` never prints, the final procedure shows the arrays unchanged; exit status 1 (A03). | H.12.4 L73337-73340; decision S40-D6. |
| `scopes_types` | One C function imported from two scopes, including an open-array import whose actual shape differs per scope; `pure` imports; enum, packed-struct and `integer` formals. | 35.5.4: one C function per C name. |
| `time_services` | `svGetTime`, `svGetTimeUnit` and `svGetTimePrecision` from module scopes at 1 ns/1 ps, 1 us/10 ns and 100 ps/1 fs and a package at 10 ps/1 ps, from a noncontext import (no scope, and a scope found by name), with a NULL scope, an invalid scope and NULL result pointers; a time above 32 bits of simulation time units. | Decision S40-D7; 20.3.1 and 20.3.3 for the scaled values (1234000 fs is 1.234 ns, 123.4 units of 10 ps, 0.001234 us; 1234568 fs is 12.34568 units of 100 ps; 5 ms is 5000000000000 fs = 1164 * 2^32 + 658067456). |
| `native_aggregates` | Structures with real, int, string, chandle, shortreal and byte members, nested structures with arrays (descending range, strings, a packed-only record, a 4-state member), sized arrays of strings (two dimensions, an array of an array type), reals (descending range, two dimensions), shortreals, chandles and records; input, output and inout; two outputs to one actual; an imported task. | H.7.8 layout with Table H.1 types; H.7.3: C index 0 is the lowest SV index at every level (`pair [1:0]`, `o [3:2]`, `a [3:1]`); `tag = 4'b1x0z` is aval c, bval 5 (H.7.7); decisions S40-D1, S40-D2 and S40-D8. |
| `native_open_arrays` | Open arrays of real (one and two open dimensions, a sized second dimension, a descending range), shortreal, string (output and two-dimensional inout), chandle and records with fixed-size actuals; `svGetArrayPtr`, `svSizeOfArray`, `svGetArrElemPtr1/2`; dimension 0 absent. | H.12.4 with the H.7.3 order (`down [4:2]`: raw element 0 is `down[2]`); S40-D3 for dimension 0 of a non-integral element; sums by hand. |
| `neg_*` | Explicit errors: dynamic actuals of open formals, an aggregate over 1048575 payload bits, a slice as the actual of an open array of reals, packed results, `ref`/`event`/class/unpacked-union formals, open arrays in exports, conflicting signatures, a missing import symbol (with `neg_missing_symbol.c`) and an unprovided routine `svGetSelectBit` of the deprecated H.13 set (with `neg_unprovided_routine.c`) (A03). | 35.5.5, 35.5.6, 35.5.6.1; llg limits in docs/known_issues.md. |

## Other implementations

Some simulators lay out a structure member array with a descending range
left bound first (`native_aggregates`: `c nest i pair=10.5 20.5`), and the
whole of an open array likewise (`native_open_arrays`: `raw0=4` for `down`);
llg follows H.7.3. Some show an output real array uninitialized and an output
chandle array with the actual's handles; llg gives the type default
(S40-D1). A string pointer of an input that the C function stores in an
output leaf (`native_aggregates`: `io[1] = a[0]`) is copied by llg when the
function returns, while the pointer still holds the input's characters; some
simulators have reused that storage by then (H.8.10 lets the C function
assume nothing about its lifetime). Some refuse a query of dimension 0.

In `roundtrip_xz` some simulators pass a whole `int` actual by reference, so
`rt_result(r)` shows the actual (`c result o=5`) instead of the output's
starting value, and assign the string result before copying the output out
(`string s=output`). llg keeps copy-in/copy-out (decision S40-D2, whose row
quotes 35.6.1 and 13.5).
