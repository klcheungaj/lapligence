// Decision T1-D11: a `force` expression sees a change of what it reads at a
// check, not at each write. The check that sees a change evaluates the
// expression again at once and updates the forced target; a read of the
// target before that check still returns the value of the last check.
//
// IEEE 1800-2009 4.9.2 (SystemVerilog-1800-2009.txt L3515-3517): "A
// procedural continuous assignment (which is the assign or force statement;
// see 10.6) corresponds to a process that is sensitive to the source elements
// in the expression. When the value of the expression changes, it causes an
// active update event to be added to the event region, using current values
// to determine the target." The text does not say when, within the writing
// process, the change is seen; llg follows industry-standard behaviour.
//
// Expected output, by step:
// - 1: the force sets y from x to 0.
// - 2: `a = 1; c = y; a = 0;` reads y before any check: c is 0, and the
//   final value of a is no change, so the waiter on y does not wake.
// - 3: a comparison right after the write still reads the old forced value
//   of u; at 4 the check at the end of the turn has updated it.
// - 5, 6: the same for a forced real.
// - 7: `a = 1; #0` is a check: y follows and its waiter wakes; the read
//   after it sees y = 1.
`timescale 1ns / 1ns
module tb;
  logic a, y, c;
  logic [7:0] v, u;
  real r, ry;
  always @(y) $display("%0t waiter y=%b", $time, y);
  initial begin
    a = 1'b0;
    v = 8'd0;
    r = 0.0;
    #1 force y = a;
    force u = v + 8'd1;
    force ry = r * 2.0;
    #1 a = 1'b1; c = y; a = 1'b0;
    $display("%0t c=%b y=%b", $time, c, y);
    #1 v = 8'd7;
    if (u == 8'd8) $display("%0t u follows", $time);
    else $display("%0t u stale", $time);
    #1 $display("%0t u=%0d", $time, u);
    #1 r = 1.5;
    if (ry == 3.0) $display("%0t ry follows", $time);
    else $display("%0t ry stale", $time);
    #1 $display("%0t ry=%0.1f", $time, ry);
    #1 a = 1'b1; #0 c = y;
    $display("%0t c=%b", $time, c);
    #1 $finish;
  end
endmodule
