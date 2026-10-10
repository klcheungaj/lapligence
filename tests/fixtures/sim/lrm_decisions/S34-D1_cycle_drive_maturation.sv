// Decision S34-D1: a synchronous drive with a cycle delay does not block the
// issuing process. It keeps the value it computed when it executed and
// matures that many events of the target's own clocking block later; issued
// away from an event it counts from the next event. Drives that mature in the
// same Re-NBA region commit in the order they were issued, so the last issued
// value is the one driven.
//
// IEEE 1800-2009 14.16 (SystemVerilog-1800-2009.txt L20103-20108):
//   "The optional cycle_delay construct, appearing on the right-hand side of a
//   clocking_drive statement, is syntactically similar to an intra-assignment
//   delay in a nonblocking assignment. Like a nonblocking intra-assignment
//   delay, it shall not cause execution of the statement to block. The
//   right-hand side expression shall be evaluated immediately even when a
//   cycle_delay is present. However, updating of the target signal shall be
//   postponed for the specified number of cycles of the target clockvar's
//   clocking block, plus any clocking output skew specified for that clockvar."
// IEEE 1800-2009 14.16.2 (L20194-20195, L20244-20260):
//   "When more than one synchronous drive on the same clocking block output
//   (or inout) is scheduled to mature in the same Re-NBA region of the same
//   time step, the last value is the only value driven onto the output sig-
//   nal."
//   "##1;                       // Wait until cycle 1
//    cb.v <= expr1;             // Matures in cycle 1, v is assigned expr1
//    cb.v <= ##2 expr2;         // Matures in cycle 3
//    #1 cb.v <= ##2 expr3;      // Matures in cycle 3
//    ##1 cb.v <= ##1 expr4;     // Matures in cycle 3, v is assigned expr4"
//
// This case is the clause's example with a 10-unit clock (cycles 1-3 at 5,
// 15 and 25). llg blocked the issuing process for the cycle count before.
module tb;
  bit clk = 0;
  logic [3:0] v = 0;
  default clocking cb @(posedge clk);
    output v;
  endclocking
  always #5 clk = ~clk;
  always @(v) $display("%0d v=%0d", $time, v);
  initial begin
    ##1;
    cb.v <= 4'd1;
    cb.v <= ##2 4'd2;
    $display("%0d issued without blocking", $time);
    #1 cb.v <= ##2 4'd3;
    ##1 cb.v <= ##1 4'd4;
    #30 $finish;
  end
endmodule
