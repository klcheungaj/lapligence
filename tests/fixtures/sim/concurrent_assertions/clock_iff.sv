// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/clock_iff.sv
// IEEE 1800-2009 9.4.2.3, 16.5, 16.17 a): an explicit or inherited
// `@(posedge clk iff en)` clocks an assertion only on edges where `en` is
// true at the edge (its current value, not the sampled one).
// - `explicit` is clocked `@(posedge clk iff en)`: `a ##1 b` skips the edge
//   at time 20 (en = 0) and matches at 30.
// - `late`: g rises in the time step of the edge at 10 but after it (#0), so
//   10 is gated off; 20 and 30 tick (g toggles only after the edge at 20); at
//   40 g falls in the edge's time step just before clk rises, so 40 is off.
// - `sampled` reads `$past(v)` on the gated clock: only gated ticks count.
module tb;
    logic clk = 1'b0;
    logic en = 1'b0;
    logic g = 1'b0;
    logic a = 1'b0;
    logic b = 1'b0;
    logic [3:0] v = 4'd0;

    explicit: cover sequence (@(posedge clk iff en) a ##1 b)
        $display("EXPLICIT %0t", $time);
    late: cover property (@(posedge clk iff g) 1'b1)
        $display("LATE %0t", $time);
    sampled: cover property (@(posedge clk iff en) v == 4'd3 && $past(v) == 4'd1)
        $display("SAMPLED %0t", $time);

    initial begin
        en = 1'b1;
        a = 1'b1;
        v = 4'd1;
        #10 clk = 1'b1;          // 10: en=1, a=1, v=1
        #0 g = 1'b1;
        #5 clk = 1'b0;
        en = 1'b0;
        a = 1'b0;
        b = 1'b1;
        v = 4'd2;
        #5 clk = 1'b1;           // 20: en=0 (no tick for explicit/sampled)
        #0 g = 1'b0;
        #0 g = 1'b1;
        #5 clk = 1'b0;
        en = 1'b1;
        v = 4'd3;
        #5 clk = 1'b1;           // 30: en=1, b=1, v=3, $past(v) on ticks = 1
        #5 clk = 1'b0;
        b = 1'b0;
        #5 g = 1'b0;
        clk = 1'b1;              // 40: g fell before the edge
        #5 $finish(0);
    end
endmodule
