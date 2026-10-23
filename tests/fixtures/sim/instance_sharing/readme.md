# Instance body sharing

`identities.sv` covers four module instances sharing a timing task and an event
worker, plus four generated processes. A top-level process triggers three distinct
events and disables only `u1.live` while it is waiting. The independent oracle
expects counts `1 0 1 1`, generated contribution slots resolving to `bits=1010`,
no late displays, and each original `%m` path. The
`sim_instance_sharing` suite checks exact stdout in both optimizer modes.
The expected generate-path spelling preserves the existing simulator output.

`netlist_cells.sv` is a netlist-style design: eight inverter cells read the bits
of a bus (`.a(din[i])`) and drive another bus through a specify path, and eight
flop cells built on a sequential UDP clock those bits through an edge path. Each
cell's port links, gates, path stamps and UDP gates differ per instance only in
their selected bits, nets, path sets and UDP states, which become record operands
of one shared body per cell port and process; the suite checks that the model
keeps one coroutine body per shared class instead of one per instance.

The independent oracle is the inverted input sampled by the flops, read 5 and 3
time units after each rising clock edge, after every path delay (at most 3) has
elapsed: `din = 10100110` gives `n = q = 01011001`, and `din = 00001111` gives
`11110000`. It relies on:

- IEEE 1800-2009 29.3.6, Table 29-1 (`SystemVerilog-1800-2009.txt` line 48842):
  "r             Same as (01)                                Rising edge on input."
- 29.6 (line 48971): "The following example describes a rising edge D flip-flop:"
- 30.5.1, Table 30-2 (lines 50028 and 50030): "0 -> 1           t             trise"
  and "1 -> 0           t             tfall" (two path delays are rise, fall).
- 20.4.2, Table 20-3 (line 34373): "units_number                 The smallest time
  precision argument of all the `timescale com-" (`%t` prints the 1 ps precision,
  so time 15 ns displays as `15000`).

The UDP table leaves clock transitions through x unspecified; the frontend's
warning about that is expected and those transitions never occur here.
