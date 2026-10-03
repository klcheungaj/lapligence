// V2001 6.2.1, 10.3.5, 12.4.1; SV2009 6.8, 6.21, 10.5: a Verilog-2001
// declaration assignment is an initial assignment that may race with other
// processes, so an `always` armed first can observe it (allowed results
// "1 0 6" and "1 1 6"). SystemVerilog initializes static variables before any
// process starts and the initialization is not a change event ("1 0 6" only).
// A constant-function call keeps its value in both editions.
module tb;
  function [7:0] double;
    input [7:0] v;
    double = v * 2;
  endfunction
  reg r = 1'b1;
  reg seen = 1'b0;
  reg [7:0] k = double(8'd3);
  always @(r) seen = 1'b1;
  initial begin
    #1 $display("%b %b %0d", r, seen, k);
    $finish(0);
  end
endmodule
