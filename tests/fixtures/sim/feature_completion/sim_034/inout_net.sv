// SIM-034 A03: a clocking inout on a wire gets its own (strong1, strong0)
// driver, initialized to 'z, that resolves against the wire's other driver
// (IEEE 1800-2009 14.16). The oracle is in readme.md.
`timescale 1ns/1ns
module tb;
  bit clk = 0;
  wire [3:0] w;
  logic [3:0] other = 4'bzzzz;
  assign w = other;

  clocking cb @(posedge clk);
    inout w;
  endclocking

  // Posedges at 5, 15, 25, 35.
  always #5 clk = ~clk;
  always @(w) if ($time > 0) $display("%0t w=%b", $time, w);

  initial begin
    #1 other = 4'b0001;
    @(cb);
    cb.w <= 4'b0011;
    @(cb);
    $display("%0t sampled cb.w=%b", $time, cb.w);
    other = 4'bzzzz;
    @(cb);
    cb.w <= 4'bzz10;
    #12 $finish;
  end
endmodule
