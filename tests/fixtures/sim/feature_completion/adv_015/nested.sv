// ADV-015 A01: a static checker instantiated in a module with default
// clocking and default disable: $inferred_clock and $inferred_disable
// formals, a nested checker whose clock is inferred again, checker let,
// function and sequence declarations, a cover, a deferred assertion and
// final procedures (IEEE 1800-2009 17.2-17.5, 16.15.7).
`timescale 1ns / 1ns
checker inner(logic x, event ck = $inferred_clock);
  bit [7:0] hits = 8'd0;
  always @(ck) hits <= hits + (x ? 8'd1 : 8'd0);
  c_in: cover property (@ck x) $display("%0t %m cover x", $time);
  final $display("%m hits=%0d", hits);
endchecker

checker outer(logic a, logic b, event clock = $inferred_clock,
              logic reset = $inferred_disable);
  default clocking @clock; endclocking
  function automatic logic both(logic p, logic q);
    return p && q;
  endfunction
  let either = a || b;
  sequence s_ab;
    a ##1 b;
  endsequence
  logic [3:0] n = 4'd0;
  always @(clock) n <= n + (both(a, b) ? 4'd1 : 4'd0);
  inner i1(either);
  a_ab: assert property (disable iff (reset) a |=> b)
    else $display("%0t %m fail n=%0d", $time, n);
  c_ab: cover property (s_ab) $display("%0t %m cover s_ab", $time);
  d1: assert #0 (!(a && reset)) else $display("%0t %m deferred fail", $time);
  final $display("%m n=%0d", n);
endchecker

module tb;
  logic clk = 1'b0, rst = 1'b0, a = 1'b0, b = 1'b0;
  always #5 clk = ~clk;
  default clocking cb @(posedge clk); endclocking
  default disable iff rst;
  outer u(a, b);
  initial begin
    @(negedge clk) a = 1'b1;
    @(negedge clk) begin
      a = 1'b0;
      b = 1'b1;
    end
    @(negedge clk) begin
      a = 1'b1;
      b = 1'b0;
    end
    @(negedge clk) a = 1'b0;
    @(negedge clk) begin
      rst = 1'b1;
      a = 1'b1;
    end
    @(negedge clk) begin
      rst = 1'b0;
      a = 1'b0;
    end
    #20 $finish;
  end
endmodule
