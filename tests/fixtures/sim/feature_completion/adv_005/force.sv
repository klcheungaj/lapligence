// ADV-005 decision A5-D3: a forced switch node is a fixed source of its
// forced value at strong strength; what arrives through its switches does not
// change it, and release returns the network to its drivers (IEEE 1800-2009
// 10.6.2: "A force procedural statement on a net shall override all drivers
// of the net").
`timescale 1ns/1ns
module tb;
  wire a, b, c;
  reg ra;
  assign (pull0, pull1) a = ra;
  tran t1(a, b);
  tran t2(b, c);
  wire [1:0] v; wire e, f, g;
  reg [1:0] rv; reg en;
  assign v = rv;
  tran u1(v[1], e);
  tranif1 u2(e, f, en);
  tran u3(f, g);
  assign (pull0, pull1) g = 1'b0;
  initial begin
    ra = 0; rv = 2'b10; en = 1; #1
    $display("1 %v %v %v | %v %v %v %v %v", a, b, c, v[1], v[0], e, f, g);
    force b = 1; force f = 1'bx; #1
    $display("2 %v %v %v | %v %v %v %v %v", a, b, c, v[1], v[0], e, f, g);
    ra = 1; en = 0; #1
    $display("3 %v %v %v | %v %v %v %v %v", a, b, c, v[1], v[0], e, f, g);
    ra = 0; force v[1] = 0; en = 1; #1
    $display("4 %v %v %v | %v %v %v %v %v", a, b, c, v[1], v[0], e, f, g);
    release b; release f; #1
    $display("5 %v %v %v | %v %v %v %v %v", a, b, c, v[1], v[0], e, f, g);
    force a = 1'bz; release v; #1
    $display("6 %v %v %v | %v %v %v %v %v", a, b, c, v[1], v[0], e, f, g);
    release a; force e = 1'bz; #1
    $display("7 %v %v %v | %v %v %v %v %v", a, b, c, v[1], v[0], e, f, g);
    $display("bits %b%b%b %b%b%b%b", a, b, c, v, e, f, g);
    $finish;
  end
endmodule
