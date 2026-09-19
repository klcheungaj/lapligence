// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_28/constant_context_runtime_read.sv
// A constant context that depends on runtime state is rejected during
// elaboration, before any model can be constructed.
module tb;
    logic [3:0] r;
    localparam int W = r + 1;

    initial begin
        $display("W=%0d", W);
        $finish(0);
    end
endmodule
