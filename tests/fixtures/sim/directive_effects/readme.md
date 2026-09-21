# SYN-017 directive effects

These exact fixtures exercise the public `llg` CLI with the `--edition 2001`
and `--edition 2009` profiles and with optimization enabled and disabled.
Positive cases compare stdout and runtime stderr; rejection cases require the
same diagnostic in both optimizer modes.

| Fixture | Coverage | Editions and controls | Expected result |
| --- | --- | --- | --- |
| `macros_include.sv` + `macro_header.svh` | `1364-2001 19.3.1, 19.4, 19.5`; `1800-2009 22.4–22.6`; guarded include, argument macro, token concatenation, stringification and all conditional branches | 2001/2009; `ENABLE`, `ALT`, and no command-line define | `branch=11/22/33 cat=a text=syn017` |
| `default_resetall.sv` | `1364-2001 19.2/19.6`; `1800-2009 22.3/22.8`; resetall restores implicit net admission | 2001/2009 | `implicit=z` |
| `default_nettype_none.sv` | `1364-2001 19.2`; `1800-2009 22.8`; implicit net is rejected | 2001/2009 | frontend rejection |
| `line_mapping.sv` | `1364-2001 19.7`; `1800-2009 22.12/22.13`; predefined file and line macros | 2001/2009 | `file=syn017_mapped.sv line=125` |
| `line_mapping_error.sv` | physical source range retained for an owned diagnostic after a line directive | 2001/2009 | rejection names physical `line_mapping_error.sv:7` |
| `unconnected_matrix.sv` | `1364-2001 19.9`; `1800-2009 22.9`; pull0, pull1 and nounconnected_drive on omitted packed inputs | 2001/2009 | `p0=0 p1=f pz=z` |
| `vectored_scalared.sv` | `1364-2001 3.3.2`; `1800-2009 6.9` | 2001/2009 | `v=a s=5`; declaration attributes are advisory and simulation-neutral |
| `later_macro.sv` | `1800-2009 22.5.1`; macro expansion into `always_comb` | 2009 positive; 2001 rejection | `x=1` in 2009; strict edition rejection in 2001 |
| `missing_include.sv` | `1364-2001 19.5`; `1800-2009 22.4` unavailable include | 2001/2009 | admission rejection |
| `unit_def.sv` + `unit_use.sv` | `1364-2001 19.3/19.4`; `1800-2009 22.5/22.6` source order | 2001/2009; separate and merged units | `unit_flag=0` separate, `unit_flag=1` merged |

`unconnected_drive` coverage is limited to omitted scalar/packed input storage.
Strength resolution against other drivers, aggregate or resizable formals, and
vendor synthesis interpretation are outside this witness. The frontend's
physical source ranges also remain the owned diagnostic identity after a
`line` directive; the executable predefined macros use the mapped file and
line values.
