// $setup, $hold, $recovery and $removal windows: equality and one tick on
// each side of every boundary, simultaneous events in either order and
// across a #0 (IEEE 1800-2009 31.3, IEEE 1364-2001 15.2; see readme.md).
`timescale 1ns/1ns
module su(input d, input c); reg n; specify $setup(d, posedge c, 10, n); endspecify endmodule
module ho(input d, input c); reg n; specify $hold(posedge c, d, 10, n); endspecify endmodule
module rc(input r, input c); reg n; specify $recovery(posedge r, posedge c, 10, n); endspecify endmodule
module rm(input r, input c); reg n; specify $removal(posedge r, posedge c, 10, n); endspecify endmodule
module tb;
  reg c;
  reg [5:0] sd;
  reg [6:0] hd;
  reg [3:0] rr, rk;
  reg [3:0] mr;
  su s0(sd[0], c); su s1(sd[1], c); su s2(sd[2], c); su s3(sd[3], c); su s4(sd[4], c); su s5(sd[5], c);
  ho h0(hd[0], c); ho h1(hd[1], c); ho h2(hd[2], c); ho h3(hd[3], c); ho h4(hd[4], c); ho h5(hd[5], c);
  ho h6(hd[6], c);
  rc r0(rr[0], rk[0]); rc r1(rr[1], rk[1]); rc r2(rr[2], rk[2]); rc r3(rr[3], rk[3]);
  rm m0(mr[0], c); rm m1(mr[1], c); rm m2(mr[2], c); rm m3(mr[3], c);
  initial
    $monitor("%0t setup=%b%b%b%b%b%b hold=%b%b%b%b%b%b%b recovery=%b%b%b%b removal=%b%b%b%b",
             $time, s5.n, s4.n, s3.n, s2.n, s1.n, s0.n,
             h6.n, h5.n, h4.n, h3.n, h2.n, h1.n, h0.n,
             r3.n, r2.n, r1.n, r0.n, m3.n, m2.n, m1.n, m0.n);
  initial begin
    c = 0; sd = 0; hd = 0; rr = 0; rk = 0; mr = 0;
    #89 sd[0] = 1;                   // 89: 11 before the clock
    #1 sd[1] = 1;                    // 90: exactly the limit before
    #1 sd[2] = 1;                    // 91: violation
    #8 sd[3] = 1; rk[3] = 1;         // 99: violation; recovery 3 before r
    #1 hd[6] = 1; sd[4] = 1;         // 100, an earlier delta than the clock
    #0 hd[0] = 1; c = 1; rr = 4'hf; hd[1] = 1; rk[0] = 1; mr[0] = 1;
    #0 hd[5] = 1; sd[5] = 1;         // 100, a later delta
    #1 hd[2] = 1; mr[1] = 1;         // 101
    #8 hd[3] = 1; rk[1] = 1; mr[2] = 1; // 109
    #1 hd[4] = 1; rk[2] = 1; mr[3] = 1; // 110: end of the windows
    #10 $finish;
  end
endmodule
