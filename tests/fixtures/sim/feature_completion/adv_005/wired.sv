// FIN-001 G3 (F08-11): wand, wor, triand and trior nets as terminals of
// bidirectional switches. Nets joined by tran/rtran collapse into one net of
// the wired type; a net reached through tranif*/rtranif* resolves its own
// drivers and what arrives with its own resolution (SV 28.12.4, 28.8,
// 28.13, 28.14, 23.3.3.7; decision A5-D7).
`timescale 1ns/1ns
module tb;
  // A wand node through a tranif1 (the known-issue reproducer).
  wand w; wire a; reg d1, d2, c;
  assign w = d1; assign w = d2;
  tranif1 t1(w, a, c);
  // A wor net joined by tran to a plain wire: one wired-OR net.
  wor o; wire b; reg e1, e2, f;
  assign o = e1; assign o = e2;
  tran t2(o, b);
  assign b = f;
  // A triand with weak drivers against a strong plain driver.
  triand ta; wire ab; reg g1, g2, h;
  assign (weak0, weak1) ta = g1; assign (weak0, weak1) ta = g2;
  assign (strong0, strong1) ab = h;
  tran t3(ta, ab);
  // A trior through a resistive switch.
  trior tr; wire tw; reg m1, m2;
  assign (pull0, pull1) tr = m1; assign (pull0, pull1) tr = m2;
  rtran t6(tr, tw);
  // A wand vector through an array of tranif1, a wand pair through a
  // tranif1 with an unknown control, and a wor through an rtran.
  wand [3:0] wv; wire [3:0] av; reg [3:0] v0, v1, va; reg cv;
  assign wv = v0; assign wv = v1; assign av = va;
  tranif1 tv[3:0](wv, av, cv);
  wand u1, u2; reg x1, x2, cu;
  assign u1 = x1; assign u2 = x2;
  tranif1 tu(u1, u2, cu);
  wor ww; wire wa; reg z1, z2;
  assign (weak0, weak1) ww = z1; assign (strong0, strong1) wa = z2;
  rtran rt(ww, wa);
  initial begin
    d1 = 1'b1; d2 = 1'b0; c = 1'b1;
    e1 = 1'b0; e2 = 1'b1; f = 1'bz;
    g1 = 1'b1; g2 = 1'b0; h = 1'bz;
    m1 = 1'b0; m2 = 1'b1;
    v0 = 4'b1100; v1 = 4'b1010; va = 4'bzz01; cv = 1'b1;
    x1 = 1'b1; x2 = 1'b0; cu = 1'bx;
    z1 = 1'b1; z2 = 1'b0;
    #1 $display("1 %v %v | %v %v | %v %v | %v %v", w, a, o, b, ta, ab, tr, tw);
    $display("1 %b %b | %v %v | %v %v", wv, av, u1, u2, ww, wa);
    c = 1'b0; f = 1'b0; h = 1'b1;
    cv = 1'b0; cu = 1'b1; z2 = 1'bz;
    #1 $display("2 %v %v | %v %v | %v %v | %v %v", w, a, o, b, ta, ab, tr, tw);
    $display("2 %b %b | %v %v | %v %v", wv, av, u1, u2, ww, wa);
    d1 = 1'bz; d2 = 1'bz; c = 1'b1; f = 1'bz; h = 1'b0; m1 = 1'bz; m2 = 1'bz;
    cv = 1'b1; cu = 1'b0; z1 = 1'bx;
    #1 $display("3 %v %v | %v %v | %v %v | %v %v", w, a, o, b, ta, ab, tr, tw);
    $display("3 %b %b | %v %v | %v %v", wv, av, u1, u2, ww, wa);
    d2 = 1'b0; e1 = 1'bx; e2 = 1'b0; f = 1'b1;
    #1 $display("4 %v %v | %v %v", w, a, o, b);
    force w = 1'b1;
    #1 $display("5 %v %v", w, a);
    release w; d1 = 1'b1;
    #1 $display("6 %v %v", w, a);
    $finish;
  end
endmodule
