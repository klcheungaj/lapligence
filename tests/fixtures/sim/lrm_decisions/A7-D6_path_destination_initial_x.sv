// Decision A7-D6: a module path destination with a driver starts at x, the
// output of its driver before the first evaluation, until its first
// path-delayed transition matures, like a net with a net delay (S2-D10); the
// first transition is from x, so it takes the x->0 or x->1 delay. This is the
// industry-standard behaviour; llg previously started the destination at z.
//
// IEEE 1800-2009 6.7 (SystemVerilog-1800-2009.txt L5199-5200): "The default
// initialization value for a net shall be the value z. Nets with drivers
// shall assume the output value of their drivers." The text does not say
// what a path destination holds before its first delayed transition.
`timescale 1ns/1ns
module pcell(input a, output y);
  assign y = a;
  specify
    (a => y) = (1, 2, 3, 4, 5, 6);
  endspecify
endmodule

module tb;
  reg a, b;
  wire y0, y1;
  pcell u0(.a(a), .y(y0));
  pcell u1(.a(b), .y(y1));
  initial begin
    $monitor("%0t y0=%b y1=%b", $time, y0, y1);
    a = 1'b0;
    b = 1'b1;
    #10 $finish;
  end
endmodule
