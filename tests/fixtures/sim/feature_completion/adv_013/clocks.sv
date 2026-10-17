// ADV-013 A02: every clocking event form clocks a concurrent assertion
// (16.14, 14.13, 14.14, 16.15.7): value-change, `edge`, `or`-list,
// clocking-block, bit-select and gated non-leading clocks tick at most once
// per time step (16.5), `$global_clock` names the global clocking event, and
// `$inferred_clock`/`$inferred_disable` defaults take the clock and disable
// condition at the instance. Expected lines are derived in readme.md.
module dmod(input logic c1, c2, a);
  default clocking @(posedge c1 or posedge c2); endclocking
  k9: assert property (a) $display("k9 P %0t", $time); else $display("k9 F %0t", $time);
endmodule

module imod_c2(input logic c2, a, b);
  default clocking @(posedge c2); endclocking
  property q(x, clk = $inferred_clock);
    @clk x |=> b;
  endproperty
  i1: assert property (q(a)) $display("i1 P %0t", $time); else $display("i1 F %0t", $time);
endmodule

// Negedge clocks read tb's variables directly: an input port net would see
// its own X->0 initialization at time 0 as a negedge.
module imod_nc1;
  default clocking @(negedge tb.c1); endclocking
  property p_multiclock(clkw, clkx = $inferred_clock, clky, w, x, y, z);
    @clkw w ##1 @clkx x |=> @clky y ##1 z;
  endproperty
  i2: assert property (p_multiclock(negedge tb.c2, , posedge tb.c1, tb.a, tb.b, tb.d, tb.e))
    $display("i2 P %0t", $time); else $display("i2 F %0t", $time);
endmodule

module imod_c1(input logic c1, c2, a, b, rst);
  default clocking @(posedge c1); endclocking
  default disable iff rst;
  property q2(x, clk = $inferred_clock);
    @clk x;
  endproperty
  property p(x, r = $inferred_disable);
    disable iff (r) x |=> b;
  endproperty
  i3: assert property (a |=> @(posedge c2) q2(b)) $display("i3 P %0t", $time); else $display("i3 F %0t", $time);
  i4: assert property (p(a)) $display("i4 P %0t", $time); else $display("i4 F %0t", $time);
endmodule

module tb;
  logic c1 = 0, c2 = 0, a = 0, b = 0, d = 0, e = 0, rst = 0;
  logic [1:0] cv;
  assign cv = {c2, c1};
  initial begin
    fork
      begin #10 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; end
      begin #20 c2 = 1; #5 c2 = 0; #5 c2 = 1; #3 c2 = 0; #12 c2 = 1; #3 c2 = 0; #12 c2 = 1; #3 c2 = 0; #27 c2 = 1; #2 c2 = 0; #8 c2 = 1; #2 c2 = 0; end
      begin #1 a = 1; #11 a = 0; #10 a = 1; #19 a = 0; #20 a = 1; end
      begin #2 b = 1; #19 b = 0; #10 b = 1; #20 b = 0; #7 b = 1; #33 b = 0; end
      begin #3 d = 1; #41 d = 0; #22 d = 1; end
      begin #26 e = 1; #10 e = 0; end
      begin #40 rst = 1; #2 rst = 0; end
    join
    #13 $finish;
  end

  clocking cb @(posedge c2); endclocking
  global clocking gc @(posedge c1 or negedge c2); endclocking

  property q3(x, clk = $inferred_clock);
    @clk x |=> b;
  endproperty
  property p5(x, r = $inferred_disable);
    @(posedge c1) disable iff (r) x |=> b;
  endproperty

  k1: assert property (@(c1) a) $display("k1 P %0t", $time); else $display("k1 F %0t", $time);
  k2: assert property (@(edge c1) a) $display("k2 P %0t", $time); else $display("k2 F %0t", $time);
  k3: assert property (@(posedge c1 or posedge c2) a) $display("k3 P %0t", $time); else $display("k3 F %0t", $time);
  k4: assert property (@(cb) b) $display("k4 P %0t", $time); else $display("k4 F %0t", $time);
  k5: assert property (@(posedge cv[1]) d) $display("k5 P %0t", $time); else $display("k5 F %0t", $time);
  k6: assert property (@(posedge c1) a ##1 @(posedge c2 iff d) b) $display("k6 P %0t", $time); else $display("k6 F %0t", $time);
  k7: assert property (@(posedge c1 iff d) a ##1 @(posedge c1) b) $display("k7 P %0t", $time); else $display("k7 F %0t", $time);
  k8: assert property (@(posedge c1) a |=> @(c2) b) $display("k8 P %0t", $time); else $display("k8 F %0t", $time);
  g1: assert property (@($global_clock) a |=> b) $display("g1 P %0t", $time); else $display("g1 F %0t", $time);
  i5: assert property (p5(a)) $display("i5 P %0t", $time); else $display("i5 F %0t", $time);
  always @(posedge c2) begin
    i6: assert property (q3(a)) $display("i6 P %0t", $time); else $display("i6 F %0t", $time);
  end
  initial repeat (4) @($global_clock) $display("g3 %0t", $time);

  dmod dm(c1, c2, a);
  imod_c2 ic2(c2, a, b);
  imod_nc1 inc1();
  imod_c1 ic1(c1, c2, a, b, rst);
endmodule
