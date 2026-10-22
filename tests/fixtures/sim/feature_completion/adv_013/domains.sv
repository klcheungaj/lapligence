// ADV-013 A03: local variables, disable iff, action blocks and end of
// simulation across clock domains: overlapping attempts each keep their own
// local values across c1 and c2 ticks, a property local gets one copy per
// semantic leading clock (16.10), asynchronous disables preempt attempts
// waiting in either domain, actions see both domains' updates of their time
// step, and strong sequence obligations still open at $finish report
// nothing, like weak ones (only s_eventually fails there, A13-D7). Same
// stimulus as flow.sv plus x, y, rst and rst2; expected lines are derived
// in readme.md.
`timescale 1ns/1ps
module tb;
  logic c1 = 0, c2 = 0, a = 0, b = 0, d = 0, e = 0, rst = 0, rst2 = 0;
  logic [2:0] x = 0, y = 0;
  int cnt1 = 0, cnt2 = 0;
  initial begin
    fork
      begin #10 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; end
      begin #20 c2 = 1; #5 c2 = 0; #5 c2 = 1; #3 c2 = 0; #12 c2 = 1; #3 c2 = 0; #12 c2 = 1; #3 c2 = 0; #27 c2 = 1; #2 c2 = 0; #8 c2 = 1; #2 c2 = 0; end
      begin #1 a = 1; #11 a = 0; #10 a = 1; #19 a = 0; #20 a = 1; end
      begin #2 b = 1; #19 b = 0; #10 b = 1; #20 b = 0; #7 b = 1; #33 b = 0; end
      begin #3 d = 1; #41 d = 0; #22 d = 1; end
      begin #26 e = 1; #10 e = 0; end
      begin #1 x = 1; #21 x = 2; #19 x = 3; #7 x = 4; #33 x = 5; end
      begin #25 y = 1; #30 y = 2; #40 y = 4; end
      begin #40 rst = 1; #2 rst = 0; #5 rst2 = 1; #1 rst2 = 0; end
    join
    #13 $finish;
  end
  always @(posedge c1) cnt1 <= cnt1 + 1;
  always @(posedge c2) cnt2 <= cnt2 + 1;

  sequence keep_x;
    logic [2:0] v;
    (1'b1, v = x) ##1 @(posedge c2) 1'b1 ##1 y == v;
  endsequence
  property x_then_y;
    logic [2:0] v;
    (a, v = x) |=> @(posedge c2) y == v - 3'd1;
  endproperty
  property two_copies;
    logic [2:0] v;
    (@(posedge c1) (1'b1, v = x) ##0 v == 3'd4) and (@(posedge c2) (1'b1, v = x) ##0 v == 3'd3);
  endproperty

  l1: assert property (@(posedge c1) keep_x)
    $display("l1 P %0t %0d %0d", $time, cnt1, cnt2); else $display("l1 F %0t %0d %0d", $time, cnt1, cnt2);
  l2: assert property (@(posedge c1) x_then_y)
    $display("l2 P %0t", $time); else $display("l2 F %0t", $time);
  l3: assert property (@(posedge c1) e |=> two_copies)
    $display("l3 P %0t", $time); else $display("l3 F %0t", $time);
  dl1: assert property (@(posedge c1) disable iff (rst) a |=> @(posedge c2) b ##1 @(posedge c1) d)
    $display("dl1 P %0t", $time); else $display("dl1 F %0t", $time);
  dl2: assert property (@(posedge c2) disable iff (rst2) b |=> @(posedge c1) d)
    $display("dl2 P %0t", $time); else $display("dl2 F %0t", $time);
  s1: assert property (@(posedge c1) a |=> strong(@(posedge c2) d ##1 d))
    $display("s1 P %0t", $time); else $display("s1 F %0t", $time);
  s2: assert property (@(posedge c1) a |=> weak(@(posedge c2) d ##1 d))
    $display("s2 P %0t", $time); else $display("s2 F %0t", $time);
endmodule
