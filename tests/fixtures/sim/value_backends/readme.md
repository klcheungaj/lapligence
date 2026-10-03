# Experimental value backend witnesses

`implemented.sv` uses assignment, 129-bit multiplication, delay and decimal display;
its independent result is 17 × 19 = 323. The historically named `missing_shift.sv` exercises implemented S2 shift
(17 << 1 = 34); `missing_select.sv` exercises S4 selection (low 65 bits of 19 = 19).
These cover IEEE 1800 arithmetic, shift, packed selection, assignment and delay
semantics through checked-in public CLI sources in both optimizer modes.

Legacy and compact execute all three. The positive CLI parity test compares each
independent stdout with legacy, compact portable and optional compact GMP in
both optimizer modes. The historical `missing_*` filenames are retained.
`sim_value_backends/parity.rs` extends this matrix to existing arithmetic/width,
selection, streams, nets/strength, NBA/force, VPI/waveform, RTL-001/002/003 and
large-array fixtures. Component export/archive tests remain separate from HDL
execution evidence.
