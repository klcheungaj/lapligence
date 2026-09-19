// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_20/ff_conflicting_writer.sv
// G1-20 ff_illegal_timing_or_writer (negative): an always_ff cannot share
// storage with another procedural writer; both origins are reported.
module tb;
    logic clk;
    logic q;

    always_ff @(posedge clk) q <= 1'b0;
    always_comb q = 1'b1;

    initial begin
        clk = 1'b0;
    end
endmodule
