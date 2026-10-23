// Decision A9-D12: after a reference event every data event beyond the
// limit violates $skew, a new reference event restarts the measurement,
// simultaneous events never violate and a zero limit is exceeded one tick
// later.
//
// IEEE 1800-2009 31.4.1 (SystemVerilog-1800-2009.txt L51438-51443):
// "(timecheck time) - (timestamp time) > limit Simultaneous transitions on
// the reference and data signals shall not cause $skew to report a timing
// violation, even when the skew limit value is zero." L51452-51456: "A second
// consecutive reference event shall cancel the old wait ... $skew shall
// report timing violations for all data events occurring beyond the limit
// after a reference event."
`timescale 1ns/1ns
module sk(input a, input b); reg n; specify $skew(posedge a, posedge b, 10, n); endspecify endmodule
module s0(input a, input b); reg n; specify $skew(posedge a, posedge b, 0, n); endspecify endmodule
module tb;
  reg [1:0] a, b;
  reg za, zb, ya, yb;
  sk k0(a[0], b[0]); sk k1(a[1], b[1]);
  s0 z(za, zb); s0 y(ya, yb);
  initial $monitor("%0t skew=%b%b zero=%b%b", $time, k1.n, k0.n, y.n, z.n);
  initial begin
    a = 0; b = 0; za = 0; zb = 0; ya = 0; yb = 0;
    #100 a = 2'b11; za = 1; zb = 1; ya = 1;  // 100
    #1 yb = 1;                    // 101: zero limit, one tick late
    #4 a[1] = 0;
    #1 a[1] = 1;                  // 106: k1 restarts
    #5 b = 2'b11;                 // 111: k0 11 late, k1 5
    #1 b = 0;
    #1 b = 2'b11;                 // 113: k0 again
    #10 $finish;
  end
endmodule
