// $setuphold/$recrem timestamp and timecheck conditions (31.9.2): each
// instance drives its reference r high at 100 and its data d high once, and
// sets the conditions as listed in the readme. Run with --neg-timing-checks
// (conditions.out) and without it (conditions_off.out); only the negative
// checks of c17 and c18 differ. Each checked module counts its notifier
// changes and keeps the time of the last one (0: none).
`timescale 1ns/1ns
module sc(input r, input d, input ts, input tc);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, 10, 5, n, ts, tc);
  endspecify
endmodule
module se(input r, input d, input ts, input tc);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, 10, 5, n, (ts == 1'b1), (tc === 1'b1));
  endspecify
endmodule
module sa(input r, input d, input en, input ts, input tc);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r &&& en, d, 10, 5, n, ts, tc);
  endspecify
endmodule
module rc(input r, input d, input ts, input tc);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $recrem(posedge r, posedge d, 10, 5, n, ts, tc);
  endspecify
endmodule
module sn(input r, input d, input ts);
  reg n;
  wire dr, dd;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, -3, 8, n, ts, , dr, dd);
  endspecify
endmodule
module tb;
  reg c1_r, c1_d, c1_ts, c1_tc;
  sc c1(c1_r, c1_d, c1_ts, c1_tc);
  initial begin
    c1_r = 1'b0;
    c1_d = 1'b0;
    c1_ts = 1'b1;
    c1_tc = 1'b1;
    #95 c1_d = 1'b1;
    #5 c1_r = 1'b1;
  end
  reg c2_r, c2_d, c2_ts, c2_tc;
  sc c2(c2_r, c2_d, c2_ts, c2_tc);
  initial begin
    c2_r = 1'b0;
    c2_d = 1'b0;
    c2_ts = 1'b0;
    c2_tc = 1'b1;
    #95 c2_d = 1'b1;
    #5 c2_r = 1'b1;
  end
  reg c3_r, c3_d, c3_ts, c3_tc;
  sc c3(c3_r, c3_d, c3_ts, c3_tc);
  initial begin
    c3_r = 1'b0;
    c3_d = 1'b0;
    c3_ts = 1'b1;
    c3_tc = 1'b0;
    #95 c3_d = 1'b1;
    #5 c3_r = 1'b1;
  end
  reg c4_r, c4_d, c4_ts, c4_tc;
  sc c4(c4_r, c4_d, c4_ts, c4_tc);
  initial begin
    c4_r = 1'b0;
    c4_d = 1'b0;
    c4_ts = 1'b1;
    c4_tc = 1'b1;
    #100 c4_r = 1'b1;
    #2 c4_d = 1'b1;
  end
  reg c5_r, c5_d, c5_ts, c5_tc;
  sc c5(c5_r, c5_d, c5_ts, c5_tc);
  initial begin
    c5_r = 1'b0;
    c5_d = 1'b0;
    c5_ts = 1'b0;
    c5_tc = 1'b1;
    #100 c5_r = 1'b1;
    #2 c5_d = 1'b1;
  end
  reg c6_r, c6_d, c6_ts, c6_tc;
  sc c6(c6_r, c6_d, c6_ts, c6_tc);
  initial begin
    c6_r = 1'b0;
    c6_d = 1'b0;
    c6_ts = 1'b1;
    c6_tc = 1'b0;
    #100 c6_r = 1'b1;
    #2 c6_d = 1'b1;
  end
  reg c7_r, c7_d, c7_ts, c7_tc;
  sc c7(c7_r, c7_d, c7_ts, c7_tc);
  initial begin
    c7_r = 1'b0;
    c7_d = 1'b0;
    c7_ts = 1'b1;
    c7_tc = 1'b1;
    #95 c7_d = 1'b1;
    #3 c7_ts = 1'b0;
    #2 c7_r = 1'b1;
  end
  reg c8_r, c8_d, c8_ts, c8_tc;
  sc c8(c8_r, c8_d, c8_ts, c8_tc);
  initial begin
    c8_r = 1'b0;
    c8_d = 1'b0;
    c8_ts = 1'b1;
    c8_tc = 1'b0;
    #95 c8_d = 1'b1;
    #4 c8_tc = 1'b1;
    #1 c8_r = 1'b1;
  end
  reg c9_r, c9_d, c9_ts, c9_tc;
  se c9(c9_r, c9_d, c9_ts, c9_tc);
  initial begin
    c9_r = 1'b0;
    c9_d = 1'b0;
    c9_ts = 1'bx;
    c9_tc = 1'b1;
    #95 c9_d = 1'b1;
    #5 c9_r = 1'b1;
  end
  reg c10_r, c10_d, c10_ts, c10_tc;
  se c10(c10_r, c10_d, c10_ts, c10_tc);
  initial begin
    c10_r = 1'b0;
    c10_d = 1'b0;
    c10_ts = 1'b1;
    c10_tc = 1'bx;
    #95 c10_d = 1'b1;
    #5 c10_r = 1'b1;
  end
  reg c11_r, c11_d, c11_ts, c11_tc, c11_en;
  sa c11(c11_r, c11_d, c11_en, c11_ts, c11_tc);
  initial begin
    c11_r = 1'b0;
    c11_d = 1'b0;
    c11_ts = 1'b1;
    c11_tc = 1'b1;
    c11_en = 1'b0;
    #95 c11_d = 1'b1;
    #5 c11_r = 1'b1;
  end
  reg c12_r, c12_d, c12_ts, c12_tc, c12_en;
  sa c12(c12_r, c12_d, c12_en, c12_ts, c12_tc);
  initial begin
    c12_r = 1'b0;
    c12_d = 1'b0;
    c12_ts = 1'b1;
    c12_tc = 1'b1;
    c12_en = 1'b0;
    #100 c12_r = 1'b1;
    #2 c12_d = 1'b1;
  end
  reg c13_r, c13_d, c13_ts, c13_tc, c13_en;
  sa c13(c13_r, c13_d, c13_en, c13_ts, c13_tc);
  initial begin
    c13_r = 1'b0;
    c13_d = 1'b0;
    c13_ts = 1'b1;
    c13_tc = 1'b1;
    c13_en = 1'b1;
    #100 c13_r = 1'b1;
    #2 c13_d = 1'b1;
  end
  reg c14_r, c14_d, c14_ts, c14_tc;
  rc c14(c14_r, c14_d, c14_ts, c14_tc);
  initial begin
    c14_r = 1'b0;
    c14_d = 1'b0;
    c14_ts = 1'b1;
    c14_tc = 1'b1;
    #97 c14_d = 1'b1;
    #3 c14_r = 1'b1;
  end
  reg c15_r, c15_d, c15_ts, c15_tc;
  rc c15(c15_r, c15_d, c15_ts, c15_tc);
  initial begin
    c15_r = 1'b0;
    c15_d = 1'b0;
    c15_ts = 1'b1;
    c15_tc = 1'b0;
    #97 c15_d = 1'b1;
    #3 c15_r = 1'b1;
  end
  reg c16_r, c16_d, c16_ts, c16_tc;
  rc c16(c16_r, c16_d, c16_ts, c16_tc);
  initial begin
    c16_r = 1'b0;
    c16_d = 1'b0;
    c16_ts = 1'b0;
    c16_tc = 1'b1;
    #100 c16_r = 1'b1;
    #3 c16_d = 1'b1;
  end
  reg c17_r, c17_d, c17_ts;
  sn c17(c17_r, c17_d, c17_ts);
  initial begin
    c17_r = 1'b0;
    c17_d = 1'b0;
    c17_ts = 1'b0;
    #100 c17_r = 1'b1;
    #2 c17_ts = 1'b1;
    #4 c17_d = 1'b1;
  end
  reg c18_r, c18_d, c18_ts;
  sn c18(c18_r, c18_d, c18_ts);
  initial begin
    c18_r = 1'b0;
    c18_d = 1'b0;
    c18_ts = 1'b1;
    #100 c18_r = 1'b1;
    #2 c18_ts = 1'b0;
    #4 c18_d = 1'b1;
  end
  initial begin
    #300;
    $display("c1 %0d@%0d c2 %0d@%0d c3 %0d@%0d c4 %0d@%0d c5 %0d@%0d c6 %0d@%0d",
             c1.count, c1.last, c2.count, c2.last, c3.count, c3.last, c4.count, c4.last, c5.count, c5.last, c6.count, c6.last);
    $display("c7 %0d@%0d c8 %0d@%0d c9 %0d@%0d c10 %0d@%0d c11 %0d@%0d c12 %0d@%0d",
             c7.count, c7.last, c8.count, c8.last, c9.count, c9.last, c10.count, c10.last, c11.count, c11.last, c12.count, c12.last);
    $display("c13 %0d@%0d c14 %0d@%0d c15 %0d@%0d c16 %0d@%0d c17 %0d@%0d c18 %0d@%0d",
             c13.count, c13.last, c14.count, c14.last, c15.count, c15.last, c16.count, c16.last, c17.count, c17.last, c18.count, c18.last);
    $finish;
  end
endmodule
