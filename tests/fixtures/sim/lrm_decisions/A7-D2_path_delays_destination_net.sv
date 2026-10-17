// Decision A7-D2: a module path delays the destination net itself, so every
// reader sees the delayed value: a process inside the module, a continuous
// assignment outside it and the testbench alike. When several drivers inside
// the module drive the destination, they resolve first and the path delays
// the resolved value.
//
// IEEE 1800-2009 30.3 (SystemVerilog-1800-2009.txt L49330-49332): "The paths
// described in the specify block, called module paths, pair a signal source
// with a signal destination." IEEE 1800-2009 30.4.1 (L49410-49413) only
// requires the destination to be "a net or variable that is connected to a
// module output port or inout port"; unlike IEEE 1364-2001 14.2.1 it does not
// require a single driver inside the module. The text does not say where the
// delay applies; llg delays the destination net.
`timescale 1ns/1ns
module pcell(input a, output y);
  assign y = a;
  specify
    (a => y) = 5;
  endspecify
  // A process inside the module copies what it sees of y.
  reg seen;
  always @(y) seen = y;
endmodule

module wired(input a, input b, output y);
  assign y = a ? 1'b1 : 1'bz;
  assign y = b ? 1'b0 : 1'bz;
  specify
    (a => y) = 2;
    (b => y) = 5;
  endspecify
endmodule

module tb;
  reg a, b;
  wire y, copy, w;
  pcell u(.a(a), .y(y));
  wired v(.a(a), .b(b), .y(w));
  assign copy = y;
  initial begin
    $monitor("%0t a=%b b=%b y=%b seen=%b copy=%b w=%b", $time, a, b, y, u.seen, copy, w);
    a = 1'b0; b = 1'b0;
    #10 a = 1'b1;
    #10 b = 1'b1;
    #10 a = 1'b0;
    #10 b = 1'b0;
    #10 $finish;
  end
endmodule
