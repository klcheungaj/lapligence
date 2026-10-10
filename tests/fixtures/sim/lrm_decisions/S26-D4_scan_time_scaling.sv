// Decision S26-D4: %t rounds the value read to the $timeformat precision in
// the $timeformat unit, then scales it into the time unit of the calling
// scope.
//
// IEEE 1800-2009 21.3.4.3 Table 21-8 (SystemVerilog-1800-2009.txt
// L36933-36938):
//   "The value matched is then scaled and rounded according to the current
//   timescale as set by $timeformat. For example, if the timescale is
//   `timescale 1ns/100ps and the time format is $timeformat(-3,2," ms",10);,
//   then a value read with $sscanf("10.345", "%t", t) would return
//   10350000.0."
//
// The example fixes the order: 10.345 ms rounds to 10.35 ms (precision 2),
// which is 10350000 ns. An integral destination receives the integer value.
`timescale 1ns / 100ps
module tb;
  integer c;
  realtime t;
  time ti;
  initial begin
    $timeformat(-3, 2, " ms", 10);
    c = $sscanf("10.345 2", "%t %t", t, ti);
    $display("c=%0d t=%0.1f ti=%0d", c, t, ti);
    $finish;
  end
endmodule
