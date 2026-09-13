# Physical-time simulator fixtures

These file-backed designs pin the femtosecond-based physical scheduler range:

- `mixed_femtoseconds.sv` schedules 1fs, 10fs, 100fs, 1ps, and 1ns events in
  one global design.
- `large_seconds.sv` distinguishes 10s and 100s units.
- `overflow.sv` rejects a checked physical-tick multiplication before model
  emission.
- `waveform_femtoseconds.sv` checks VCD timestamps and its 1fs header.

The owning suite runs positive fixtures through `llg` and `llg --no-opt`.
