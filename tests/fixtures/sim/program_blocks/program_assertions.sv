// llg-test-fixture: tests/fixtures/sim/program_blocks/program_assertions.sv
// IEEE 1800-2009 A.1.7 (non_port_program_item) and 24.3.1: concurrent
// assertions are legal program members with the same scheduling as in a
// module. They sample in Preponed, evaluate in Observed, run their actions in
// Reactive, and neither keep a program alive nor stop the implicit $finish.
program prg(input logic clk);
    logic a = 1'b0;
    logic pclk = 1'b0;

    // Design clock: `a` becomes 1 through a Re-NBA update at 5, so the tick
    // at 5 samples 0 and the ticks at 15 and 25 sample 1.
    pa: cover property (@(posedge clk) a) $display("%0d pa a=%b", $time, a);

    // Unlabeled, with a named action block that declares a variable.
    assert property (@(negedge clk) !a) else begin : fact
        int k;
        k = 7;
        $display("%0d fail k=%0d", $time, k);
    end

    // Clocked by a program variable written in Re-NBA at 12: the scheduler
    // returns to the Observed region to evaluate it (24.3.1).
    pc: cover property (@(posedge pclk) a) $display("%0d pc a=%b", $time, a);

    initial begin
        @(posedge clk) a <= 1'b1;
        #7 pclk <= 1'b1;
        #21 $display("%0d prog done", $time);
    end
endprogram

module tb;
    logic clk = 1'b0;
    always #5 clk = ~clk;
    prg p0(clk);
endmodule
