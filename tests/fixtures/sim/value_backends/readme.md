# Experimental value backend witnesses

`implemented.sv` uses assignment, 129-bit multiplication, delay and decimal display;
its independent result is 17 × 19 = 323. `missing_shift.sv` exercises S2 shift
(17 << 1 = 34); `missing_select.sv` exercises S4 selection (low 65 bits of 19 = 19).
These cover IEEE 1800 arithmetic, shift, packed selection, assignment and delay
semantics through checked-in public CLI sources in both optimizer modes.

Legacy executes all three. Compact archive/value clients can build, but the shared
scheduler currently refers to missing S2–S5 symbols even for `implemented.sv`.
The compact CLI test records that link rejection in portable and optional GMP
configurations; it is a dependency witness, not compact HDL execution acceptance.
Update the rejection test to positive parity when the operation families merge.
