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

`generate_bodies.sv` is a loop generate construct whose eight iterations each
hold a function, a task, an inertial continuous assignment to one bit of a
common net and an initial process. The iterations differ only in the genvar
value, in which driver of `y` they are and in their own inertial handle, so
the suite checks that each kind keeps one shared body (four coroutine bodies
with the top initial process, one multiplication) and that the eight handles
stay distinct file-scope objects.

The independent oracle: `a` is 0 until time 20, so every `y[i]` settles to 1 by
time 8. At time 20 `a` becomes `10100110`; bits 1, 2, 5 and 7 fall at
`20 + i + 1`, so at 24.5 only bits 1 and 2 have fallen (`y=11111001`) and at 40
`y` is `~a` (`01011001`). Each `hold(i)` copies `a[i]` at `21 + i`, after the
change (`q=10100110`), and `scale(i)` is `10 * i + i` (0, 33, 77). Distinct
handles matter: one shared handle would let each later schedule deschedule the
pending change of another bit. It relies on:

- IEEE 1800-2009 27.4 (`SystemVerilog-1800-2009.txt` line 46120): "Within the
  generate block of a loop generate construct, there is an implicit localparam
  declaration" whose "value within each instance of the generate block is the
  value of the index variable at the time the instance was elaborated."
- 10.3.3 (line 12894): "A delay given to a continuous assignment shall specify
  the time duration between a right-hand operand value change and the
  assignment made to the left-hand side."
- 10.3.3 (line 12920): "If this right-hand side value differs from the value
  currently scheduled to propagate to the left-hand side, then the currently
  scheduled propagation event is descheduled."

`sdf_gates.sv` with `sdf_gates.sdf` annotates eight AND2 cells on the bits of a
bus, each with its own DEVICE delay `i + 1`. The gates differ only in their
bits, nets and annotated delay slots, so the suite checks that they keep one
shared gate body. The independent oracle: `b` is 1 and `a` is 0 until time 20,
so every `y[i]` is 0 by time 8. At time 20 `a` becomes all ones and `y[i]` rises
at `20 + i + 1`: at 24.5 bits 0 to 3 have risen (`y=00001111`) and at 34.5 all
have (`y=11111111`). It relies on IEEE 1800-2009 32.4.3, Table 32-1
(`SystemVerilog-1800-2009.txt` line 52686): "(DEVICE... All specify paths to
module outputs. If no specify paths, all primitives driving module outputs."
