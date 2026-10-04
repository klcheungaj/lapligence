// V2001 17.3.2 Syntax17-10 Table76; SV2009 20.4.2 Syntax20-4 Table20-3
// Expected: 1000 | 
`timescale 1ns/1ps
module tb; initial begin $timeformat(-9,2," ns",0); $timeformat; #1; $display("%0t",$realtime); $finish(0); end endmodule
