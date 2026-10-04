// V2001 17.3.2 Syntax17-10; SV2009 20.4.2 Syntax20-4
// Expected: 1000 | 
`timescale 1ns/1ps
module tb; initial begin $timeformat; #1; $display("%0t",$realtime); $finish(0); end endmodule
