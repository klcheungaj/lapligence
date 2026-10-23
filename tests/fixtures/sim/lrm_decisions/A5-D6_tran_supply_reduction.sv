// Decision A5-D6: tran, tranif0 and tranif1 pass every strength unchanged
// except supply, which becomes strong, whatever the other strength of the
// driver that produced it; rtran devices reduce by Table 28-8.
//
// IEEE 1800-2009 28.13 (SystemVerilog-1800-2009.txt L48297-48298): "The
// tran, tranif0, and tranif1 switches shall not affect signal strength across
// the bidirectional terminals, except that a supply strength shall be reduced
// to a strong strength." Some simulators pass a supply strength through a
// tran unchanged when only one of the driver's two strengths is supply; llg
// follows the clause text.
`timescale 1ns/1ns
module tb;
  reg r;
  wire a1, b1; assign (supply0, strong1) a1 = r; tran t1(a1, b1);
  wire a2, b2; assign (strong0, supply1) a2 = r; tran t2(a2, b2);
  wire a3, b3; assign (supply0, supply1) a3 = r; tran t3(a3, b3);
  wire a4, b4; reg e; assign (supply0, strong1) a4 = r; tranif1 t4(a4, b4, e);
  wire a5, b5; assign (supply0, strong1) a5 = r; rtran t5(a5, b5);
  initial begin
    r = 0; e = 1; #1
    $display("0 %v %v | %v %v | %v %v | %v %v | %v %v", a1, b1, a2, b2, a3, b3, a4, b4, a5, b5);
    r = 1; #1
    $display("1 %v %v | %v %v | %v %v | %v %v | %v %v", a1, b1, a2, b2, a3, b3, a4, b4, a5, b5);
    r = 1'bx; #1
    $display("x %v %v | %v %v | %v %v | %v %v | %v %v", a1, b1, a2, b2, a3, b3, a4, b4, a5, b5);
    $finish;
  end
endmodule
