// Conditioned events (`&&&`): deterministic and nondeterministic forms with
// 0, 1, x and z, conditions of timestamp events, `$width` and `$nochange`
// (IEEE 1800-2009 31.7, IEEE 1364-2001 15.6; see readme.md).
`timescale 1ns/1ns
module k0(input d, input c, input en); reg n; specify $setup(d, posedge c &&& en, 10, n); endspecify endmodule
module k1(input d, input c, input en); reg n; specify $setup(d, posedge c &&& ~en, 10, n); endspecify endmodule
module k2(input d, input c, input en); reg n; specify $setup(d, posedge c &&& (en == 1), 10, n); endspecify endmodule
module k3(input d, input c, input en); reg n; specify $setup(d, posedge c &&& (en === 1), 10, n); endspecify endmodule
module k4(input d, input c, input en); reg n; specify $setup(d, posedge c &&& (en != 0), 10, n); endspecify endmodule
module k5(input d, input c, input en); reg n; specify $setup(d, posedge c &&& (en !== 0), 10, n); endspecify endmodule
module k6(input d, input c, input en); reg n; specify $setup(d, posedge c &&& (en == 1'b0), 10, n); endspecify endmodule
module ts(input d, input c, input en); reg n; specify $setup(d &&& en, posedge c, 10, n); endspecify endmodule
module th(input d, input c, input en); reg n; specify $hold(posedge c &&& en, d, 10, n); endspecify endmodule
module tw(input c, input en); reg n; specify $width(posedge c &&& en, 10, 0, n); endspecify endmodule
module tn(input c, input d, input en, input den); reg n;
  specify $nochange(posedge c &&& en, d &&& den, 0, 0, n); endspecify
endmodule
module tb;
  reg d, c;
  reg [3:0] en;                      // per index: 0, 1, x, z
  k0 a0 [3:0] (d, c, en); k1 a1 [3:0] (d, c, en); k2 a2 [3:0] (d, c, en); k3 a3 [3:0] (d, c, en);
  k4 a4 [3:0] (d, c, en); k5 a5 [3:0] (d, c, en); k6 a6 [3:0] (d, c, en);
  reg [3:0] sd, sen, hd, hen, wc, wen, nd, nen, nden;
  reg sc, hc, nc;
  ts s [3:0] (sd, sc, sen);
  th h [3:0] (hd, hc, hen);
  tw w [3:0] (wc, wen);
  tn q [3:0] (nc, nd, nen, nden);
  initial
    $monitor("%0t en=%b%b%b%b ~en=%b%b%b%b ==1=%b%b%b%b ===1=%b%b%b%b !=0=%b%b%b%b !==0=%b%b%b%b ==0=%b%b%b%b timestamp=%b%b%b%b hold=%b%b%b%b width=%b%b%b%b nochange=%b%b%b%b",
             $time, a0[3].n, a0[2].n, a0[1].n, a0[0].n, a1[3].n, a1[2].n, a1[1].n, a1[0].n,
             a2[3].n, a2[2].n, a2[1].n, a2[0].n, a3[3].n, a3[2].n, a3[1].n, a3[0].n,
             a4[3].n, a4[2].n, a4[1].n, a4[0].n, a5[3].n, a5[2].n, a5[1].n, a5[0].n,
             a6[3].n, a6[2].n, a6[1].n, a6[0].n, s[3].n, s[2].n, s[1].n, s[0].n,
             h[3].n, h[2].n, h[1].n, h[0].n, w[3].n, w[2].n, w[1].n, w[0].n,
             q[3].n, q[2].n, q[1].n, q[0].n);
  initial begin
    d = 0; c = 0; en = 4'bzx10;
    sd = 0; sc = 0; sen = 4'b0110;
    hd = 0; hc = 0; hen = 4'b0011;
    wc = 0; wen = 4'b0101;
    nc = 0; nd = 0; nen = 4'b0101; nden = 4'b0111;
    #95 d = 1;
    sd = 4'hf; sen[2] = 0; sen[3] = 1;  // 95: timestamp conditions settle to 0, 1, 0, 1
    #5 c = 1; sc = 1; sen[1] = 0;       // 100
    hc = 1; hen[0] = 0; hen[2] = 1;     // 100: hold stamp conditions settle to 0, 1, 1, 0
    wc = 4'hf; nc = 1;                  // 100: width and nochange leading edges
    nen[1] = 1; nen[2] = 0;             // nochange conditions at the data event: 1, 1, 0, 0
    nden[2] = 1; nden[3] = 0;
    #2 hd = 4'hf;                       // 102: hold data events
    #1 wen[2] = 0; wen[3] = 1;          // 103: width conditions at the falling edge: 1, 0, 0, 1
    #2 wc = 0;                          // 105: width 5
    nd = 4'hf;                          // 105: nochange data while c is high
    #10 $finish;
  end
endmodule
