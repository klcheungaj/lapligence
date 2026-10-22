# Q02 characterizations: cases the memory-file text leaves open

These plain HDL sources and data files record what `llg` does where IEEE
1364-2001 17.2.8 and IEEE 1800-2009 21.4-21.4.3 do not settle the result. Each
source has top module `tb`. No source asserts a conformance result: the
`.llg.out`, `.llg.err` and `.llg.status` files are exact current `llg` stdout,
stderr (without the lint report) and exit status, kept so that a change stays
visible and so that another simulator's output can be compared. A
characterization is not an IEEE undefined-behavior designation, and output
from another simulator is corroboration, not the normative oracle.

The Q02 cases that the text settles moved to conformance tests with
hand-derived expected values in
[`../feature_completion/sim_027/`](../feature_completion/sim_027/readme.md):
the extension and truncation of short and long words, two-state conversion,
selected views, file addresses outside the task range, short and long files,
and the order of processes woken by a load. Q03 (tagged-member NBA retagging)
is resolved from IEEE 1800-2009 4.9.4, 10.4.2, 7.3.2 and 11.9; its tests are in
[`../feature_completion/rtl_016/`](../feature_completion/rtl_016/).

## What stays open, and why

The words are numbers "as in a ... source description" (IEEE 1800-2009 21.4,
`SystemVerilog-1800-2009.txt` L37273-37276; IEEE 1364-2001 17.2.8,
`Verilog-1364-2001.txt` L19318-19321), which settles their extension. For enum
elements the text only says (IEEE 1800-2009 21.4.2, L37368-37370):

> For enumerated types, the file data represents the numeric values associated
> with each element of the enumerated type (see 6.19). If a numeric value is
> out of range for a given type, then an error shall be issued and no further
> reading shall take place.

It does not say whether "out of range" means outside the base type's width or
not a member value. `llg` follows industry-standard behaviour and reads it as
the width: an enum word is stored like a word of the base type, member or not,
and only a number with more digits than the base type needs is an error that
ends the load (decisions S27-D2 and S27-D4 in
[the register](../../../../docs/lrm_decisions.md)).

For a malformed token the text only lists what the file may contain (IEEE
1800-2009 21.4, L37268-37271; IEEE 1364-2001 17.2.8, L19311-19316):

> The text file to be read shall contain only the following:
> — White space (spaces, newlines, tabs, and formfeeds)
> — Comments (both types of comment are allowed)
> — Binary or hexadecimal numbers

It gives no recovery. `llg` ends the load with an error at the first malformed
token and keeps the words before it (decision S27-D5).

The wording of diagnostics, beyond the warning and error categories the text
prescribes, is `llg`'s own.

## Sources

Run from this directory so the named `.mem` files are in the simulator's
working directory. `.v` sources run in both Verilog-2001 and
SystemVerilog-2009 modes; `.sv` sources require SystemVerilog-2009. There are
no include paths, defines, plusargs or simulator-specific HDL tasks.

| Source | Probe | Data files | Editions |
| --- | --- | --- | --- |
| `q02_types.sv` | Hex X/Z words in two-state words and in a four-state enum with an `x` member | `q02_hex.mem` | 2009 |
| `q02_binary_types.sv` | Binary X/Z words in two-state words and in a four-state enum with an `x` member | `q02_binary.mem` | 2009 |
| `q02_enum_numeric.sv` | Signed enum: a word at the base width, a wider word and an x word | `q02_enum.mem` | 2009 |
| `q02_enum_tokens.sv` | Each short hex/binary X/Z token loaded separately into a four-state enum word | `q02_hex_x.mem`, `q02_hex_z.mem`, `q02_hex_1x.mem`, `q02_hex_x1.mem`, `q02_hex_zX.mem`, `q02_bin_x.mem`, `q02_bin_z.mem`, `q02_bin_1x.mem`, `q02_bin_x1.mem`, `q02_bin_zX.mem` | 2009 |
| `q02_malformed.v` | An illegal hex character in the second word | `q02_malformed.mem` | 2001, 2009 |

The two-state halves of `q02_types` and `q02_binary_types` are settled (IEEE
1800-2009 21.4.2, L37366-37368) and also covered by `sim_027/q02_two_state`.

## Exact `llg` invocations used for the goldens

`sim_undefined_behavior` runs the following commands from this directory with
the absolute path to the same public CLI binary, comparing stdout, stderr and
exit status byte for byte. All optimizer and edition variants currently agree
for each source, but each has its own golden so later divergence stays visible.

| Golden prefix | Exact invocation from this directory |
| --- | --- |
| `q02_binary_types.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_binary_types.sv` |
| `q02_binary_types.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_binary_types.sv` |
| `q02_enum_numeric.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_enum_numeric.sv` |
| `q02_enum_numeric.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_enum_numeric.sv` |
| `q02_enum_tokens.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_enum_tokens.sv` |
| `q02_enum_tokens.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_enum_tokens.sv` |
| `q02_malformed.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_malformed.v` |
| `q02_malformed.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_malformed.v` |
| `q02_malformed.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_malformed.v` |
| `q02_malformed.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_malformed.v` |
| `q02_types.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_types.sv` |
| `q02_types.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_types.sv` |

## Run on another simulator

Compile the selected source as top `tb` using the simulator's Verilog-2001 or
SystemVerilog-2009 switch as shown in the table. Launch the resulting model
with this directory as its working directory, so the `.mem` names resolve.
Capture stdout, stderr and exit status separately. For example, after saving
another simulator's stdout as `other.out`:

```sh
diff -u q02_types.2009.opt.llg.out other.out
```

Compare labelled `Q02.` lines as well as diagnostics. Other simulators may
format routine termination messages differently; retain their raw stderr for
review rather than rewriting either simulator's output.
