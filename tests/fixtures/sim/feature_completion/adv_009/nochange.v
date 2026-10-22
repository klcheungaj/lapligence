// $nochange windows (lead - start offset, trail + end offset), end points
// excluded, data in the same step as an edge, reporting times (IEEE
// 1800-2009 31.4.6, IEEE 1364-2001 15.3.6; see readme.md).
`timescale 1ns/1ns
module nc(input c, input d); reg n; specify $nochange(posedge c, d, 0, 0, n); endspecify endmodule
module nx(input c, input d); reg n; specify $nochange(posedge c, d, 2, 3, n); endspecify endmodule
module tb;
  reg c;
  reg [5:0] qd;
  reg [3:0] xd;
  nc q0(c, qd[0]); nc q1(c, qd[1]); nc q2(c, qd[2]); nc q3(c, qd[3]); nc q4(c, qd[4]); nc q5(c, qd[5]);
  nx x0(c, xd[0]); nx x1(c, xd[1]); nx x2(c, xd[2]); nx x3(c, xd[3]);
  always @(q1.n or q2.n or q3.n or q4.n or q5.n or q0.n or x0.n or x1.n or x2.n or x3.n)
    if ($time > 0) $display("%0t toggled", $time);
  initial
    $monitor("%0t nochange00=%b%b%b%b%b%b nochange23=%b%b%b%b", $time,
             q5.n, q4.n, q3.n, q2.n, q1.n, q0.n, x3.n, x2.n, x1.n, x0.n);
  initial begin
    c = 0; qd = 0; xd = 0;
    #98 xd[0] = 1;                   // 98: start offset 2 exactly
    #1 xd[1] = 1; qd[5] = 1;         // 99: inside (98, ...); before the zero window
    #1 qd[0] = 1; c = 1; qd[1] = 1;  // 100: leading edge, data in the same step
    #1 qd[2] = 1;                    // 101: inside
    #18 qd[3] = 1; c = 0; qd[4] = 1; // 119: trailing edge, data in the same step
    #2 xd[2] = 1;                    // 121: inside (..., 119 + 3)
    #1 xd[3] = 1;                    // 122: end offset 3 exactly
    #10 $finish;
  end
endmodule
