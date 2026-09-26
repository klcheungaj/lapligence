# SYN-031 combinational UDP qualification

`sim_udp` runs `syn_031_combinational_udp.v` as Verilog-2001 and the matching
`.sv` source as SystemVerilog-2009, each with optimization enabled and disabled.
The paired files intentionally use the common syntax of both editions. Exact
expected stdout is in `tests/sim_udp.rs`.

IEEE 1364-2001 §§8.1.6, 8.2, 8.6 and IEEE 1800-2009 §§29.3.5, 29.4, 29.8
supply the oracle: `b` covers known 0/1, `?` covers every input value, a Z
input is treated as X, and a table with no matching row produces X. The mux
rows also specify a known result for an unknown select when both data inputs
are the same known value. The parity rows match built-in `xor` for all observed
inputs. The RTL mux reference normalizes data Z to X before its conditional;
this reflects UDP input semantics. The two-driver net reference uses a separate
built-in buffer, and the delayed reference uses a built-in delayed buffer.

The matrix exercises scalar instances, an instance array, input changes, a
second mux instance with reversed data ports, net resolution and a two-tick
primitive delay. `invalid_port_list.sv` and `invalid_table_width.sv` are
single-fault frontend negatives in both editions. Existing
`partial_features/udp_sequential_rejected.sv` and `udp_edge_rejected.sv` retain
the sequential and edge-table unsupported diagnostics.
