# Q02 oracle characterizations

These plain HDL sources and data files are for cross-simulator comparison. Each source has top module `tb`. No source asserts a result for Q02. The `.llg.out`, `.llg.err` and `.llg.status` files record exact current `llg` stdout, stderr and exit status. They are **characterizations, not conformance oracles**. An unresolved oracle is not itself an IEEE undefined-behavior designation. The memory-load references are IEEE 1364-2001 §17.2.8 and IEEE 1800-2009 §§21.4–21.4.3. The prescribed warning/error categories and settled behavior described in `docs/sim_features.md` remain separate.

For Q02 wakeup traces, IEEE 1364-2001 §§5.4–5.5 and IEEE 1800-2009 §§4.6–4.7 allow independent Active-region readers and the loading process to interleave. Test that each reader follows its own triggering word update; do not require one total print order. Short-token/type/enum intersections remain unresolved separately. Cross-simulator output is corroboration, never the normative oracle. Q03 (tagged-member NBA retagging) is resolved from IEEE 1800-2009 §§4.9.4, 10.4.2, 7.3.2 and 11.9; its clause-derived conformance tests are in `../feature_completion/rtl_016/`.

Run from this directory so the named `.mem` files are in the simulator's working directory. `.v` sources run in both Verilog-2001 and SystemVerilog-2009 modes; `.sv` sources require SystemVerilog-2009. All run with top `tb`. There are no include paths, defines, plusargs or simulator-specific HDL tasks.

| Source | Probe | Data files | Editions |
| --- | --- | --- | --- |
| `q02_short_hex.v` | Short `x`, `z`, `1x`, `x1`, `zX` hex tokens in wider 8-bit words | `q02_hex.mem` | 2001, 2009 |
| `q02_short_binary.v` | Same binary tokens in wider 8-bit words | `q02_binary.mem` | 2001, 2009 |
| `q02_binary_narrow.v` | Binary token truncation into signed 1-bit words | `q02_binary.mem` | 2001, 2009 |
| `q02_narrow_signed.v` | Hex token truncation into signed 4-bit words | `q02_hex.mem` | 2001, 2009 |
| `q02_types.sv` | Hex X/Z conversion into two-state and four-state enum words | `q02_hex.mem` | 2009 |
| `q02_binary_types.sv` | Binary X/Z conversion into two-state and four-state enum words | `q02_binary.mem` | 2009 |
| `q02_enum_numeric.sv` | Signed enum pre-truncation numeric and X policy | `q02_enum.mem` | 2009 |
| `q02_enum_tokens.sv` | Each short hex/binary X/Z token loaded separately into a four-state enum word | `q02_hex_x.mem`, `q02_hex_z.mem`, `q02_hex_1x.mem`, `q02_hex_x1.mem`, `q02_hex_zX.mem`, `q02_bin_x.mem`, `q02_bin_z.mem`, `q02_bin_1x.mem`, `q02_bin_x1.mem`, `q02_bin_zX.mem` | 2009 |
| `q02_views.sv` | Short tokens through a slice, fixed selected row and runtime-selected row | `q02_hex.mem` | 2009 |
| `q02_malformed.v` | Illegal hex character in a data token and retained words | `q02_malformed.mem` | 2001, 2009 |
| `q02_bad_address.v` | Out-of-range `@` jump after a valid word | `q02_bad_address.mem` | 2001, 2009 |
| `q02_short_file.v` | Too few words, unchanged trailing cells | `q02_short_file.mem` | 2001, 2009 |
| `q02_long_file.v` | Too many words, bounded destination | `q02_long_file.mem` | 2001, 2009 |
| `q02_wakeup.v` | Several readers of same/different words loaded in one slot, plus a separate same-slot event | `q02_wakeup.mem` | 2001, 2009 |

## Exact `llg` invocations used for the goldens

`sim_undefined_behavior` runs the following commands from this directory with the absolute path to the same public CLI binary, comparing stdout, stderr and exit status byte for byte. A golden prefix identifies its `.out`, `.err` and `.status` files. All recorded optimizer and applicable edition variants currently agree for each source, but each has its own golden so later divergence remains visible.

### Per-golden command list

The following are the concrete invocations used to capture each golden.

| Golden prefix | Exact invocation from this directory |
| --- | --- |
| `q02_bad_address.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_bad_address.v` |
| `q02_bad_address.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_bad_address.v` |
| `q02_bad_address.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_bad_address.v` |
| `q02_bad_address.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_bad_address.v` |
| `q02_binary_narrow.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_binary_narrow.v` |
| `q02_binary_narrow.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_binary_narrow.v` |
| `q02_binary_narrow.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_binary_narrow.v` |
| `q02_binary_narrow.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_binary_narrow.v` |
| `q02_binary_types.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_binary_types.sv` |
| `q02_binary_types.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_binary_types.sv` |
| `q02_enum_numeric.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_enum_numeric.sv` |
| `q02_enum_numeric.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_enum_numeric.sv` |
| `q02_enum_tokens.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_enum_tokens.sv` |
| `q02_enum_tokens.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_enum_tokens.sv` |
| `q02_long_file.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_long_file.v` |
| `q02_long_file.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_long_file.v` |
| `q02_long_file.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_long_file.v` |
| `q02_long_file.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_long_file.v` |
| `q02_malformed.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_malformed.v` |
| `q02_malformed.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_malformed.v` |
| `q02_malformed.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_malformed.v` |
| `q02_malformed.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_malformed.v` |
| `q02_narrow_signed.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_narrow_signed.v` |
| `q02_narrow_signed.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_narrow_signed.v` |
| `q02_narrow_signed.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_narrow_signed.v` |
| `q02_narrow_signed.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_narrow_signed.v` |
| `q02_short_binary.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_short_binary.v` |
| `q02_short_binary.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_short_binary.v` |
| `q02_short_binary.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_short_binary.v` |
| `q02_short_binary.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_short_binary.v` |
| `q02_short_file.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_short_file.v` |
| `q02_short_file.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_short_file.v` |
| `q02_short_file.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_short_file.v` |
| `q02_short_file.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_short_file.v` |
| `q02_short_hex.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_short_hex.v` |
| `q02_short_hex.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_short_hex.v` |
| `q02_short_hex.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_short_hex.v` |
| `q02_short_hex.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_short_hex.v` |
| `q02_types.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_types.sv` |
| `q02_types.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_types.sv` |
| `q02_views.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_views.sv` |
| `q02_views.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_views.sv` |
| `q02_wakeup.2001.opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 q02_wakeup.v` |
| `q02_wakeup.2001.no-opt.llg` | `../../../../target/debug/llg --top tb --edition v2001 --no-opt q02_wakeup.v` |
| `q02_wakeup.2009.opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 q02_wakeup.v` |
| `q02_wakeup.2009.no-opt.llg` | `../../../../target/debug/llg --top tb --edition sv2009 --no-opt q02_wakeup.v` |

## Run on another simulator

Compile the selected source as top `tb` using the simulator's Verilog-2001 or SystemVerilog-2009 switch as shown in the table. Launch the resulting model with this directory as its working directory, so the `.mem` names resolve. Capture stdout, stderr and exit status separately. For example, after saving another simulator's stdout as `other.out`:

```sh
diff -u q02_short_hex.2001.opt.llg.out other.out
```

Compare labelled `Q02.` lines as well as diagnostics. Other simulators may format routine termination messages differently; retain their raw stderr for review rather than rewriting either simulator's output.
