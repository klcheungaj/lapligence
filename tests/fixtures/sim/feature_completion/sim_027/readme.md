# SIM-027: memory-image views, memory-file diagnostics and the Q02 resolution

These public HDL fixtures cover `$readmemb`, `$readmemh`, `$writememb` and
`$writememh` (IEEE 1364-2001 17.2.8; IEEE 1800-2009 21.4-21.5) through
[`sim_feature_completion::sim_027`](../../../../sim_feature_completion/sim_027.rs).
Every test runs on the legacy, compact/portable and compact/GMP value backends
in both optimizer modes; `.v` sources also run in both editions. Each expected
value was derived by hand from the data and the clauses quoted below, not
copied from a simulator transcript. Line numbers refer to the `pdftotext
-layout` extractions `SystemVerilog-1800-2009.txt` (SV) and
`Verilog-1364-2001.txt` (V2001).

Run the suite with:

```sh
scripts/run-tests.sh -E 'test(/^sim_027::/)'
```

## Clauses the expectations rely on

Memory name and storage (SV 21.4, L37251-37255; 21.4.1, L37347):

> The memory_name can be an unpacked array or a partially indexed
> multidimensional unpacked array that resolves to a lesser dimensioned
> unpacked array (see 21.4.3). Higher order dimensions shall be specified with
> an index, rather than a complete or partial dimension range. The lowest
> dimension (i.e., the rightmost specified dimension in the identifier) can be
> specified with slice syntax.

> When loading dynamic arrays and queues, the current size of the array is
> fixed; the array shall not be resized.

Words (SV 21.4, L37273-37276; V2001 17.2.8, L19318-19321 says the same with
"Verilog HDL"):

> The numbers shall have neither the length nor the base format specified.
> For $readmemb, each number shall be binary. For $readmemh, the numbers shall
> be hexadecimal. The unknown value (x or X), the high-impedance value (z or Z),
> and the underscore ( _ ) can be used in specifying a number as in a
> SystemVerilog source description.

Extension of such a number (SV 5.7.1, L3921-3923, L3927-3929; V2001 3.5.1,
L1602-1604, L1637-1639):

> If the size of the unsigned number is smaller than the size specified for
> the literal constant, the unsigned number shall be padded to the left with
> zeros. If the leftmost bit in the unsigned number is an x or a z, then an x
> or a z shall be used to pad to the left, respectively.

> Unsized unsigned literal constants where the high-order bit is unknown (X or
> x) or three-state (Z or z) shall be extended to the size of the expression
> containing the literal constant.

Two-state words (SV 21.4.2, L37366-37368):

> For 2-state integer types, reading proceeds the same as for 4-state variable
> types (e.g., integer), with the exception that X or Z data are converted to 0.

Addressing (SV 21.4, L37280, L37300-37301, L37303-37308, L37310-37313,
L37315-37317, L37319-37323):

> As the file is read, each number encountered is assigned to a successive
> word element of the memory.

> When the system task encounters an address specification, it loads
> subsequent data starting at that memory address.

> If no addressing information is specified within the system task and no
> address specifications appear within the data file, then the default start
> address shall be the lowest address in the memory. ... If the start address
> is specified in the task without the finish address, then loading shall
> start at the specified start address and shall continue upward toward the
> highest address in the memory. In both cases, loading shall continue upward
> even after an address specification in the data file.

> If both start and finish addresses are specified as arguments to the task,
> then loading shall begin at the start address and shall continue toward the
> finish address. If the start address is greater than the finish address,
> then the address will be decremented between consecutive loads rather than
> being incremented. Loading shall continue to follow this direction even
> after an address specification in the data file.

> When addressing information is specified both in the system task and in the
> data file, the addresses in the data file shall be within the address range
> specified by the system task arguments; otherwise, an error message is
> issued, and the load operation is terminated.

> A warning shall be issued if the number of data words in the file differs
> from the number of words in the range implied by the start through finish
> addresses and no address specifications appear within the data file. In
> this case, the data words that are contained in the file shall be loaded
> into the memory beginning at the start address, and memory addresses for
> which the file does not contain data words are not modified by the
> operation.

Verilog-2001 defaults (V2001 17.2.8, L19338-19343, L19364-19365):

