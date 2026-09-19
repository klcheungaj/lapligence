// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_05/owner_allocation_plateau.sv
// G1-05 owner_allocation_plateau: repeated equal-size wide owner construction
// and release must stay bounded. The standalone tracked-allocation benchmark
// (tests/runtime_value_storage) measures the exact live plateau; this fixture
// exercises the same shape through the emitted model in both optimizer modes.
module tb;
    logic [129:0] accumulator;
    integer i;

    initial begin
        accumulator = 130'd0;
        for (i = 0; i < 20000; i = i + 1) begin
            logic [129:0] temporary;
            temporary = (accumulator + 130'd3) + 130'd4;
            accumulator = temporary;
        end
        $display("%0d", accumulator);
        $finish(0);
    end
endmodule
