// Decision S99-D3: $assertoff stops new procedural instances from being
// queued, but instances already queued or matured still begin their
// attempts. $assertkill clears the pending instances and also the matured
// instances still waiting for their leading clock.
//
// IEEE 1800-2009 20.11 (SystemVerilog-1800-2009.txt L35050-35052) for
// $assertoff: "In the case of a pending procedural assertion instance (see
// 16.15.6), currently queued instances are not flushed and may still mature,
// though no new instances may be queued until the $asserton." L35054-35057
// for $assertkill: "abort execution of any currently executing specified
// assertions and then stop the checking of all specified assertions until a
// subsequent $asserton. This also flushes any queued pending reports of
// deferred assertions (see 16.4) or pending procedural assertion instances
// (see 16.15.6) that have not yet matured." The text does not say what
// happens to a matured instance still waiting for its clock; llg reading:
// $assertoff lets it begin, $assertkill discards it.
// k1 is queued at every posedge clk (5, 15, ...) and begins at the next
// posedge clk2 (10, 30, 50, 70). The instance from 15 begins at 30 although
// $assertoff ran at 17; none is queued at 25; the instances from 35 and 45
// are discarded by $assertkill at 47; those from 55 and 65 begin at 70.
`timescale 1ns/1ns
module tb;
  logic clk = 0, clk2 = 0;
  logic a = 1;
  always #5 clk = ~clk;
  always #10 clk2 = ~clk2;
  always @(posedge clk) begin
    k1: assert property (@(posedge clk2) a) $display("%0t k1 pass", $time);
  end
  initial begin
    #17 $assertoff(0, tb.k1);
    #15 $asserton(0, tb.k1);
    #15 $assertkill(0, tb.k1);
    #1 $asserton(0, tb.k1);
    #24 $finish;
  end
endmodule
