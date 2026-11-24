# FIN-002 adversarial inputs (step 3)

The bounded corpora of FIN-002 step 3 generate their inputs from one fixed
seed ([`tests/support/fuzz.rs`](../../../../support/fuzz.rs)); the checked-in
files of this directory are the model they run and the inputs they minimized,
run by [`tests/sim_feature_completion/fin_002b.rs`](../../../../sim_feature_completion/fin_002b.rs).
The composed scenarios of steps 1 and 2 are in [`fin_002`](../fin_002/readme.md).
Clause line numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt`, which is not part of the repository.
Suites and commands: [tests readme](../../../../readme.md#adversarial-corpora).

### `fuzz_inputs.sv`

Run by `runtime_readers_end_normally_on_adversarial_input`.
One file `in.mem` and the plusargs of the run go through `$readmemh`,
`$readmemb` (whole memory, address range, descending and 100-bit memories),
`$fscanf`, `$fgets`, `$fgetc`, `$fread`, `$ungetc`, `$sscanf` and
`$value$plusargs`. The suite builds the model once per optimizer mode and
runs it once per generated input. Text from the file is printed as lengths
and sums only, so the transcript is ASCII whatever the input holds. There is
no `.out`: the suite checks every run for exit status 0, the last line
`done`, catalogued messages only, and for well-formed images the words the
load leaves, computed while the image is generated. The two plusarg values
it checks rest on 21.6, L37578-37579: "The first string from the list of
plusargs provided to the simulator, which matches the plusarg_string portion
of the user_string specified shall be the plusarg string available for
conversion."

### `fuzz_expr_cast_fill.sv`

Run by `unsized_literals_take_the_size_of_a_cast`;
minimized from two differences the expression corpus found (variables case
11, parameters case 1). 6.24.1, L7328-7329: "When changing the size, the
cast shall return the value that a packed array type with a single [n-1:0]
dimension would hold after being assigned the expression, where n is the cast
size." 5.7.1, L3944-3946: "In a self-determined context, an unsized
single-bit value shall have a width of 1 bit, and the value shall be treated
as unsigned." The operand of a size cast is therefore in an assignment
context of the cast's size, and an unbased unsized literal in it has that
size also below an operator that takes its size from its operands. The
expected output is the industry-standard behaviour.

| Lines | Why |
| --- | --- |
| `constant ffffffff 7fffffff f8 ff` | The same casts as parameter values. |
| `not`, `shr`, `shr x`, `shl`, `cond`, `narrow`, `minus`, `sum`, `or`, `display` | `~'0`, `'1 >> 1`, `'x >> 1`, `'1 << 3`, `1 ? '1 : '0`, `-'1` evaluated at the cast's size at run time (each was one bit, then extended). |
| `power xxxx...`, `actual xxxxxxxx`, `base 01` | The left operand of `**` has the size of the context: an unknown base gives an unknown result of that size (11.4.2, Table 11-4), and `8'hff ** 2` is `01`. |
| `sized`, `assign` | Sized operands and the assignment form, which were right before. |
