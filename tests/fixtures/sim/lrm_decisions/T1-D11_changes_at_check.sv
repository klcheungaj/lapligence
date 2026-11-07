// Decision T1-D11: waiters see a change at a check, not at each write. A
// write updates the object at once, but waiters, clocking events and
// assertion clocks see it only at a check, which compares the value with the
// value at the previous check, so only the first and the last value count. A
// check runs when the writing process suspends or ends, after the NBA region
// (one batch for all processes), and at print and random calls. Updates that
// mature from an earlier time slot commit as a batch of their own.
//
// IEEE 1800-2009 4.5 (SystemVerilog-1800-2009.txt L3410-3411): "update the
// modified object; schedule evaluation event for any process sensitive to the
// object;" 4.9.3 L3529-3530: "the process performs the assignment to the
// left-hand side and enables any events based upon the update of the
// left-hand side." 9.4.2 L11772-11773: "A negedge shall be detected on the
// transition from 1 to x, z, or 0 ... A posedge shall be detected on the
// transition from 0 to x, z, or 1". 9.4.2.3 L11967: "This type of expression
// is evaluated when a changes and not when enable changes." Read strictly,
// every one of these wakes on each write; llg departs from them on purpose,
// so that code such as `q <= 0; if (c) q <= 1;` gives no false edges.
//
// Expected output, by step:
// - 10: `a = 1; a = 0;` from 0 wakes nothing.
// - 20: x, 0, 1 is a posedge and not a negedge.
// - 30: `q <= 1` is a posedge; at 31 `q <= 0; q <= 1;` on q holding 1 fires
//   no edge, and neither do the same two updates from two processes.
// - 40: the default-then-override idioms in an `always @(posedge clk)` and an
//   `always @(clk or sel)` give no false edges on q and gclk.
// - 50: `b = 1` and `b = 0` from two processes in one region: the first
//   process's check wakes the waiter, which then reads 0.
// - 60: a print between the writes is a check; the waiter reads 0.
// - 70: so is a random call.
// - 80: `@(posedge k iff en)` with `k = 1; en = 1;` reads en at the check.
// - 90: a bit, an array cell, a real and an `always @*` see no glitch.
// - 100: delayed updates maturing together are one batch.
// - 110: a matured update and an update of this slot are two batches.
// - 120: an assertion clock sees no glitch.
`timescale 1ns / 1ns
module tb;
  // Two-state storage starts at its default, so no time-0 change is seen
  // (T0-D1 differs between simulators otherwise).
  bit a, b, c, d, e, k, en, s, clk, sel, q, gclk;
  logic x;
  bit [7:0] v;
  bit [7:0] m [0:3];
  bit [7:0] seen;
  real r;
  int comb_runs;
  int seed;

  always @(a) $display("%0t any a=%b", $time, a);
  always @(posedge x) $display("%0t posedge x=%b", $time, x);
  always @(negedge x) $display("%0t negedge x=%b", $time, x);
  always @(posedge q) $display("%0t posedge q", $time);
  always @(negedge q) $display("%0t negedge q", $time);
  always @(posedge gclk) $display("%0t posedge gclk", $time);
  always @(negedge gclk) $display("%0t negedge gclk", $time);
  always @(b) $display("%0t any b=%b", $time, b);
  always @(c) $display("%0t any c=%b", $time, c);
  always @(posedge k iff en) $display("%0t posedge k iff en", $time);
  always @(v[0]) $display("%0t v[0]=%b", $time, v[0]);
  always @(m[1]) $display("%0t m[1]=%0d", $time, m[1]);
  always @(r) $display("%0t r=%f", $time, r);
  always @* seen = m[2] + v;
  always @(seen) comb_runs = comb_runs + 1;
  always @(d) $display("%0t any d=%b", $time, d);
  always @(e) $display("%0t any e=%b", $time, e);
  assert property (@(posedge s) 0) else $display("%0t clock s", $time);

  always @(posedge clk) begin
    q <= 0;
    if (sel) q <= 1;
  end
  always @(clk or sel) begin
    gclk = 0;
    if (sel) gclk = clk;
  end

  initial begin
    #10 $display("-- 10"); a = 1; a = 0;
    #10 $display("-- 20"); x = 0; x = 1;
    #10 $display("-- 30");
    #10 $display("-- 40"); sel = 1; #1 clk = 1; #1 clk = 0; #1 clk = 1; #1 clk = 0;
    #6 $display("-- 50");
    #10 $display("-- 60"); c = 1; $display("  mid"); c = 0;
    #10 $display("-- 70"); c = 1; seed = $urandom; c = 0;
    #10 $display("-- 80"); k = 1; en = 1;
    #10 $display("-- 90 comb_runs=%0d", comb_runs);
    v[0] = 1; v[0] = 0; m[1] = 5; m[1] = 0; m[2] = 7; m[2] = 0; r = 1.5; r = 0.0;
    #1 $display("   comb_runs=%0d", comb_runs);
    #9 $display("-- 100");
    #10 $display("-- 110");
    #10 $display("-- 120"); s = 1; s = 0;
    #10 $finish(0);
  end
  // Nonblocking steps, kept apart from the blocking ones above.
  initial begin
    #30 q <= 1;
    #1 q <= 0; q <= 1;
    #69 d <= #5 1; d <= #5 0;
    #10 e <= #5 1;
    #5 e <= 0;
  end
  initial #31 q <= 0;
  initial #31 q <= 1;
  initial #50 b = 1;
  initial #50 b = 0;
endmodule
