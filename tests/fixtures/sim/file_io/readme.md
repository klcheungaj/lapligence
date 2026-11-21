# File I/O fixtures

H01 covers descriptors, multichannel output, typed formatting and status/control;
H02 covers character, line, formatted and binary input.

`file_input.sv` follows IEEE 1364-2001 §17.2.4.4 / IEEE 1800-2009 §21.3.4.4:
binary memory reads advance numeric addresses regardless of declaration direction.
For `[3:0]` start 2/count 2, addresses 2 then 3 print in declaration order as
`34,12,xx,xx`. Its native companion is
`runtime_value_storage/file_input_isolation_probe.c`; native coverage is not HDL
execution.

`file_modes.sv` checks that `$fopen` passes the requested mode to the C library:
IEEE 1800-2009 §21.3.1 keeps `b` to distinguish binary from text files on hosts
that map line ends. Text files use the host's native newline (CRLF on Windows),
binary files are byte-exact everywhere.

`large_positions.sv` moves the position of a 4-byte binary file beyond 2 GiB
with relative seeks (32-bit offsets from the start, the current position and
the end) and prints `$ftell`; a seek before the start fails with -1. The
runtime keeps 64-bit positions on every host (`long`, which C `ftell`/`fseek`
use, is 32-bit on Windows), and llg's `$ftell` result is 64 bits wide. IEEE 1800-2009 §21.3.5
(`SystemVerilog-1800-2009.txt` lines 37134-37135 and 37145-37147):

> sets the position of the next input or output operation on the file specified by fd. The new position is at the
> signed distance offset bytes from the beginning, from the current position, or from the end of the file,

> $fseek() allows the file position indicator to be set beyond the end of the existing data in the file. If data
> are later written at this point, subsequent reads of data in the gap shall return zero until data are actually writ-
> ten into the gap. $fseek, by itself, does not extend the size of the file.
