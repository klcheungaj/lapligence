// V2001 17.3.1; SV2009 20.4.1
// Expected: Time scale of (tb.c) is 1ns / 1ps | 
`timescale 1ns/1ps
module child; endmodule
`timescale 1us/1ns
module tb; child c(); initial begin $printtimescale(c); $finish(0); end endmodule
