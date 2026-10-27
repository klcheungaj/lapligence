// ADV-014 A01: global clocking future sampled value functions (IEEE
// 1800-2009 16.9.4) inside property operators and assertion forms: an if-else
// condition, a synchronous abort, an assertion that is the whole body of an
// initial procedure (one attempt), a procedural assertion with an inferred
// clock, and a property local input formal, which keeps its value from the
// attempt's start. Expected lines: readme.md.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, slow = 1'b0;
  logic a = 1'b0, b = 1'b0, c = 1'b0;
  global clocking gc @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  always #20 slow = ~slow;
  initial begin
    a = 1'b1;
    #22 b = 1'b1;
    #128 c = 1'b1;
    #22 $finish;
  end
  property p_local(local input logic x);
    $steady_gclk(a) |-> x;
  endproperty
  ifs: assert property (@(posedge clk) if ($future_gclk(b)) $steady_gclk(a) else $changing_gclk(a))
    else $display("%0t ifs fail", $time);
  syn: assert property (@(posedge clk) sync_accept_on(c) $steady_gclk(a) ##1 !a)
    $display("%0t syn pass", $time); else;
  initial assert property (@(posedge clk) ##2 $changing_gclk(b))
    $display("%0t ini pass", $time); else $display("%0t ini fail", $time);
  always @(posedge slow) begin
    prc: assert property ($steady_gclk(a)) $display("%0t prc pass", $time);
  end
  loc: assert property (@(posedge slow) p_local(b))
    $display("%0t loc pass", $time); else $display("%0t loc fail", $time);
endmodule
