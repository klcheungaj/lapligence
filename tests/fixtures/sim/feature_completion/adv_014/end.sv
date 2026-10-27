// ADV-014 A01: the end of simulation before the global clock tick an
// evaluation needs (IEEE 1800-2009 16.9.4, Annex F.6.3: the future value at
// the last letter of a finite word is undefined). The last global clock tick
// is 45 and the run ends at 47: an assertion clock tick at 45 or later is
// never evaluated, so its attempt neither begins nor reports; the attempts
// evaluated up to then settle by the ordinary end-of-simulation rule
// (decisions S38-D1, A13-D7, A14-D3). Expected lines: readme.md.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, s = 1'b0, r = 1'b0;
  global clocking gc @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  initial begin
    s = 1'b1;
    #47 $finish;
  end
  e1: assert property (@(posedge clk) $steady_gclk(s))
    $display("%0t e1 pass", $time); else $display("%0t e1 fail", $time);
  e2: assert property (@(posedge clk) s |-> s_eventually $rising_gclk(r))
    $display("%0t e2 pass", $time); else $display("%0t e2 fail", $time);
  e3: assert property (@(posedge clk) s |-> strong(##2 $steady_gclk(s)))
    $display("%0t e3 pass", $time); else $display("%0t e3 fail", $time);
  e4: assert property (@(negedge clk) $future_gclk(s))
    $display("%0t e4 pass", $time); else $display("%0t e4 fail", $time);
endmodule
