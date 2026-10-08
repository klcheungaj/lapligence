// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/exhaustive.sv
// SIM-037 A01: every sequence below starts at tick 1 of each of the 1024
// two-signal traces of length 5 (go at tick 0) and prints each match end;
// pending attempts are killed between traces. The expected match sets come
// from the test-side Annex F trace interpreter in sim_037.rs.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic kill = 1'b1;
  logic a = 1'b0;
  logic b = 1'b0;
  int trace = 0;
  int pos = 0;

  s00: cover property (@(posedge clk) disable iff (kill) go ##1 a |-> 1'b1) $display("00 %0d %0d", trace, pos);
  s01: cover property (@(posedge clk) disable iff (kill) go ##1 (a ##1 b) |-> 1'b1) $display("01 %0d %0d", trace, pos);
  s02: cover property (@(posedge clk) disable iff (kill) go ##1 (a ##0 b) |-> 1'b1) $display("02 %0d %0d", trace, pos);
  s03: cover property (@(posedge clk) disable iff (kill) go ##1 (a ##[1:3] b) |-> 1'b1) $display("03 %0d %0d", trace, pos);
  s04: cover property (@(posedge clk) disable iff (kill) go ##1 (a ##[2:$] b) |-> 1'b1) $display("04 %0d %0d", trace, pos);
  s05: cover property (@(posedge clk) disable iff (kill) go ##1 (##[1:2] b) |-> 1'b1) $display("05 %0d %0d", trace, pos);
  s06: cover property (@(posedge clk) disable iff (kill) go ##1 ((a)[*0] ##1 b) |-> 1'b1) $display("06 %0d %0d", trace, pos);
  s07: cover property (@(posedge clk) disable iff (kill) go ##1 ((a)[*0:2] ##0 b) |-> 1'b1) $display("07 %0d %0d", trace, pos);
  s08: cover property (@(posedge clk) disable iff (kill) go ##1 (a)[*1:3] |-> 1'b1) $display("08 %0d %0d", trace, pos);
  s09: cover property (@(posedge clk) disable iff (kill) go ##1 (a)[*0:$] |-> 1'b1) $display("09 %0d %0d", trace, pos);
  s10: cover property (@(posedge clk) disable iff (kill) go ##1 ((a ##1 b))[*1:2] |-> 1'b1) $display("10 %0d %0d", trace, pos);
  s11: cover property (@(posedge clk) disable iff (kill) go ##1 (((a)[*0:1] ##1 b))[*2:$] |-> 1'b1) $display("11 %0d %0d", trace, pos);
  s12: cover property (@(posedge clk) disable iff (kill) go ##1 (((a)[*0] or b))[*2:3] |-> 1'b1) $display("12 %0d %0d", trace, pos);
  s13: cover property (@(posedge clk) disable iff (kill) go ##1 b[->2] |-> 1'b1) $display("13 %0d %0d", trace, pos);
  s14: cover property (@(posedge clk) disable iff (kill) go ##1 (a[->1:2] ##1 b) |-> 1'b1) $display("14 %0d %0d", trace, pos);
  s15: cover property (@(posedge clk) disable iff (kill) go ##1 (b[=1] ##1 a) |-> 1'b1) $display("15 %0d %0d", trace, pos);
  s16: cover property (@(posedge clk) disable iff (kill) go ##1 a[=0:2] |-> 1'b1) $display("16 %0d %0d", trace, pos);
  s17: cover property (@(posedge clk) disable iff (kill) go ##1 ((a ##1 b) or (b)[*2]) |-> 1'b1) $display("17 %0d %0d", trace, pos);
  s18: cover property (@(posedge clk) disable iff (kill) go ##1 ((a ##[1:2] b) and (b)[*1:3]) |-> 1'b1) $display("18 %0d %0d", trace, pos);
  s19: cover property (@(posedge clk) disable iff (kill) go ##1 ((a)[*0:1] and (b ##1 b)) |-> 1'b1) $display("19 %0d %0d", trace, pos);
  s20: cover property (@(posedge clk) disable iff (kill) go ##1 ((a)[*0:2] and (b)[*0:1]) |-> 1'b1) $display("20 %0d %0d", trace, pos);
  s21: cover property (@(posedge clk) disable iff (kill) go ##1 ((a)[*1:4] intersect (b ##[1:$] a)) |-> 1'b1) $display("21 %0d %0d", trace, pos);
  s22: cover property (@(posedge clk) disable iff (kill) go ##1 (((1)[*0:3] ##1 b) intersect (a)[*2:3]) |-> 1'b1) $display("22 %0d %0d", trace, pos);
  s23: cover property (@(posedge clk) disable iff (kill) go ##1 ((a)[*0:2] intersect (b)[*0:2]) |-> 1'b1) $display("23 %0d %0d", trace, pos);
  s24: cover property (@(posedge clk) disable iff (kill) go ##1 (a throughout (b ##[1:2] b)) |-> 1'b1) $display("24 %0d %0d", trace, pos);
  s25: cover property (@(posedge clk) disable iff (kill) go ##1 (!b throughout (a)[*0:2]) |-> 1'b1) $display("25 %0d %0d", trace, pos);
  s26: cover property (@(posedge clk) disable iff (kill) go ##1 (b within (a ##[2:3] a)) |-> 1'b1) $display("26 %0d %0d", trace, pos);
  s27: cover property (@(posedge clk) disable iff (kill) go ##1 ((a ##1 a) within (1)[*1:4]) |-> 1'b1) $display("27 %0d %0d", trace, pos);
  s28: cover property (@(posedge clk) disable iff (kill) go ##1 first_match((a ##[1:3] b)) |-> 1'b1) $display("28 %0d %0d", trace, pos);
  s29: cover property (@(posedge clk) disable iff (kill) go ##1 first_match(((a)[*1:3] or (b ##1 b))) |-> 1'b1) $display("29 %0d %0d", trace, pos);
  s30: cover property (@(posedge clk) disable iff (kill) go ##1 first_match(((a)[*1:$] and (b ##[1:2] a))) |-> 1'b1) $display("30 %0d %0d", trace, pos);
  s31: cover property (@(posedge clk) disable iff (kill) go ##1 (((a)[*1:2] intersect (b)[*1:2]) or first_match((a within (b)[*2:3]))) |-> 1'b1) $display("31 %0d %0d", trace, pos);
  s32: cover property (@(posedge clk) disable iff (kill) go ##1 ((a and (b ##1 b)) ##1 ((a)[*1:2] intersect (b ##[0:1] a))) |-> 1'b1) $display("32 %0d %0d", trace, pos);
  s33: cover property (@(posedge clk) disable iff (kill) go ##1 ((a and (b ##1 a)))[*1:2] |-> 1'b1) $display("33 %0d %0d", trace, pos);
  s34: cover property (@(posedge clk) disable iff (kill) go ##1 ((a)[*0:1] ##1 (b)[*0:1]) |-> 1'b1) $display("34 %0d %0d", trace, pos);
  s35: cover property (@(posedge clk) disable iff (kill) go ##1 (##[0:1] (a)[*0:1]) |-> 1'b1) $display("35 %0d %0d", trace, pos);

  task automatic tick();
    #5 clk = 1'b1;
    #5 clk = 1'b0;
  endtask

  initial begin
    for (int tr = 0; tr < 1024; tr++) begin
      trace = tr;
      pos = 0;
      go = 1'b1;
      kill = 1'b0;
      a = 1'b0;
      b = 1'b0;
      tick();
      go = 1'b0;
      for (int k = 0; k < 5; k++) begin
        pos = k + 1;
        a = tr[2 * k];
        b = tr[2 * k + 1];
        tick();
      end
      kill = 1'b1;
      tick();
    end
    $finish;
  end
endmodule