> If no addressing information is specified within the system task, and no
> address specifications appear within the data file, then the default start
> address shall be the left-hand address given in the declaration of the
> memory. ... If the start address is specified in the task without the finish
> address, then loading shall start at the specified start address and shall
> continue towards the right-hand address given in the declaration of the
> memory.

> A warning message shall be issued if the number of data words in the file
> differs from the number of words in the range implied by the start through
> finish addresses.

Views (SV 21.4.3, L37377-37378, L37427):

> The file contents are organized in row-major order, with each dimension's
> entries ranging from low to high address.

> Address entries in the file exclusively address the highest dimension's
> words.

Dumps (SV 21.5, L37462-37463; 21.5.3, L37497-37498):

> The $writememb and $writememh system tasks can be used to dump memory array
> (see 7.4.4) contents to files that are readable by $readmemb and $readmemh,
> respectively.

> When $writememb and $writememh write out data corresponding to unpacked or
> dynamic arrays, address specifiers (@-words) shall not be written to the
> output file.

Scheduling (SV 4.3, L3039-3043; 4.6, L3439-3440; 4.7, L3465-3471; V2001 5.3,
L5113-5118; 5.4.1, L5219-5222; 5.4.2, L5240-5246):

> Every change in state of a net or variable in the system description being
> simulated is considered an update event.

> Processes are sensitive to update events. When an update event is executed,
> all the processes that are sensitive to that event are considered for
> evaluation in an arbitrary order.

> Statements within a begin-end block shall be executed in the order in which
> they appear in that begin-end block.

> One source of nondeterminism is the fact that active events can be taken off
> the Active or Reactive event region and processed in any order. Another
> source of nondeterminism is that statements without time-control constructs
> in procedural blocks do not have to be executed as one event.

Slices of dynamic arrays and queues (SV 21.4, L37253-37254; 21.4.1, L37347
quoted above; 7.4.6, L8235-8237; decision S27-D14 rejects a slice outside the
current size):

> The lowest dimension (i.e., the rightmost specified dimension in the
> identifier) can be specified with slice syntax.

> If an index expression is out of the address bounds or if any bit in the
> address is X or Z, then the index shall be invalid. ... Writing to an array
> with an invalid index shall perform no operation.

Slices with a variable position and queue slice bounds (SV 7.4.6, L8226;
7.10, L8977-8978; 7.10.1, L9028-9029, L9034-9035; decisions S27-D19 and
S27-D20 evaluate each position or bound once and reject a slice outside the
array, with an unknown position or with reversed queue bounds):

> The size of the part-select or slice shall be constant, but the position
> can be variable.

> Each element in a queue is identified by an ordinal number that represents
> its position within the queue, with 0 representing the first, and $
> representing the last.

> In a queue slice expression such as Q[a:b], the slice bounds may be
> arbitrary integral expressions and, in particular, are not required to be
> constant expressions.

> Q[ a : b ] yields a queue with b - a + 1 elements.
> If a > b, then Q[a:b] yields the empty queue {}.

Elements of dynamic arrays and queues of unpacked arrays (SV 21.4,
L37251-37253; 21.4.1, L37347 quoted above; decision S27-D21 rejects an
element index outside the current size or with an X or Z bit):

> The memory_name can be an unpacked array or a partially indexed
> multidimensional unpacked array that resolves to a lesser dimensioned
> unpacked array (see 21.4.3). Higher order dimensions shall be specified
> with an index, rather than a complete or partial dimension range.

Class properties named through `this` or a handle (SV 8.10, L9926-9928; 8.4,
L9653-9654):

> The this keyword is used to unambiguously refer to class properties, value
> parameters, local value parameters or methods of the current instance.

> Accessing nonstatic members (see 8.8) or virtual methods (see 8.19) via a
> null object handle is illegal. The result of an illegal access via a null
> object is indeterminate, and implementations may issue an error.

Associative memories (SV 21.4.1, L37359-37362; 21.5.3, L37500-37502; 7.8.4,
L8696-8699; decisions S27-D15..D18):

> When loading associative arrays, indices shall be of integral types.
> Loading an address creates an element of that index, if it does not already
> exist. When an associative array's index is of an enumerated type, address
> entries in the pattern file are in numeric format and correspond to the
> numeric values associated with the elements of the enumerated type.

> When $writememb and $writememh write out data corresponding to associative
> arrays, address specifiers shall be written to the output file. As
> specified in 21.4.1, associative arrays shall have indices of integral types
> in order to be legal arguments to the $writememb and $writememh calls.

