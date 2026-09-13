// llg-test-fixture: tests/fixtures/sim/process_semantics/legal_latch_ff.sv
// IEEE 1800-2009 §§9.2.2.3–9.2.2.4: legal latch and edge-triggered
// processes retain their distinct time-zero and event-control behavior.
module tb;
    logic en;
    logic data;
    logic latched;
    logic clk;
    logic q;

    always_latch if (en) latched = data;
    always_ff @(posedge clk) q <= data;

    initial begin
        en = 1'b0;
        data = 1'b0;
        clk = 1'b0;
        #1 $display("start latched=%b q=%b", latched, q);
        data = 1'b1;
        #1 $display("hold latched=%b q=%b", latched, q);
        en = 1'b1;
        #1 $display("open latched=%b q=%b", latched, q);
        clk = 1'b1;
        #1 $display("edge latched=%b q=%b", latched, q);
        data = 1'b0;
        #1 $display("data latched=%b q=%b", latched, q);
        $finish;
    end
endmodule
