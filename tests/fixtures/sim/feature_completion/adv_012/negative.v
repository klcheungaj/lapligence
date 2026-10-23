// ADV-012 negative: annotated $setuphold limits with negative timing
// checks enabled (--neg-timing-checks) settle and solve the delayed signal
// delays again, as the same limits declared in the source do (IEEE
// 1800-2009 31.9.1, 32.4.1). `h` declares setup -2 and hold 5; `a` declares
// 1 and 1 and is annotated with -2 and 5; `z` is annotated with -3 and 4,
// whose window cannot report. Oracles in readme.md.
`timescale 1ns/1ps

module FFH (input clk, input d, output reg q);
  reg n = 1'b0;
  wire dclk, dd;
  always @(posedge dclk) q <= dd;
  specify
    $setuphold(posedge clk, d, -2, 5, n, , , dclk, dd);
  endspecify
endmodule

module FFA (input clk, input d, output reg q);
  reg n = 1'b0;
  wire dclk, dd;
  always @(posedge dclk) q <= dd;
  specify
    $setuphold(posedge clk, d, 1, 1, n, , , dclk, dd);
  endspecify
endmodule

module tb;
  reg clk = 1'b0, d = 1'b0;
  wire qh, qa, qz;
  FFH h (clk, d, qh);
  FFA a (clk, d, qa);
  FFA z (clk, d, qz);
  initial begin
    $sdf_annotate("negative.sdf");
    #50 $monitor("%0t dclk=%b%b%b dd=%b%b%b q=%b%b%b n=%b%b%b", $realtime,
                 h.dclk, a.dclk, z.dclk, h.dd, a.dd, z.dd, qh, qa, qz, h.n, a.n, z.n);
    // Data 4 after the clock: inside the window (2, 5) after it.
    #50 clk = 1'b1;
    #4 d = 1'b1;
    #6 clk = 1'b0;
    // Data 1 before the clock: outside the window.
    #90 clk = 1'b1;
    #1 d = 1'b0;
    #9 clk = 1'b0;
    // Data 1 after the clock: outside the window, and captured.
    #90 clk = 1'b1;
    #1 d = 1'b1;
    #9 clk = 1'b0;
    #90 $finish;
  end
endmodule
