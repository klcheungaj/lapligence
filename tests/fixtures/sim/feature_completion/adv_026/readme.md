# ADV-026: `$sreadmemb` / `$sreadmemh`

These public HDL fixtures cover the string-sourced memory loads of IEEE
1800-2009 Annex D.14 and IEEE 1364-2001 Annex C.13 through
[`sim_feature_completion::adv_026`](../../../../sim_feature_completion/adv_026.rs).
Positive cases run on the legacy, compact/portable and compact/GMP value
backends in both optimizer modes; `.v` sources also run in both editions. Each
expected value was derived by hand from the data and the clauses quoted below,
not copied from a simulator transcript. Line numbers refer to the `pdftotext
-layout` extractions `SystemVerilog-1800-2009.txt` (SV) and
`Verilog-1364-2001.txt` (V2001).

Run the suite with:

```sh
scripts/run-tests.sh -E 'binary(sim_feature_completion) & test(adv_026::)'
```

## Clauses the expectations rely on

The task (SV D.14, L70152-70153, L70155, L70157-70160; V2001 C.13, L48048-48049,
L48051-48052, L48054-48056):

> $sreadmemb ( mem_name , start_address , finish_address , string { , string } ) ;
> $sreadmemh ( mem_name , start_address , finish_address , string { , string } ) ;

> The system tasks $sreadmemb and $sreadmemh load data into memory mem_name from a character string.

> The $sreadmemh and $sreadmemb system tasks take memory data values and addresses as string literal
> arguments. The start and finish addresses indicate the bounds for where the data from strings will be stored
> in the memory. These strings take the same format as the strings that appear in the input files passed as argu-
> ments to $readmemb and $readmemh.

V2001 C.13 says "as string arguments" where SV says "as string literal
arguments". Annex D is informative (SV D.1, L69791-69792: "The system tasks and
system functions described in this annex are for informative purposes only and
are not part of this standard."); this project implements it by its completion
scope. The argument order differs from `$readmemh(file, memory, start, finish)`:
the memory comes first and the start and finish addresses are required.

The file format the strings follow (SV 21.4, L37268-37276, L37280; V2001 17.2.8,
L19325):

> The text file to be read shall contain only the following:
> — White space (spaces, newlines, tabs, and formfeeds)
> — Comments (both types of comment are allowed)
> — Binary or hexadecimal numbers

> The numbers shall have neither the length nor the base format specified. For $readmemb, each number shall
> be binary. For $readmemh, the numbers shall be hexadecimal. The unknown value (x or X), the high-imped-
> ance value (z or Z), and the underscore ( _ ) can be used in specifying a number as in a SystemVerilog
> source description. White space and/or comments shall be used to separate the numbers.

> As the file is read, each number encountered is assigned to a successive word element of the memory.

Addresses and direction (SV 21.4, L37294-37301, L37310-37313, L37315-37317,
L37319-37323; V2001 17.2.8, L19329-19336, L19344-19350, L19364-19365):

> When the system task encounters an address specification, it loads subsequent data starting at that memory address.

> If both start and finish addresses are specified as arguments to the task, then loading shall begin at the start
> address and shall continue toward the finish address. If the start address is greater than the finish address,
> then the address will be decremented between consecutive loads rather than being incremented. Loading
> shall continue to follow this direction even after an address specification in the data file.

> When addressing information is specified both in the system task and in the data file, the addresses in the
> data file shall be within the address range specified by the system task arguments; otherwise, an error message
> is issued, and the load operation is terminated.

> A warning shall be issued if the number of data words in the file differs from the number of words in the
> range implied by the start through finish addresses and no address specifications appear within the data file.
> In this case, the data words that are contained in the file shall be loaded into the memory beginning at the
> start address, and memory addresses for which the file does not contain data words are not modified by the
> operation.

