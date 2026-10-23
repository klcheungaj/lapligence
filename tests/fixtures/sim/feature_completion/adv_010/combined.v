// $setuphold and $recrem with non-negative limits: each instance drives the
// reference r high at 100 and the data d high at 100 + OFF (both at 100 in
// one time step, data or reference first, for OFF 0). Each checked module
// counts its notifier changes and keeps the time of the last one (0: none).
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
module sh_10_5(input r, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, 10, 5, n);
  endspecify
endmodule
module sh_10_0(input r, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, 10, 0, n);
  endspecify
endmodule
module sh_0_5(input r, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, 0, 5, n);
  endspecify
endmodule
module rr_10_5(input r, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $recrem(posedge r, posedge d, 10, 5, n);
  endspecify
endmodule
module tb;
  wire [9:0] ar, ad;
  wire [5:0] br, bd, cr, cd;
  wire [9:0] er, ed;
  // $setuphold(posedge r, d, 10, 5): violations for data in (90, 105).
  drive #(-11) a0d(ar[0], ad[0]); sh_10_5 a0(ar[0], ad[0]);
  drive #(-10) a1d(ar[1], ad[1]); sh_10_5 a1(ar[1], ad[1]);
  drive #(-9) a2d(ar[2], ad[2]); sh_10_5 a2(ar[2], ad[2]);
  drive #(-1) a3d(ar[3], ad[3]); sh_10_5 a3(ar[3], ad[3]);
  drive #(0, 1) a4d(ar[4], ad[4]); sh_10_5 a4(ar[4], ad[4]);
  drive #(0, 0) a5d(ar[5], ad[5]); sh_10_5 a5(ar[5], ad[5]);
  drive #(1) a6d(ar[6], ad[6]); sh_10_5 a6(ar[6], ad[6]);
  drive #(4) a7d(ar[7], ad[7]); sh_10_5 a7(ar[7], ad[7]);
  drive #(5) a8d(ar[8], ad[8]); sh_10_5 a8(ar[8], ad[8]);
  drive #(6) a9d(ar[9], ad[9]); sh_10_5 a9(ar[9], ad[9]);
  // $setuphold(posedge r, d, 10, 0): data in (90, 100) only.
  drive #(-10) b0d(br[0], bd[0]); sh_10_0 b0(br[0], bd[0]);
  drive #(-9) b1d(br[1], bd[1]); sh_10_0 b1(br[1], bd[1]);
  drive #(-1) b2d(br[2], bd[2]); sh_10_0 b2(br[2], bd[2]);
  drive #(0, 1) b3d(br[3], bd[3]); sh_10_0 b3(br[3], bd[3]);
  drive #(0, 0) b4d(br[4], bd[4]); sh_10_0 b4(br[4], bd[4]);
  drive #(1) b5d(br[5], bd[5]); sh_10_0 b5(br[5], bd[5]);
  // $setuphold(posedge r, d, 0, 5): data in [100, 105) only.
  drive #(-1) c0d(cr[0], cd[0]); sh_0_5 c0(cr[0], cd[0]);
  drive #(0, 1) c1d(cr[1], cd[1]); sh_0_5 c1(cr[1], cd[1]);
  drive #(0, 0) c2d(cr[2], cd[2]); sh_0_5 c2(cr[2], cd[2]);
  drive #(1) c3d(cr[3], cd[3]); sh_0_5 c3(cr[3], cd[3]);
  drive #(4) c4d(cr[4], cd[4]); sh_0_5 c4(cr[4], cd[4]);
  drive #(5) c5d(cr[5], cd[5]); sh_0_5 c5(cr[5], cd[5]);
  // $recrem(posedge r, posedge d, 10, 5): removal for d in (95, 100),
  // recovery for d in [100, 110).
  drive #(-6) e0d(er[0], ed[0]); rr_10_5 e0(er[0], ed[0]);
  drive #(-5) e1d(er[1], ed[1]); rr_10_5 e1(er[1], ed[1]);
  drive #(-4) e2d(er[2], ed[2]); rr_10_5 e2(er[2], ed[2]);
  drive #(-1) e3d(er[3], ed[3]); rr_10_5 e3(er[3], ed[3]);
  drive #(0, 1) e4d(er[4], ed[4]); rr_10_5 e4(er[4], ed[4]);
  drive #(0, 0) e5d(er[5], ed[5]); rr_10_5 e5(er[5], ed[5]);
  drive #(1) e6d(er[6], ed[6]); rr_10_5 e6(er[6], ed[6]);
  drive #(9) e7d(er[7], ed[7]); rr_10_5 e7(er[7], ed[7]);
  drive #(10) e8d(er[8], ed[8]); rr_10_5 e8(er[8], ed[8]);
  drive #(11) e9d(er[9], ed[9]); rr_10_5 e9(er[9], ed[9]);
  initial begin
    #300;
    $display("setuphold 10 5: -11 %0d@%0d -10 %0d@%0d -9 %0d@%0d -1 %0d@%0d 0d %0d@%0d",
             a0.count, a0.last, a1.count, a1.last, a2.count, a2.last, a3.count, a3.last,
             a4.count, a4.last);
    $display("setuphold 10 5: 0r %0d@%0d +1 %0d@%0d +4 %0d@%0d +5 %0d@%0d +6 %0d@%0d",
             a5.count, a5.last, a6.count, a6.last, a7.count, a7.last, a8.count, a8.last,
             a9.count, a9.last);
    $display("setuphold 10 0: -10 %0d@%0d -9 %0d@%0d -1 %0d@%0d 0d %0d@%0d 0r %0d@%0d +1 %0d@%0d",
             b0.count, b0.last, b1.count, b1.last, b2.count, b2.last, b3.count, b3.last,
             b4.count, b4.last, b5.count, b5.last);
    $display("setuphold 0 5: -1 %0d@%0d 0d %0d@%0d 0r %0d@%0d +1 %0d@%0d +4 %0d@%0d +5 %0d@%0d",
             c0.count, c0.last, c1.count, c1.last, c2.count, c2.last, c3.count, c3.last,
             c4.count, c4.last, c5.count, c5.last);
    $display("recrem 10 5: -6 %0d@%0d -5 %0d@%0d -4 %0d@%0d -1 %0d@%0d 0d %0d@%0d",
             e0.count, e0.last, e1.count, e1.last, e2.count, e2.last, e3.count, e3.last,
             e4.count, e4.last);
    $display("recrem 10 5: 0r %0d@%0d +1 %0d@%0d +9 %0d@%0d +10 %0d@%0d +11 %0d@%0d",
             e5.count, e5.last, e6.count, e6.last, e7.count, e7.last, e8.count, e8.last,
             e9.count, e9.last);
    $finish;
  end
endmodule
