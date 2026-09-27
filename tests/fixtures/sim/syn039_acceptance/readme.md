# SYN-039 integrated acceptance fixtures

These fixtures are public CLI witnesses for the final selected profile. Each
positive design composes more than one completed task: fixed record arrays
through value ports, functions, combinational logic and a clocked process;
parameterized interfaces with generated memory consumers; true aliases with
multi-instance wired drivers; and the selected combinational UDP, tagged
pattern-case and library/configuration paths.

`unsupported_sequential_udp.sv` is a single-fault neighboring rejection. The
selected profile admits combinational UDPs; sequential or edge-sensitive UDP
forms remain outside the profile.
# SYN-039 selected-profile integration

All positive fixtures run through the public CLI with optimization enabled and
disabled. Runtime plusargs select data after model generation; the Rust suite
checks exact output and zero runtime stderr. The four compositions have separate
single-feature owners in the SYN-038 ledger.

- `array_record_datapath.sv` (SV2009 §§7.2, 7.4.2, 9.2.2.2, 13.4.2,
  23.2.2.2): for input tags 3/5 and values `4+s`/`6+s`, function `score`
  returns tag+value; `always_comb` returns values `11+2s`/`17+2s`, and
  `always_ff` captures total `18+2s`. The suite uses `s=1,2`.
- `interface_generate_memory.sv` (SV2009 §§25.3, 25.5, 27): bank `i`
  initializes row `j` to `0x10*(j+1)+i`, then adds `i` to the read result.
  `+address=0,1` selects different first reads; second reads select rows 3/2.
- `alias_wired_multi.sv` (SV2009 §§6.5–6.6, 10.3, 10.11): two instance
  drivers on one `wand` resolve Z, 0, 1 and 0 as enabled sites change.
  `+start=0,1` also checks the initial Z/0 choice through both alias names.
- `extended_top.sv` (SV2009 §§7.3.2, 12.6, 23.11, 29.3–29.4, 33):
  `+left=5a` chooses the tagged pattern payload and `+left=03` chooses
  the UDP parity fallback. A structural bind observes the result; a map
  configuration selects the `gate` library cell, which adds `0x20`.
  `unsupported_sequential_udp.sv` remains the neighboring selected-profile
  rejection control.

Verilog-2001 does not admit the SV-only compositions. Its retained Core,
library/configuration and non-ANSI combinational UDP evidence is in the
SYN-038 linked suites and their edition gates.