> The index expression shall be evaluated in terms of a cast to the index
> type, except that an implicit cast from a real or shortreal data type shall
> be illegal.

> The ordering is signed or unsigned numerical, depending on the signedness
> of the index type.

Evaluating memory_name (SV 21.4, L37251-37252; 11.4.1, L14478-14479, the
nearest rule for a target that is read and written; decision S27-D13):

> The memory_name can be an unpacked array or a partially indexed
> multidimensional unpacked array that resolves to a lesser dimensioned
> unpacked array (see 21.4.3).

> An assignment operator is semantically equivalent to a blocking assignment,
> with the exception that any left-hand index expression is only evaluated
> once.

## When loaded data become visible

A load stores each word when the task reaches it, as a blocking store in the
calling process. A word whose value changes is an update event: each process
waiting on that word (`@(mem[i])`, an `always` reading it, a `wait` on it) is
scheduled in the Active region of the same time slot and reads the loaded
value; a word loaded with its current value is no event, and cells outside the
range or view are not touched. The loading process sees every loaded word in
its next statement. `llg` runs the woken processes after the loading process
suspends; any order of them, and any interleaving with the loading process
that keeps its statement order, is permitted (4.7), so the tests accept every
such order. Memories without model array storage (subroutine and block
locals, `ref` formals, structure members) are packed images: a whole variable
or local, or a constant member of one, is loaded in place and published once
after its last word; any other root is loaded through a lexical copy stored
back with one blocking assignment. Because the loading process does not
suspend in between, the woken processes and the values they read are the
same. Dynamic arrays, queues and
fixed-size class properties are written in place and send one contents
notification. Decision S27-D1 records this.

## Fixtures

