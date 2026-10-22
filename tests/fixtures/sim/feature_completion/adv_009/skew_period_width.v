// $skew, $period and $width boundaries (IEEE 1800-2009 31.4.1, 31.4.4,
// 31.4.5; IEEE 1364-2001 15.3.1, 15.3.4, 15.3.5; see readme.md).
`timescale 1ns/1ns
module sk(input a, input b); reg n; specify $skew(posedge a, posedge b, 10, n); endspecify endmodule
module sk0(input a, input b); reg n; specify $skew(posedge a, posedge b, 0, n); endspecify endmodule
module pe(input c); reg n; specify $period(posedge c, 10, n); endspecify endmodule
module wi(input c); reg n; specify $width(posedge c, 10, 2, n); endspecify endmodule
module tb;
  reg [4:0] ka, kb;
  reg [1:0] za, zb;
  reg [2:0] pc;
  reg [5:0] wc;
  sk k0(ka[0], kb[0]); sk k1(ka[1], kb[1]); sk k2(ka[2], kb[2]); sk k3(ka[3], kb[3]); sk k4(ka[4], kb[4]);
  sk0 z0(za[0], zb[0]); sk0 z1(za[1], zb[1]);
  pe p0(pc[0]); pe p1(pc[1]); pe p2(pc[2]);
  wi w0(wc[0]); wi w1(wc[1]); wi w2(wc[2]); wi w3(wc[3]); wi w4(wc[4]); wi w5(wc[5]);
  initial
    $monitor("%0t skew=%b%b%b%b%b skew0=%b%b period=%b%b%b width=%b%b%b%b%b%b",
             $time, k4.n, k3.n, k2.n, k1.n, k0.n, z1.n, z0.n, p2.n, p1.n, p0.n,
             w5.n, w4.n, w3.n, w2.n, w1.n, w0.n);
  initial begin
    ka = 0; kb = 0; za = 0; zb = 0; pc = 0; wc = 0;
    #95 kb[3] = 1;                    // 95: data before any reference
    #5 ka = 5'h1f; za = 2'b11; kb[2] = 1; zb[0] = 1; pc = 3'b111; wc = 6'h3f;
    wc[5] = 0;                        // 100: zero width
    #1 zb[1] = 1; wc[0] = 0;          // 101: zero-limit skew one tick late; width 1
    #1 wc[1] = 0;                     // 102: width 2 (the threshold)
    #1 wc[2] = 0;                     // 103: width 3
    #2 ka[4] = 0; pc = 0;             // 105
    #1 wc[3] = 0;                     // 106: width 6
    #1 ka[4] = 1;                     // 107: a new reference event restarts $skew
    #2 pc[0] = 1;                     // 109: period 9
    #1 kb[0] = 1; pc[1] = 1; wc[4] = 0; // 110: skew 10, period 10, width 10
    #1 kb[1] = 1; pc[2] = 1;          // 111: skew 11, period 11
    #1 kb[1] = 0; pc[2] = 0;          // 112
    #1 kb[1] = 1;                     // 113: skew 13 again
    #2 pc[2] = 1;                     // 115: period 4
    #1 kb[4] = 1;                     // 116: skew 9 after the second reference
    #10 $finish;
  end
endmodule
