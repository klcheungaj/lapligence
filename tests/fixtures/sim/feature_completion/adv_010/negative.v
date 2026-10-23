// Negative $setuphold/$recrem limits (run with --neg-timing-checks, and
// without it for negative_off.out): each instance drives the reference r high
// at 100 and the data d high at 100 + OFF; the checked modules count their
// notifier changes and keep the time of the last one (0: none). The trace
// instances print their delayed signals after time 50.
`timescale 1ns/1ns
module drive #(parameter integer OFF = 0, parameter integer DATA_FIRST = 0)
  (output reg r, output reg d);
  initial begin
    r = 1'b0;
    d = 1'b0;
    if (OFF < 0) begin
      #(100 + OFF) d = 1'b1;
      #(-OFF) r = 1'b1;
    end else if (OFF > 0) begin
      #100 r = 1'b1;
      #(OFF) d = 1'b1;
    end else if (DATA_FIRST != 0) begin
      #100 d = 1'b1;
      r = 1'b1;
    end else begin
      #100 r = 1'b1;
      d = 1'b1;
    end
  end
endmodule
module na(input r, input d);
  reg n;
  wire dr, dd;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, -3, 8, n, , , dr, dd);
  endspecify
endmodule
module nb(input r, input d);
  reg n;
  wire dr, dd;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, 8, -3, n, , , dr, dd);
  endspecify
endmodule
module nc(input r, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, -5, 7, n);
  endspecify
endmodule
module nd(input r, input d);
  reg n;
  wire dr, dd;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $recrem(posedge r, posedge d, 8, -3, n, , , dr, dd);
  endspecify
endmodule
module ne(input r, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, -5, 4, n);
  endspecify
endmodule
module nf(input r, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, -5, 5, n);
  endspecify
endmodule
module pe(input C, input D);
  reg n;
  wire dC, dD;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge C, posedge D, -3, 8, n, , , dC, dD);
    $setuphold(posedge C, negedge D, -7, 13, n, , , dC, dD);
  endspecify
endmodule
module pg(input C, input D);
  reg n;
  wire dC, dD;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge C, posedge D, 8, -5, n, , , dC, dD);
    $setuphold(posedge C, negedge D, 8, -1, n, , , dC, dD);
  endspecify
endmodule
module tb;
  wire [5:0] ar, ad;
  wire [5:0] br, bd;
  wire [2:0] cr, cd;
  wire [3:0] er, ed;
  wire [2:0] fr, fd;
  wire [2:0] gr, gd;
  drive #(2) a0d(ar[0], ad[0]); na a0(ar[0], ad[0]);
  drive #(3) a1d(ar[1], ad[1]); na a1(ar[1], ad[1]);
  drive #(4) a2d(ar[2], ad[2]); na a2(ar[2], ad[2]);
  drive #(7) a3d(ar[3], ad[3]); na a3(ar[3], ad[3]);
  drive #(8) a4d(ar[4], ad[4]); na a4(ar[4], ad[4]);
  drive #(9) a5d(ar[5], ad[5]); na a5(ar[5], ad[5]);
  drive #(-9) b0d(br[0], bd[0]); nb b0(br[0], bd[0]);
  drive #(-8) b1d(br[1], bd[1]); nb b1(br[1], bd[1]);
  drive #(-7) b2d(br[2], bd[2]); nb b2(br[2], bd[2]);
  drive #(-4) b3d(br[3], bd[3]); nb b3(br[3], bd[3]);
  drive #(-3) b4d(br[4], bd[4]); nb b4(br[4], bd[4]);
  drive #(-2) b5d(br[5], bd[5]); nb b5(br[5], bd[5]);
  drive #(5) c0d(cr[0], cd[0]); nc c0(cr[0], cd[0]);
  drive #(6) c1d(cr[1], cd[1]); nc c1(cr[1], cd[1]);
  drive #(7) c2d(cr[2], cd[2]); nc c2(cr[2], cd[2]);
  drive #(3) e0d(er[0], ed[0]); nd e0(er[0], ed[0]);
  drive #(4) e1d(er[1], ed[1]); nd e1(er[1], ed[1]);
  drive #(7) e2d(er[2], ed[2]); nd e2(er[2], ed[2]);
  drive #(8) e3d(er[3], ed[3]); nd e3(er[3], ed[3]);
  drive #(0, 0) f0d(fr[0], fd[0]); ne f0(fr[0], fd[0]);
  drive #(3) f1d(fr[1], fd[1]); ne f1(fr[1], fd[1]);
  drive #(4) f2d(fr[2], fd[2]); ne f2(fr[2], fd[2]);
  drive #(4) g0d(gr[0], gd[0]); nf g0(gr[0], gd[0]);
  drive #(5) g1d(gr[1], gd[1]); nf g1(gr[1], gd[1]);
  drive #(6) g2d(gr[2], gd[2]); nf g2(gr[2], gd[2]);
  // Delayed signal traces.
  reg tr, td, ur, ud, uc, uv, held;
  na t(tr, td);
  nb u(ur, ud);
  pe v(uc, held);
  pg w(held, uv);
  initial begin
    tr = 1'b0; td = 1'b0; ur = 1'b0; ud = 1'b0; uc = 1'b0; uv = 1'b0; held = 1'b0;
    #100 tr = 1'b1; ur = 1'b1;
    #10 td = 1'b1;
    #1 td = 1'b0;
    #9 tr = 1'b0;
    #10 ud = 1'b1;
    #1 ud = 1'b0;
    #9 ud = 1'bx;
    #10 ud = 1'b0;
    #20 uc = 1'b1;
    #30 uv = 1'b1;
    #1 uv = 1'b0;
    #9 uv = 1'b1;
    #10 uv = 1'b0;
  end
  always @(t.dr) if ($time > 50) $display("%0t na dr=%b", $time, t.dr);
  always @(t.dd) if ($time > 50) $display("%0t na dd=%b", $time, t.dd);
  always @(u.dd) if ($time > 50) $display("%0t nb dd=%b", $time, u.dd);
  always @(v.dC) if ($time > 50) $display("%0t pe dC=%b", $time, v.dC);
  always @(w.dD) if ($time > 50) $display("%0t pg dD=%b", $time, w.dD);
  initial begin
    #300;
    $display("na: +2 %0d@%0d +3 %0d@%0d +4 %0d@%0d +7 %0d@%0d +8 %0d@%0d +9 %0d@%0d",
             a0.count, a0.last, a1.count, a1.last, a2.count, a2.last, a3.count, a3.last, a4.count, a4.last, a5.count, a5.last);
    $display("nb: -9 %0d@%0d -8 %0d@%0d -7 %0d@%0d -4 %0d@%0d -3 %0d@%0d -2 %0d@%0d",
             b0.count, b0.last, b1.count, b1.last, b2.count, b2.last, b3.count, b3.last, b4.count, b4.last, b5.count, b5.last);
    $display("nc: +5 %0d@%0d +6 %0d@%0d +7 %0d@%0d",
             c0.count, c0.last, c1.count, c1.last, c2.count, c2.last);
    $display("nd: +3 %0d@%0d +4 %0d@%0d +7 %0d@%0d +8 %0d@%0d",
             e0.count, e0.last, e1.count, e1.last, e2.count, e2.last, e3.count, e3.last);
    $display("ne: 0r %0d@%0d +3 %0d@%0d +4 %0d@%0d",
             f0.count, f0.last, f1.count, f1.last, f2.count, f2.last);
    $display("nf: +4 %0d@%0d +5 %0d@%0d +6 %0d@%0d",
             g0.count, g0.last, g1.count, g1.last, g2.count, g2.last);
    $finish;
  end
endmodule