| Fixture | Covers | Expected result |
| --- | --- | --- |
| `contexts.sv` | Module, static task, automatic task, automatic function, block and fork-shared automatic memories, `ref` formals bound to module and automatic memories, a `const ref` dump, structure members, class properties (instance, slice, static), package and hierarchical memories, a constant and a runtime row of a local two-dimensional memory, a local slice, dynamic arrays and queues; every dump read back | `contexts.out`; seven "more words" warnings for the files longer than their view |
| `contexts_v2001.v` | Verilog-2001 module, static task, automatic task, function and hierarchical memories, ascending and descending | `contexts_v2001.2001.out` (declaration order) and `.2009.out` (lowest address first) |
| `round_trip.sv` | Write then read per radix: ascending, descending, negative and nonzero-based ranges, 100- and 200-bit words, two-state words, X/Z digits; explicit forward, reversed, mirrored and start-only ranges | `round_trip.out`: no mismatch; a hex digit with mixed known and unknown bits reads back as `x` |
| `file_format.sv` | Comments of both kinds, tabs, form feeds, underscores, digit case, X/Z, sparse `@` addresses (also backwards), a descending explicit range across addresses | `file_format.out`, no diagnostics (the file has addresses) |
| `invalid_data.v` | Missing file, malformed digit, file address outside the task range and outside the memory, unknown address, long and short files, start or finish outside the memory, the same in an automatic memory; guard memories on both sides | `invalid_data.out`; warnings for the missing file and word counts, errors for the rest; no guard or neighbouring cell changes |
| `q02_tokens.v` | Q02: short and long hex/binary words in 8-, 12-, 4-bit signed and 1-bit signed words | `q02_tokens.out` |
| `q02_two_state.sv` | Q02: X/Z digits in two-state words | `q02_two_state.out`; one conversion warning per file |
| `q02_views.sv` | Q02: the same words through a slice, a constant row and a runtime row in reverse | `q02_views.out` |
| `q02_bounds.v` | Q02: file address outside the task range; short and long files | `q02_bounds.out`; one error, two warnings |
| `wakeup.v` | A02: readers of loaded words, an unchanged word, a word outside the range and a later same-slot event | Lines `caller=11,22,33,00`, `reader0=11`, `reader0b=11`, `reader2=33`, `tick=1` each once in any order with `caller` before `tick`, then `done` |
| `wakeup_storage.sv` | A02 through a packed image: `ref` formal, structure member, static task memory | The four woken lines of `wakeup_storage.out` in any order, then its `done` line |
| `single_evaluation.sv` | A selector with side effects (a counting function) in memory_name: module and automatic structure-array members (load and dump), a runtime row of a local two-dimensional memory, a fork-shared row, a `ref` actual, a start bound | `single_evaluation.out`: every call counts one evaluation; one "more words" warning for the start-bound load |
| `mixed_state_struct.sv` | Memories that are members of a structure mixing two- and four-state members, in module, automatic and `inout` storage; X/Z digits loaded into both, each member dumped and read back | `mixed_state_struct.out`: X/Z kept in the four-state member, read as 0 in the two-state one (21.4.2); the scalar members keep their values; one conversion warning per two-state load |
| `resizable_slices.sv` | Constant slices (`[a:b]`, `+:`, `-:`) of a module queue and dynamic array, an automatic queue and a class-property dynamic array; explicit start and finish inside a slice; dumps of slices; a slice beyond the current size for a load and a dump | `resizable_slices.out`: only the selected elements change, sizes never change; the two slices beyond the size are errors that load nothing and write no file (S27-D14) |
| `variable_slices.sv` | Slices with a variable position (`+:`, `-:`) of ascending and descending module memories, with explicit start and finish, after a runtime row (a counting function), dumped and read back; a class property through `this.` and through a handle; a dynamic array; queue slices `[i:$]`, `[$-2:$]` and `[i-2:$-1]`; an automatic memory; slices outside the array, with an unknown position, outside the current size and with reversed queue bounds (also constant) | `variable_slices.out`: only the selected indices change, the row and slice functions run once, sizes never change; one "too few words" warning, then four load and two dump errors that change nothing and keep the dump file (S27-D19, S27-D20) |
| `container_rows.sv` | Dynamic arrays and queues whose elements are unpacked arrays: one element at a runtime index (a counting function) and at `$`, a variable slice inside an element with a start and finish, the whole array, constant and variable slices of its elements, two-dimensional elements, a class property through a handle; an element outside the current size or with an unknown index, a slice outside it, an invalid dump and an empty array | `container_rows.out`: only the selected cells change, the index function runs once, sizes never change; four "more words" warnings, three load errors and one dump error that change nothing (the dump file is kept), and the empty-array warning (S27-D21, S27-D19, S27-D10) |
| `null_handle.sv`, `null_handle_load.sv` | A dump of a variable slice and a load of a queue property through a null class handle | The null-handle error at the access, before any memory-file diagnostic; the run stops |
| `associative.sv` | Associative memories with `int`, `byte unsigned`, enum, 100-bit, `longint unsigned` (automatic) and `shortint` (class property) indices: default, start-only and ranged loads over existing entries, file addresses wider than or negative in the index type, a file address outside the range, binary and hexadecimal dumps (all, from a start, descending range, empty array) read back | `associative.out`: entries created or updated only at the loaded indices (S27-D15), addresses cast to the index type (S27-D16), each dumped word preceded by its `@` address in index order (S27-D17); one "more words" warning, one range error, one "too few words" warning |
| `neg_*` | Real, string and automatic real elements; a fully indexed word; a net memory; a `const ref` target; a wildcard-index associative memory (S27-D18); a variable-position slice wider than its dimension; an element of an associative array of unpacked arrays; `$writememh` and a two-dimensional memory in Verilog-2001 | Rejected before a model is built |

Data files: `format.hex`, `format.bin`, `tokens.hex`, `tokens.bin`,
`wake.hex`, `bad_digit.hex`, `outside_task.hex`, `outside_memory.hex`,
`unknown_address.hex`, `long.hex`, `short.hex`, `two_words.hex`,
`three_words.hex`, `far_address.hex`. `contexts.sv`, `contexts_v2001.v`,
`round_trip.sv`, `single_evaluation.sv`, `mixed_state_struct.sv`,
`resizable_slices.sv`, `associative.sv`, `variable_slices.sv` and
`container_rows.sv` write their own files.

## Diagnostics

The text prescribes a warning for a word count that differs from the range and
an error that terminates the load for a file address outside the task range
or an enum value out of range. `llg` prints them as
`llg: warning: memory file `<name>`: ...` and
`llg: error: memory file `<name>`: ...`. Neither stops the simulation or
changes its exit status. The remaining categories are `llg` decisions in the
[register](../../../../../docs/lrm_decisions.md#memory-files-sim-027): a
missing file is a warning (S27-D9); a malformed token (S27-D5), an address or
range outside the memory (S27-D7) and an invalid dump range (S27-D12) are
errors.
