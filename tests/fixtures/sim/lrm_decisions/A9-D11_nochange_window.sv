// Decision A9-D11: the $nochange window (leading edge - start offset,
// trailing edge + end offset) excludes its end points, and a data change in
// the time step of an edge is outside a window with zero offsets. A data
// change before the leading edge is reported at the leading edge, one after
// the trailing edge when it occurs.
//
// IEEE 1800-2009 31.4.6 (SystemVerilog-1800-2009.txt L51959-51962): "(beginning
// of time window) < (data event time) < (end of time window) The end points
// of the time window are not included." L51971-51972: "It shall not be a
// violation if posedge clk and a transition on data occur simultaneously."
`timescale 1ns/1ns
module nc(input c, input d); reg n; specify $nochange(posedge c, d, 0, 0, n); endspecify endmodule
module nx(input c, input d); reg n; specify $nochange(posedge c, d, 2, 3, n); endspecify endmodule
module tb;
  reg c;
  reg [3:0] qd, xd;
  nc q0(c, qd[0]); nc q1(c, qd[1]); nc q2(c, qd[2]); nc q3(c, qd[3]);
  nx x0(c, xd[0]); nx x1(c, xd[1]); nx x2(c, xd[2]); nx x3(c, xd[3]);
  always @(x1.n) if ($time > 0) $display("%0t x1 reported", $time);
  initial $monitor("%0t zero=%b%b%b%b offsets=%b%b%b%b", $time,
                   q3.n, q2.n, q1.n, q0.n, x3.n, x2.n, x1.n, x0.n);
  initial begin
    c = 0; qd = 0; xd = 0;
    #98 xd[0] = 1;               // 98: the start offset exactly
    #1 xd[1] = 1;                // 99: inside, reported at the leading edge
    #1 qd[0] = 1; c = 1;         // 100: data with the leading edge
    #1 qd[1] = 1;                // 101: inside
    #19 qd[2] = 1; c = 0;        // 120: data with the trailing edge
    #2 xd[2] = 1;                // 122: inside (..., 123)
    #1 xd[3] = 1;                // 123: the end offset exactly
    qd[3] = 1;                   // 123: after the zero window
    #10 $finish;
  end
endmodule
