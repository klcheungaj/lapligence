// The derived data event of `$period` needs a rising-only or falling-only
// reference event (31.4.5).
`timescale 1ns/1ns
module m(input c);
  reg n;
  specify
    $period(edge[01, 10] c, 10, n);
  endspecify
endmodule
module tb;
  reg c;
  m u(c);
  initial begin $display("ran"); #1 $finish; end
endmodule
