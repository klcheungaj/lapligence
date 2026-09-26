# File I/O fixtures

H01 covers descriptors, multichannel output, typed formatting and status/control;
H02 covers character, line, formatted and binary input.

`file_input.sv` follows IEEE 1364-2001 §17.2.4.4 / IEEE 1800-2009 §21.3.4.4:
binary memory reads advance numeric addresses regardless of declaration direction.
For `[3:0]` start 2/count 2, addresses 2 then 3 print in declaration order as
`34,12,xx,xx`. Its native companion is
`runtime_value_storage/file_input_isolation_probe.c`; native coverage is not HDL
execution.
