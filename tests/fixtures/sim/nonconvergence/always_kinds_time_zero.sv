// llg-test-fixture: tests/fixtures/sim/nonconvergence/always_kinds_time_zero.sv
// IEEE 1800-2009 §9.2.2.2 and §9.2.2.3: always_comb and always_latch have
// distinct implicit-sensitivity and time-zero behavior.
module tb;
    logic en = 1'b0;
    logic data = 1'b1;
    logic comb;
    logic latched = 1'b0;

    always_comb comb = data;
    always_latch if (en) latched = data;

    initial begin
        #0;
        $display("initial comb=%0d latch=%0d", comb, latched);
        en = 1'b1;
        data = 1'b0;
        #0;
        $display("updated comb=%0d latch=%0d", comb, latched);
        $finish(0);
    end
endmodule
