// Plain selected net ports, whose copy would show the same values one update
// later, merge with their formal bit by bit too (decision A21-D5): one-run
// formals (a bit and a part select), a reversed concatenation, an output bit
// beside a parent driver, strengths, a force of the actual. Each time step
// has one printing process.
`timescale 1ns/1ns
module child(input wire a, input wire [1:0] p, input wire [1:0] q, output wire o);
  reg d = 1'b0;
  assign o = d;
  integer wakes = 0;
  always @(a) if ($time > 0) wakes = wakes + 1;
endmodule

module tb;
  reg [3:0] r = 4'b0000;
  wire [3:0] v;
  wire [3:0] w;
  assign v = r;
  assign w[0] = 1'b1;
  child u(.a(v[1]), .p(v[3:2]), .q({v[0], v[3]}), .o(w[2]));
  always @(v) if ($time > 0) $display("%0t v=%b u.a=%b u.p=%b u.q=%b", $time, v, u.a, u.p, u.q);
  always @(w) if ($time > 0) $display("%0t w=%b u.o=%b", $time, w, u.o);
  initial begin
    #1 r = 4'b0110;
    #1 $display("strength: %v %v %v %v", u.a, v[1], u.o, w[2]);
    #1 u.d = 1'b1;
    #1 force v = 4'b1001;
    #1 release v;
    #1 r = 4'b1000;
    #1 $display("wakes=%0d v=%b u.a=%b u.p=%b u.q=%b w=%b u.o=%b", u.wakes, v, u.a, u.p, u.q, w, u.o);
    $finish;
  end
endmodule
