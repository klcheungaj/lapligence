// ADV-013 A02: multiclocked sequences and properties (16.14.1-16.14.3):
// ##1 and ##0 between clocks, overlapping and non-overlapping implication,
// clock flow across |=> and parentheses and into named instances, the
// adjointness examples of 16.14.3, and juxtaposed clocking events. Clock c1
// and c2 ticks coincide at 30 and 90. Expected lines are derived in
// readme.md.
`timescale 1ns/1ps
module tb;
  logic c1 = 0, c2 = 0, a = 0, b = 0, d = 0, e = 0;
  initial begin
    fork
      begin #10 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; end
      begin #20 c2 = 1; #5 c2 = 0; #5 c2 = 1; #3 c2 = 0; #12 c2 = 1; #3 c2 = 0; #12 c2 = 1; #3 c2 = 0; #27 c2 = 1; #2 c2 = 0; #8 c2 = 1; #2 c2 = 0; end
      begin #1 a = 1; #11 a = 0; #10 a = 1; #19 a = 0; #20 a = 1; end
      begin #2 b = 1; #19 b = 0; #10 b = 1; #20 b = 0; #7 b = 1; #33 b = 0; end
      begin #3 d = 1; #41 d = 0; #22 d = 1; end
      begin #26 e = 1; #10 e = 0; end
    join
    #13 $finish;
  end

  sequence s2;
    @(posedge c2) b;
  endsequence

  f1: assert property (@(posedge c1) a ##1 @(posedge c2) b) $display("f1 P %0t", $time); else $display("f1 F %0t", $time);
  f2: assert property (@(posedge c1) a ##0 @(posedge c2) b) $display("f2 P %0t", $time); else $display("f2 F %0t", $time);
  f3: assert property (@(posedge c1) a |-> @(posedge c2) b) $display("f3 P %0t", $time); else $display("f3 F %0t", $time);
  f4: assert property (@(posedge c1) a |=> @(posedge c2) b) $display("f4 P %0t", $time); else $display("f4 F %0t", $time);
  f5: assert property (@(posedge c1) a |=> d ##1 @(posedge c2) b) $display("f5 P %0t", $time); else $display("f5 F %0t", $time);
  f5r: assert property (@(posedge c1) a |=> @(posedge c1) d ##1 @(posedge c2) b) $display("f5r P %0t", $time); else $display("f5r F %0t", $time);
  f6: assert property (@(posedge c1) a ##1 d |=> @(posedge c2) b) $display("f6 P %0t", $time); else $display("f6 F %0t", $time);
  f6r: assert property (@(posedge c1) a |=> d |=> @(posedge c2) b) $display("f6r P %0t", $time); else $display("f6r F %0t", $time);
  f7: assert property (@(posedge c1) a ##0 d |=> @(posedge c2) b) $display("f7 P %0t", $time); else $display("f7 F %0t", $time);
  f7r: assert property (@(posedge c1) a |-> d |=> @(posedge c2) b) $display("f7r P %0t", $time); else $display("f7r F %0t", $time);
  f8: assert property (@(posedge c1) a ##1 (d ##1 @(posedge c2) b) |=> e) $display("f8 P %0t", $time); else $display("f8 F %0t", $time);
  f9: assert property (@(posedge c1) a |=> (d ##1 @(posedge c2) b) and (b ##1 d)) $display("f9 P %0t", $time); else $display("f9 F %0t", $time);
  f10: assert property (@(posedge c2) @(posedge c1) a) $display("f10 P %0t", $time); else $display("f10 F %0t", $time);
  f11: assert property (@(posedge c1) s2) $display("f11 P %0t", $time); else $display("f11 F %0t", $time);
  f12: assert property (@(posedge c1) a ##1 (@(posedge c2) b) ##1 d) $display("f12 P %0t", $time); else $display("f12 F %0t", $time);
  f13: assert property (@(posedge c1) a ##1 @(posedge c2) b |=> d) $display("f13 P %0t", $time); else $display("f13 F %0t", $time);
endmodule
