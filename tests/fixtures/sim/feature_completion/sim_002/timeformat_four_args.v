// V2001 17.3.2 Syntax17-10; SV2009 20.4.2 Syntax20-4
// Expected: 1.00 ns | 
`timescale 1ns/1ps
module tb; initial begin $timeformat(-9,2," ns",0); #1; $display("%t",$realtime); $finish(0); end endmodule
