// Decision A10-D1: with non-negative limits $setuphold(r, d, S, H) is
// $setup(d, r, S) plus $hold(r, d, H), and $recrem(r, d, REC, REM) is
// $removal(r, d, REM) plus $recovery(r, d, REC): each violation is reported
// once, as the component check, at the later of the two events.
//
// IEEE 1800-2009 31.3.3 (SystemVerilog-1800-2009.txt L51119): "is equivalent
// in functionality to the following, if tSU and tHLD are not negative";
// 31.3.6 L51336-51339: "is equivalent in functionality to the following, if
// tREC and tREM are not negative: $removal( posedge clear, posedge clk, tREM
// ); $recovery( posedge clear, posedge clk, tREC );".
`timescale 1ns/1ns
module sh(input r, input d); reg n; specify $setuphold(posedge r, d, 10, 5, n); endspecify endmodule
module rr(input r, input d); reg n; specify $recrem(posedge r, posedge d, 10, 5, n); endspecify endmodule
module tb;
  reg [5:0] r, d;
  sh s0(r[0], d[0]); sh s1(r[1], d[1]); sh s2(r[4], d[4]);
  rr q0(r[2], d[2]); rr q1(r[3], d[3]); rr q2(r[5], d[5]);
  initial $monitor("%0t n=%b%b%b %b%b%b", $time, q2.n, q1.n, q0.n, s2.n, s1.n, s0.n);
  initial begin
    r = 0; d = 0;
    #90 d[4] = 1;      // 90: on the start of the setup window (90, 100): none
    #1 d[0] = 1;       // 91: setup of s0, reported at 100
    #4 d[5] = 1;       // 95: on the start of the removal window (95, 100): none
    #1 d[2] = 1;       // 96: removal of q0, reported at 100
    #4 r = 6'b111111;  // 100
    d[1] = 1;          // 100, after the reference: hold of s1
    #2 d[3] = 1;       // 102: recovery of q1
    #3 d[4] = 0;       // 105: the end of the hold window [100, 105): none
    #5 d[5] = 0;       // 110: a falling data edge is no $recrem event
    #10 $finish;
  end
endmodule
