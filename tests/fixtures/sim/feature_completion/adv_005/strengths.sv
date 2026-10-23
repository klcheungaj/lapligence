// ADV-005: strength through tran and rtran (IEEE 1800-2009 28.13, 28.14
// Table 28-8; IEEE 1364-2001 7.11, 7.12 Table 38) and the strengths of
// tri0, tri1, supply0, supply1 and pullup sources (28.15).
`timescale 1ns/1ns
module tb;
  // Each source strength through one tran and one rtran.
  wire su_a, su_b, su_c;
  wire st_a, st_b, st_c;
  wire pu_a, pu_b, pu_c;
  wire we_a, we_b, we_c;
  wire x_a, x_b, x_c;
  wire h_a, h_b, h_c;
  reg en;
  assign (supply0, supply1) su_a = 1'b0;
  assign (strong0, strong1) st_a = 1'b1;
  assign (pull0, pull1) pu_a = 1'b0;
  assign (weak0, weak1) we_a = 1'b1;
  assign (strong0, pull1) x_a = 1'bx;
  bufif1 (strong0, strong1) bh(h_a, 1'b1, en);
  tran a1(su_a, su_b); rtran b1(su_a, su_c);
  tran a2(st_a, st_b); rtran b2(st_a, st_c);
  tran a3(pu_a, pu_b); rtran b3(pu_a, pu_c);
  tran a4(we_a, we_b); rtran b4(we_a, we_c);
  tran a5(x_a, x_b);   rtran b5(x_a, x_c);
  tran a6(h_a, h_b);   rtran b6(h_a, h_c);

  // A chain: two trans, then rtrans down to small capacitor strength.
  wire a, b, c, d, e, f, g;
  reg ra;
  assign (strong0, strong1) a = ra;
  tran t1(a, b);
  tran t2(b, c);
  rtran t3(c, d);
  rtran t4(d, e);
  rtran t5(e, f);
  rtran t6(f, g);

  // Supply and pull net types as sources.
  supply1 vdd;
  supply0 gnd;
  wire s1, s2, s3, s4, s5, s6;
  tran u1(vdd, s1);
  rtran u2(s1, s2);
  rtran u3(s2, s3);
  rtran u4(s3, s4);
  rtran u5(s4, s5);
  rtran u6(s5, s6);
  wire g1, g2;
  reg r;
  assign (strong0, strong1) g1 = r;
  tran u7(gnd, g1);
  rtran u8(g1, g2);
  tri0 n0; tri1 n1; wire w0, w1;
  tran u9(n0, w0);
  rtran u10(n1, w1);
  wire p, pw;
  pullup (p);
  tran u11(p, pw);
  tri1 n2; wire w3; reg r3;
  assign (weak0, weak1) w3 = r3;
  tran u12(n2, w3);

  initial begin
    en = 1'bx; r = 1; r3 = 0;
    #1;
    $display("su %v %v %v", su_a, su_b, su_c);
    $display("st %v %v %v", st_a, st_b, st_c);
    $display("pu %v %v %v", pu_a, pu_b, pu_c);
    $display("we %v %v %v", we_a, we_b, we_c);
    $display("x  %v %v %v", x_a, x_b, x_c);
    $display("h  %v %v %v", h_a, h_b, h_c);
    ra = 1'bz; #1 $display("z: %v %v %v %v %v %v %v", a, b, c, d, e, f, g);
    ra = 1;    #1 $display("1: %v %v %v %v %v %v %v", a, b, c, d, e, f, g);
    ra = 0;    #1 $display("0: %v %v %v %v %v %v %v", a, b, c, d, e, f, g);
    ra = 1'bx; #1 $display("x: %v %v %v %v %v %v %v", a, b, c, d, e, f, g);
    ra = 1'bz; #1 $display("z: %v %v %v %v %v %v %v", a, b, c, d, e, f, g);
    $display("supply %v %v %v %v %v %v %v | %v %v %v", vdd, s1, s2, s3, s4, s5, s6, gnd, g1, g2);
    $display("pull %v %v %v %v | %v %v | %v %v", n0, w0, n1, w1, p, pw, n2, w3);
    r = 1'bz; r3 = 1'bz; #1
    $display("supply %v %v %v | %v %v", gnd, g1, g2, n2, w3);
    $display("bits %b%b%b%b%b%b%b %b%b", a, b, c, d, e, f, g, g1, w3);
    $finish;
  end
endmodule