V2001 17.2.8 (L19364-19365) has no exception for address specifications:
"A warning message shall be issued if the number of data words in the file
differs from the number of words in the range implied by the start through
finish addresses." So a hole left by an `@` address is a warning in the
Verilog-2001 edition and none in the SystemVerilog-2009 edition, for strings as
for files (the project's edition rule).

Word extension and two-state words (SV 5.7.1, L3921-3924; 21.4.2, L37366-37370):

> If the size of the unsigned number is smaller than the size specified for the literal constant, the unsigned
> number shall be padded to the left with zeros. If the leftmost bit in the unsigned number is an x or a z, then
> an x or a z shall be used to pad to the left, respectively. If the size of the unsigned number is larger than the
> size specified for the literal constant, the unsigned number shall be truncated from the left.

> $readmemb and $readmemh support packed data of 2-state types, such as int or enumerated types. For 2-
> state integer types, reading proceeds the same as for 4-state variable types (e.g., integer), with the exception
> that X or Z data are converted to 0. For enumerated types, the file data represents the numeric values associ-
> ated with each element of the enumerated type (see 6.19). If a numeric value is out of range for a given type,
> then an error shall be issued and no further reading shall take place.

Storage (SV 21.4, L37251-37255; 21.4.1, L37347, L37359-37360):

> The memory_name can be an unpacked array or a partially indexed multidimensional unpacked array that
> resolves to a lesser dimensioned unpacked array (see 21.4.3). Higher order dimensions shall be specified
> with an index, rather than a complete or partial dimension range. The lowest dimension (i.e., the rightmost
> specified dimension in the identifier) can be specified with slice syntax. See 7.4.6 for details on legal array
> indexing in SystemVerilog.

> When loading dynamic arrays and queues, the current size of the array is fixed; the array shall not be resized.

> When loading associative arrays, indices shall be of integral types. Loading an address creates an element of
> that index, if it does not already exist.

Update events and ordering (SV 4.3, L3039-3040; 4.7, L3465-3466):

> Every change in state of a net or variable in the system description being simulated is considered an update
> event.

> One source of nondeterminism is the fact that active events can be taken off the Active or Reactive event
> region and processed in any order.

Built-in override (SV 36.3.2, L56427-56429):

> If a user-provided PLI application is associated with the same name as a built-in system task or system
> function (using the PLI mechanism), the user-provided C application shall override the built-in system task
> or system function, replacing its functionality with that of the user-provided C application.

## Decisions

The task text does not say how several strings combine, what happens at an
error, or whether a string variable is allowed. Each choice has a portable case
in `tests/fixtures/sim/lrm_decisions/` (`.sv` + `.out`), listed in
[`docs/lrm_decisions.md`](../../../../../docs/lrm_decisions.md) under "String-sourced
memory loads (ADV-026)". The numbers continue the S26 series, whose D1-D15 belong
to SIM-026.

| ID | Choice | Case |
| --- | --- | --- |
| S26-D16 | The strings of a call are one image: joined, in order, by one newline each. Words and `@` addresses continue across strings; a number never does ("1", "2" are two words). The equivalent file holds the strings separated by newlines. | `S26-D16_strings_form_one_image` |
| S26-D17 | A data argument may be any string expression or an integral value holding text, read when the task runs (SV D.14 L70157-70158 says "string literal arguments", but Annex D is informative; V2001 C.13 L48054-48055 says "string arguments"). | `S26-D17_string_variable_data` |
| S26-D18 | Loading runs from `start_address` toward `finish_address` as `$readmem*` does, descending when start is larger (D.14 does not say; 21.4 L37310-37313 and 17.2.8 L19344-19346 do for files). | `S26-D18_start_after_finish_descends` |
| S26-D19 | An address outside the start/finish range and a token that is not a number are errors that end the load; the words before them stay (D.14 states no error rules; 21.4 L37315-37317 does for files). | `S26-D19_errors_end_the_load` |
| S26-D20 | A `//` comment ends with its string; a `/*` comment still open at the end of a string continues in the next one (D.14 does not relate comments to string ends). | `S26-D20_comments_across_strings` |

## Fixtures and expected results

Diagnostics are `llg: warning: memory string ...` or `llg: error: memory string
...`, the categories of the file tasks. The label names the call as `$finish`
reports do: `` `$sreadmemh at tb:<line>:<column>` ``. The test checks that the
label points at a `$sreadmem*` call of that name.

### `equivalence.v` (A01, both editions)

Every memory is loaded twice, from a data file and from the strings holding the
same text, and printed (`file` line, then `str ` line); `mismatches` counts
`!==` differences over every cell. The image of each call is its strings joined
by one newline, which is what the checked-in data file holds. All expected
cells follow from the quoted rules:

- `A`, ascending `[0:7]`, `$sreadmemh(sa, 0, 7, "10 20 3x // line comment",
  "@5 zZ 6F_", "7a")`: 10 at 0, 20 at 1, 3x at 2 (x is a digit), `@5` moves to 5:
  `zZ` is the number zz, `6F_` is 6f (underscore), 7a at 7. Cells 3 and 4 were
  not written and are x. The line comment ends with its string, so `@5` is
  read. Cells: `10 20 3x xx xx zz 6f 7a`.
- `B`, descending `[7:0]`, start 7 finish 0, `"1 2 3", "4 5", "@2 a b c"`: 1 to
  7, 2 to 6, 3 to 5, 4 to 4, 5 to 3, `@2` then a at 2, b at 1, c at 0 (decrement
  continues after an address). Printed from 7 down: `01 02 03 04 05 0a 0b 0c`.
- `F`, start 5 finish 2 in an ascending memory, `"a b", "c d e"`: a at 5, b at 4,
  c at 3, d at 2; e is past the finish address, so it is dropped with the
  warning "contains more words than the selected range". Cells 0..7:
  `xx xx 0d 0c 0b 0a xx xx`.
- `G`, start 2 finish 5, `"@4 44 55", "@2 22 33"`: 44 at 4, 55 at 5, back to 2
  for 22 and 33 (both addresses are inside 2..5). `xx xx 22 33 44 55 xx xx`.
- `D`, 100-bit words, `"123456789abcdef0123456789 f", "@3
  x23456789abcdef012345678z"`: a 25-digit number is exactly 100 bits; `f` is
  zero-extended; cell 2 is unwritten (x); the number at 3 keeps its leftmost x
  and its trailing z.
- `E`, 200-bit words: a 50-digit number and `1` zero-extended to 50 digits.
- `H`, `$sreadmemb`, 6-bit words, `"000001 1x10_10", "@3 z"`: 000001; 1x1010
  (underscore ignored); cell 2 unwritten; `z` pads to zzzzzz (leftmost z).

Diagnostics: for each call, the string load reports exactly what the file load
reports. Verilog-2001 (17.2.8, L19364-19365) warns "too few words" for `A`
(6 of 8), `D` (3 of 4) and `H` (3 of 4) because their words do not fill the
range; SystemVerilog-2009 (21.4, L37319-37320) does not, since the strings
contain `@` addresses. Both editions warn "more words" for `F`.

### `equivalence_sv.sv` (A01, SystemVerilog types)

- `C`, two-state 12-bit binary: aaa, f0f; `x01` becomes 001 and `1z1` 101 (x/z
  read as 0, 21.4.2) with one conversion warning; 0000_0001_1111 is 01f; the
  sixth cell is unwritten (0) and five of six words warn "too few" (no `@`).
- `I`, signed `int`: `ffffffff` is -1; `7fffffff` is 2147483647; `123456789`
  has nine digits, one more than an `int` holds: an error that ends the load,
  so the last two cells keep 0 (S27-D2).
- `P`, packed-struct `{hi, lo}` nibbles: `a5`, `x1` (hi is x), `0f`.
- `N`, enum: 1, 2, 0 are GREEN, BLUE, RED.
- `M`, `logic [3:0] [2][3]`: six words fill the cells row-major: `1 2 3 4 5 6`.
- `V`, `$sreadmemh(sv[2:4], 2, 4, "e1", "e2 e3")`: the slice cells 2..4 take e1,
  e2, e3; the other cells stay x.

### `contexts.sv`, `contexts_v2001.v` (step 1)

`contexts.sv` is sim_027's `contexts.sv` with strings for files, so its lines are
the ones derived there for the same words (module, static and automatic task
locals, function local, `ref` formals, block local, fork-shared automatic, struct
member, class property and slice, static property, package, hierarchical memory,
row of a 2-D memory, dynamic array, queue); each call gets exactly the words of
its range, so no word-count warning is expected. With explicit start and finish
the load direction no longer depends on the edition (sim_027 shows the editions
differ only where `$readmemh` omits them), so `contexts_v2001.v` has one
expected output for both editions: `[5:2]` loaded 2 to 5 prints `13,12,11,10`
from 5 down; `[0:3]` loaded 3 to 0 prints `23,22,21,20`; `$sreadmemb` words `1`,
`10`, `11`, `100` are 1, 2, 3, 4 and with bias 100 sum to 110; `[3:0]` loaded 0
to 3 prints `13,12,11,10`.

### `joining.v` (S26-D16, S26-D20)

Memory cells start at 5a. `J1` `"1", "2", "3 4"` loads 01 02 03 04 (not 12). `J2`
(range 1..5) `"@4 aa", "bb", "@1 cc", "dd"`: aa at 4, bb at 5, cc at 1, dd at 2.
`J3` `"11 // 99", "22"`: the comment ends with its string, so 22 is a word. `J4`
`"11 /* 99", "88 */ 22"`: the open comment ends in the next string; 11 and 22.
`J5` empty strings add nothing. `J6` binary `"1", "10", "1_1"` is 1, 2, 3. `J7`
`"1 2 3", "@6", "4"` places 4 at 6. `J2` and `J7` leave holes after `@`, so
Verilog-2001 warns "too few" for them (17.2.8) and SystemVerilog-2009 does not.

### `diagnostics.v`, `diagnostics_sv.sv` (A02)

All cells start at 5a (`G` cells in the SV file). The test compares every
diagnostic and the printed cells.

| Tag | Call | Result |
| --- | --- | --- |
| D1 | `(mem, 0, 7, "11 22 1g 33")` | 11 22 stay; `1g` is not a number: error, load ends |
| D2 | `"44 /* open"` | 44 loaded; comment never closes: error |
| D3 | `(mem, 2, 4, "11 @6 22")` | 11 at 2; `@6` outside 2..4: error, load ends (L37315-37317) |
| D4 | `(mem, 0, 7, "11 @9 22")` | 11 at 0; `@9` outside the memory: error |
| D5 | `(mem, 0, 9, "11")` | finish outside the memory: error, nothing loaded |
| D6 | start is `4'bx` | unknown start: error, nothing loaded |
| D7 | start is -1 | outside the memory: error |
| D8 | `(mem, 0, 7, "11 22")` | 2 of 8 words: warning, 11 22 loaded |
| D9 | `(mem, 0, 1, "11 22 33 44")` | 4 of 2 words: warning, 11 22 loaded |
| D10 | `""` | no word: warning |
| D11 | only comments | no word: warning |
| D12 | `"11 @zz 22"` | `@zz` is not an address: error after 11 |
| D13 | `$sreadmemb(..., "1 2")` | `2` is not a binary digit: error after 01 |
| D14 | `(mem, 3, 0, "11 22 33 44 55")` | 11 at 3 ... 44 at 0; 55 past the finish: warning |
| E1 | enum `"1 2 3 10"` | 1 and 2 are members and 3, which fits the base, loads as a non-member (S27-D4); `10` has a digit more than the base holds: error, load ends (S27-D2) |
| S1 | slice `g[4:7]`, 6 words | cells 4..7 loaded, guards 3 and 8 unchanged: warning |
| S2 | slice `g[4:7]`, finish 9 | outside the memory: error, nothing loaded |
| S3 | slice, `@3` | `@3` outside the slice: error after 11 |
| S4 | slice `g[4:7]`, start 7 finish 4 | descending: cells 4..7 are 44 33 22 11 |
| R1 | empty dynamic array, `(0, 0)` | address 0 is outside the memory: error |
| R2 | 3-element dynamic array, finish 5 | outside its size: error, array keeps 7 8 9, size 3 |
| R3 | same array, `(0, 2)`, 6 words | 1 2 3; extra words warn; size stays 3 (L37347) |
| R4 | queue 7 8 9, start 2 finish 1, `"a b c"` | 7, b, a: 0xb at 1, 0xa at 2; c dropped with a warning |
| A1 | associative array, `(0, 2)`, 4 words | entries 0, 1, 2 created with 1 2 3 (L37359-37360); warning |
| W1 | two-state `"1x 2z 3"` | 10 20 03, cell 3 unchanged 77; conversion warning, then too few |
| W2 | 8-bit cell, `"123"` | a digit more than the cell holds: error, the cell stays x (S27-D2) |

### `notify.sv`, `notify.c` (step 3)

`mf` is loaded from `w1.hex` and `ms` from strings of the same words; both are
watched by one process per word, by continuous assignments `sum = m[1]` and `tot
= m[0] + m[2]`, and by VPI value-change callbacks on the assigned copies (the
callbacks report only while `armed` is 1). At time 1 all cells are preset to 00,
22, 00, 77 and `armed` is set. Expected events per time step (as sets, SV 4.7
L3465-3466):

- 2 (`$readmemh` 11 22 33 over cells 0..2): cells 0 and 2 change (11, 33); cell 1
  already holds 22 and cell 3 is outside the range, so there are no events for
  them; `tot` becomes 11 + 33 = 044 (nine bits, hex).
- 3: the same events for `ms`, loaded by `$sreadmemh(ms, 0, 2, "11", "22 33")`.
- 4 and 5: the same image again: no change, no event, no callback.
- 6 (`w2.hex` = 11 99 33): only cell 1 changes; `sum` becomes 99.
- 7: the same for `ms` with `"11 99", "33"`.
- 8: final cells `11 99 33 77` in both.

### String expression data (S26-D17)

- `string_expressions.sv` (`.out` hand-derived): a `localparam string` loads
  `11 22`; a variable plus a literal fills cells 2-3 (`33 44`); the variable
  changed to `"@1 55"` writes cell 1 only; the concatenation `{"7", "7"}` is the
  word `77`; a 48-bit vector holding `"88 99"` fills cells 2-3; a variable set
  by `$sformatf` in a loop loads `a0` then `a1`. No diagnostics.
- `string_expressions_v2001.v` (both editions): a parameter, a reg vector
  reassigned between loads and a concatenation of literals (`"@0 6"`, `"6"` is
  `@0 66`). Lines `a 11 22 33 44`, `b 66 22 33 55`; in Verilog-2001 the second
  load's `@` holes give the "too few words" warning.

### Negatives (A02)

| Fixture | Diagnostic (position) | Reason |
| --- | --- | --- |
| `neg_order.sv` | `:5:22 expression is not assignable` | the `$readmemh` order puts a string where the memory belongs (frontend) |
| `neg_count.sv` | `:4:11 too few arguments ... expected 4 but 3` | memory, start, finish and one string are required (frontend) |
| `neg_scalar.sv` | `:4:28 invalid argument type` | the first argument must be an array (frontend) |
| `neg_net_memory.sv` | `:4:22 cannot assign to a net` | a net array is not a variable memory (frontend) |
| `neg_real_memory.sv`, `neg_string_elements.sv`, `neg_wildcard_index.sv` | `:4:22`/`:5:22` located | no memory image for real or string elements, no index type for `[*]` (21.4.1) |
| `neg_slice_v2001.v`, `neg_multidimensional_v2001.v` | `:4:22` requires SystemVerilog-2009 | slices and multidimensional views are not Verilog-2001 (as for `$readmem*`) |

### VPI registrations (A03, S99-D6)

`sreadmem_override.c` registers `$sreadmemh` as a VPI task. A design that calls
it (`neg_sreadmem_override.sv`) stops at startup with "a VPI application
registers `$sreadmemh`, which llg implements as a built-in (called at <path>:7)",
as for every other built-in (36.3.2 L56427-56429 requires the override; llg
refuses it). A design that calls only `$sreadmemb`
(`sreadmem_unused_registration.sv`, printing `11 22`) is unaffected. Every other
fixture runs the tasks with no registration at all.
