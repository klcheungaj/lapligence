// ADV-005: undelayed controls that are one bit of a signal (a 1-bit reg, a
// bit of a 100-bit reg above the first word, a net driven by a weak buf,
// one bit shared by four switches) are read directly from the signal;
// expression controls (c1 & c2, ~c1) run in a control process. Both follow
// x, z, force, release and a nonblocking update alike.
`timescale 1ns/1ns
module tb;
  // expression controls (process path) next to direct ones
  wire a1, b1, a2, b2, a3, b3, a4, b4, a5, b5;
  reg c1, c2;
  reg [99:0] wide;
  reg r1, r2, r3, r4, r5;
  assign (strong0, strong1) a1 = r1;
  assign (strong0, strong1) a2 = r2;
  assign (strong0, strong1) a3 = r3;
  assign (pull0, pull1) a4 = r4;
  assign (strong0, strong1) a5 = r5;
  tranif1 e1(a1, b1, c1 & c2);
  rtranif0 e2(a2, b2, ~c1);
  tranif1 w1(a3, b3, wide[70]);
  tranif0 w2(a4, b4, wide[70]);
  // one control bit drives several switches
  wire [3:0] s, d;
  assign s = 4'b1010;
  tranif1 m0(s[0], d[0], c2);
  tranif1 m1(s[1], d[1], c2);
  rtranif1 m2(s[2], d[2], c2);
  tranif0 m3(s[3], d[3], c2);
  // a control net driven through a gate with strength
  wire cn;
  buf (weak0, weak1) bc(cn, c1);
  tranif1 g1(a5, b5, cn);
  initial #2 c2 <= 1'bx;
  initial begin
    r1 = 1'b1; r2 = 1'b0; r3 = 1'b1; r4 = 1'b0; r5 = 1'b1;
    c1 = 1'b0; c2 = 1'b0; wide = 100'd0;
    #1 $display("1 %v %v %v %v %v | %v%v%v%v", b1, b2, b3, b4, b5, d[3], d[2], d[1], d[0]);
    c1 = 1'b1; c2 = 1'b1; wide[70] = 1'b1;
    #1 $display("2 %v %v %v %v %v | %v%v%v%v", b1, b2, b3, b4, b5, d[3], d[2], d[1], d[0]);
    c1 = 1'bx; wide[70] = 1'bz;
    #1 $display("3 %v %v %v %v %v | %v%v%v%v", b1, b2, b3, b4, b5, d[3], d[2], d[1], d[0]);
    force c2 = 1'b0; wide = {100{1'b1}};
    #1 $display("4 %v %v %v %v %v | %v%v%v%v", b1, b2, b3, b4, b5, d[3], d[2], d[1], d[0]);
    release c2; c2 = 1'b1; c1 = 1'b0; wide[69] = 1'b0; wide[71] = 1'b0;
    #1 $display("5 %v %v %v %v %v | %v%v%v%v", b1, b2, b3, b4, b5, d[3], d[2], d[1], d[0]);
    r3 = 1'b0; r4 = 1'b1; wide[70] = 1'b0;
    #1 $display("6 %v %v %v %v %v | %v%v%v%v", b1, b2, b3, b4, b5, d[3], d[2], d[1], d[0]);
    $finish;
  end
endmodule
