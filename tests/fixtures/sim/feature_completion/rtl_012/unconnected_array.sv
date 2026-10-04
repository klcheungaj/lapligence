// SV2009 22.9,23.3
// Expected: 11 11 |
// Adopted FND-002 witness L-F11-05-01 (unconnected_array); `$finish(0)` keeps stderr empty.
`unconnected_drive pull1
module child(input wire [1:0] a[2]); initial begin #1; $display("%b %b",a[0],a[1]); end endmodule
module tb; child c(); initial #2 $finish(0); endmodule
`nounconnected_drive
