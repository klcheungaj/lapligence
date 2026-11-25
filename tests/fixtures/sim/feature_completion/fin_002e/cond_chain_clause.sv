// Conditional chains over unpacked arrays, unpacked structures, class handles
// and reals with ambiguous predicates, and increments in alternatives that
// are not selected (IEEE 1800-2009 11.4.11).
`timescale 1ns/1ns
module tb;
  typedef struct { logic [7:0] a; int b; } rec_t;
  class item; int id; function new(int v); id = v; endfunction endclass
  logic [7:0] s;
  logic [7:0] ar [9][4]; logic [7:0] yar [4];
  int ia [9][3]; int yia [3];
  rec_t rc [9]; rec_t yrc;
  item it [9]; item yit;
  real rl [9]; real yr;
  int ci, cj, ck, cl; logic u; logic [15:0] y;
  task automatic eval(string what);
    yar = s[0] ? ar[0] : s[1] ? ar[1] : s[2] ? ar[2] : s[3] ? ar[3] : s[4] ? ar[4] : s[5] ? ar[5] : s[6] ? ar[6] : s[7] ? ar[7] : ar[8];
    yia = s[0] ? ia[0] : s[1] ? ia[1] : s[2] ? ia[2] : s[3] ? ia[3] : s[4] ? ia[4] : s[5] ? ia[5] : s[6] ? ia[6] : s[7] ? ia[7] : ia[8];
    yrc = s[0] ? rc[0] : s[1] ? rc[1] : s[2] ? rc[2] : s[3] ? rc[3] : s[4] ? rc[4] : s[5] ? rc[5] : s[6] ? rc[6] : s[7] ? rc[7] : rc[8];
    yit = s[0] ? it[0] : s[1] ? it[1] : s[2] ? it[2] : s[3] ? it[3] : s[4] ? it[4] : s[5] ? it[5] : s[6] ? it[6] : s[7] ? it[7] : it[8];
    yr = s[0] ? rl[0] : s[1] ? rl[1] : s[2] ? rl[2] : s[3] ? rl[3] : s[4] ? rl[4] : s[5] ? rl[5] : s[6] ? rl[6] : s[7] ? rl[7] : rl[8];
    $display("%s s=%b yar=%h %h %h %h yia=%0d %0d %0d yrc=%h %0d yit=%0d yr=%g", what, s, yar[0], yar[1], yar[2], yar[3], yia[0], yia[1], yia[2], yrc.a, yrc.b, yit == null ? -1 : yit.id, yr);
  endtask
  initial begin
    for (int i = 0; i <= 8; i++) begin
      for (int e = 0; e < 4; e++) ar[i][e] = 8'(i * 16 + e);
      for (int e = 0; e < 3; e++) ia[i][e] = i * 100 + e;
      rc[i] = '{8'(i + 8'h40), i * 11};
      it[i] = new(i + 500); rl[i] = i * 1.25 + 0.5;
    end
    for (int pos = 0; pos < 8; pos++) begin
      s = '0; s[pos] = 1'b1; eval("one");
      s = '0; s[pos] = 1'bx; eval("x");
      s = '0; s[pos] = 1'bz; if (pos > 0) s[pos - 1] = 1'bx; eval("zx");
      s = '0; s[pos] = 1'bx; s[7] = 1'b1; eval("x1");
    end
    s = '0; eval("none");
    s = 'x; eval("allx");
    for (int i = 0; i <= 8; i++) begin
      for (int e = 0; e < 4; e++) ar[i][e] = 8'(e + 1);
      for (int e = 0; e < 3; e++) ia[i][e] = e + 10;
      rc[i] = '{8'h11, 22}; it[i] = it[0]; rl[i] = 2.5;
    end
    s = 'x; eval("same");
    s = 8'b0000x000; ar[3][2] = 8'hee; ia[8][1] = -1; rc[3].b = 23; eval("near");
    s = 8'b0000x000; ar[3][0] = 8'h7x; ar[8][0] = 8'h7x; eval("xleaf");
    ci = 10; cj = 20; ck = 30; cl = 40; u = 1'b1;
    y = u ? 16'(ci++) : !u ? 16'(cj++) : 16'(ck++);
    $display("inc u=%b y=%b i=%0d j=%0d k=%0d", u, y, ci, cj, ck);
    y = (u ? 16'(++ci) : 16'(--cj)) + (u ? 16'(ck--) : 16'(cl++));
    $display("inc2 u=%b y=%b i=%0d j=%0d k=%0d l=%0d", u, y, ci, cj, ck, cl);
    ci = 10; cj = 20; ck = 30; cl = 40; u = 1'b0;
    y = u ? 16'(ci++) : !u ? 16'(cj++) : 16'(ck++);
    $display("inc u=%b y=%b i=%0d j=%0d k=%0d", u, y, ci, cj, ck);
    y = (u ? 16'(++ci) : 16'(--cj)) + (u ? 16'(ck--) : 16'(cl++));
    $display("inc2 u=%b y=%b i=%0d j=%0d k=%0d l=%0d", u, y, ci, cj, ck, cl);
    ci = 10; cj = 20; ck = 30; cl = 40; u = 1'bx;
    y = u ? 16'(ci++) : !u ? 16'(cj++) : 16'(ck++);
    $display("inc u=%b y=%b i=%0d j=%0d k=%0d", u, y, ci, cj, ck);
    y = (u ? 16'(++ci) : 16'(--cj)) + (u ? 16'(ck--) : 16'(cl++));
    $display("inc2 u=%b y=%b i=%0d j=%0d k=%0d l=%0d", u, y, ci, cj, ck, cl);
    ci = 10; cj = 20; ck = 30; cl = 40; u = 1'bz;
    y = u ? 16'(ci++) : !u ? 16'(cj++) : 16'(ck++);
    $display("inc u=%b y=%b i=%0d j=%0d k=%0d", u, y, ci, cj, ck);
    y = (u ? 16'(++ci) : 16'(--cj)) + (u ? 16'(ck--) : 16'(cl++));
    $display("inc2 u=%b y=%b i=%0d j=%0d k=%0d l=%0d", u, y, ci, cj, ck, cl);
  end
endmodule
