// ADV-005: switch loops, self loops and opposing drivers (IEEE 1800-2009
// 4.9.5: switch processing "shall consider all the devices in a
// bidirectional switch-connected net"; 28.12.1 combined signals).
`timescale 1ns/1ns
module tb;
  // A tran loop, an rtran loop and a self-connected tran.
  wire a, b, c, d;
  reg ra, en;
  assign (strong0, strong1) a = ra;
  tran t1(a, b);
  tran t2(b, c);
  tran t3(c, d);
  tran t4(d, b);
  wire p, q, r;
  reg rp;
  assign (pull0, pull1) p = rp;
  rtran u1(p, q);
  rtran u2(q, r);
  rtran u3(r, p);
  wire m, n;
  reg rm;
  assign m = rm;
  tranif1 w1(m, n, en);
  tran w2(n, n);
  // Opposing drivers on both sides of a switch.
  wire oa, ob;
  reg ora, orb;
  assign (strong0, strong1) oa = ora;
  assign (pull0, pull1) ob = orb;
  tran o1(oa, ob);
  wire od, oe, of;
  reg ord, orf;
  assign (strong0, strong1) od = ord;
  assign (strong0, strong1) of = orf;
  tran o2(od, oe);
  rtran o3(oe, of);
  wire og, oh;
  reg org, orh;
  assign (weak0, weak1) og = org;
  assign (strong0, strong1) oh = orh;
  rtran o4(og, oh);
  initial begin
    ra = 1; rp = 0; rm = 1; en = 1;
    ora = 0; orb = 1; ord = 0; orf = 1; org = 1; orh = 0; #1
    $display("1 %v %v %v %v | %v %v %v | %v %v", a, b, c, d, p, q, r, m, n);
    $display("1 %v %v | %v %v %v | %v %v", oa, ob, od, oe, of, og, oh);
    ra = 1'bz; rp = 1'bz; en = 0;
    ora = 1; orb = 0; ord = 1; orf = 1; org = 0; orh = 1'bz; #1
    $display("2 %v %v %v %v | %v %v %v | %v %v", a, b, c, d, p, q, r, m, n);
    $display("2 %v %v | %v %v %v | %v %v", oa, ob, od, oe, of, og, oh);
    ra = 0; rp = 1; en = 1;
    ora = 1'bz; orb = 1'bx; ord = 1'bx; orf = 0; org = 1'bx; orh = 1; #1
    $display("3 %v %v %v %v | %v %v %v | %v %v", a, b, c, d, p, q, r, m, n);
    $display("3 %v %v | %v %v %v | %v %v", oa, ob, od, oe, of, og, oh);
    rm = 1'bz;
    ora = 1'bx; orb = 1; ord = 0; orf = 1'bx; org = 1'bz; orh = 1'bx; #1
    $display("4 %v %v %v %v | %v %v %v | %v %v", a, b, c, d, p, q, r, m, n);
    $display("4 %v %v | %v %v %v | %v %v", oa, ob, od, oe, of, og, oh);
    $display("bits %b%b%b%b %b%b%b %b%b", a, b, c, d, p, q, r, m, n);
    $finish;
  end
endmodule
