# File I/O fixtures

These designs cover the Phase 03 H01 descriptor table, multichannel output,
typed radix formatting, portable file status/control tasks, and H02 character,
line, formatted, and binary input operations.

`file_input.sv` uses the IEEE 1364-2001 17.2.4.4 / IEEE 1800-2009
21.3.4.4 binary-read rule: memory addresses increase independently of the
declaration direction. For `[3:0]`, start 2/count 2 reads addresses 2 and 3;
the printed physical-declaration order is `34,12,xx,xx`. Its former
`xx,12,34,xx` oracle followed the implementation bug rather than that rule.
The public regression remains pending in this continuation; its native companion
is exercised by `runtime_value_storage/file_input_isolation_probe.c`.
