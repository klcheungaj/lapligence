// Decision A7-D4: two declarations of the same module path, or of paths that
// share a source bit and a destination bit (here `a => y` and
// `a[0] *> y[0]`), are legal. Each declaration is a path of its own; when
// their common input changes, all of them are active and the smallest delay
// for the transition being scheduled applies, separately for rise and fall.
// The more specific path does not override the general one.
//
// IEEE 1800-2009 30.5.3 (SystemVerilog-1800-2009.txt L50166-50173):
// "Active specify paths are those whose input has transitioned most recently
// in time, and either they have no condition or their conditions are true.
// In the presence of simultaneous input transitions, it is possible for many
// specify paths to an output to be simultaneously active." "Once the active
// specify paths are identified, a delay shall be selected from among them.
// This is done by comparing the correct delay for the specific transition
// being scheduled from each specify path and choosing the smallest."
// (Example 2, L50184-50196, declares `A => Y` five times.)
`timescale 1ns/1ns
module general_small(input [1:0] a, output [1:0] y);
  assign y = a;
  specify
    (a => y) = 1;
    (a[0] *> y[0]) = 3;
  endspecify
endmodule

module specific_small(input [1:0] a, output [1:0] y);
  assign y = a;
  specify
    (a => y) = 3;
    (a[0] *> y[0]) = 1;
  endspecify
endmodule

module twice(input a, output y);
  assign y = a;
  specify
    (a => y) = (1, 4);
    (a => y) = (3, 2);
  endspecify
endmodule

module tb;
  reg [1:0] a;
  wire [1:0] g, s;
  wire t;
  general_small u1(.a(a), .y(g));
  specific_small u2(.a(a), .y(s));
  twice u3(.a(a[0]), .y(t));
  initial begin
    $monitor("%0t g=%b s=%b t=%b", $time, g, s, t);
    a = 2'b00;
    #10 a = 2'b11;
    #10 a = 2'b00;
    #10 $finish;
  end
endmodule
