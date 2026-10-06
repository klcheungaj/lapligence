// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/action_retention.sv
// IEEE 1800-2009 16.14/16.9.3: every assert pass action (including vacuous
// successes) and every cover match action runs as its own Reactive process,
// and `$past(count, 3)`/`$rose` read a bounded sampled history. Completed
// actions and old samples are released while simulation continues, so the
// process statistics printed by `$finish(2)` do not depend on `CYCLES.
`ifndef CYCLES
`define CYCLES 50
`endif
module tb;
    logic clk = 1'b0;
    logic [7:0] count = 8'd0;
    int passes = 0;
    int fails = 0;
    int covers = 0;

    always #1 clk = ~clk;
    always @(posedge clk) count <= count + 8'd1;

    step: assert property (@(posedge clk) count >= 8'd3 |-> count == $past(count, 3) + 8'd3)
        passes++;
    else
        fails++;
    rising: cover property (@(posedge clk) $rose(count[0]))
        covers++;

    initial begin
        repeat (`CYCLES) @(posedge clk);
        #1;
        $display("passes=%0d fails=%0d covers=%0d", passes, fails, covers);
        $finish(2);
    end
endmodule
