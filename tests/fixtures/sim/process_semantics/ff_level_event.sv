// llg-test-fixture: tests/fixtures/sim/process_semantics/ff_level_event.sv
// IEEE 1800-2009 §9.2.2.4: one event control is required, but its event
// expression may be a legal level-sensitive any-change expression.
module tb;
    logic clk;
    logic q;

    always_ff @(clk) q <= 1'b1;

    initial begin
        clk = 1'b0;
        #1 clk = 1'b1;
        #1 $display("q=%b", q);
        $finish;
    end
endmodule
