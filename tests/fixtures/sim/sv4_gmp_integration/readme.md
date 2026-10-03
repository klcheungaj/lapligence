# Packed-owner integration across backends

Each fixture checks retained packed owners against literals computed outside
the simulator (Python integers or hand-derived X/Z layouts) and prints
`PASS <fixture>` only when every check holds. `sim_value_backends/parity.rs`
runs them through the public CLI with legacy, compact portable and (given
`LLG_TEST_GMP_ROOT`) compact GMP in both optimizer modes; stdout and stderr
must equal legacy and the expected line. Values cross the 64-bit inline/wide
boundary and the known/X-Z boundary, where the compact backend moves payloads.

| Fixture | Retained owners exercised |
| --- | --- |
| `scheduler_snapshots.sv` | Delayed whole and selected NBAs whose source then becomes X/Z or known; force/release of a wide net; posedge NBA capture before a same-slot source write; self-assignment with overlapping up/down slices and a rotation. |
| `container_owners.sv` | Queue read snapshots, a pending `ref` write that follows its element across `push_front`, pop/delete; dynamic-array resize copy and X default; associative read after delete; mailbox payloads that survive mutation of the value that was put. |
| `activation_owners.sv` | Recursive functions with two wide locals per activation (chain arena) and a 200-bit factorial; add/mul/sub destinations aliasing their operands; wide-snapshot waits canceled by `disable fork`, then a fresh wait woken by an X-to-1 bit change. |
| `sampled_owners.sv` | A clocking output drive captured before its source becomes X; inertial continuous assignment rejecting narrow wide pulses; a sequence local holding a wide value across later X writes. |
