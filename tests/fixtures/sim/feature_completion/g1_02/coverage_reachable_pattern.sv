// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_02/coverage_reachable_pattern.sv
// A reached pattern-matching case must be rejected with its location before a
// runnable model is built; it must never lower as an empty ordinary case.
module tb;
    logic [1:0] x;
    logic [3:0] o;

    always_comb begin
        o = 4'd0;
        case (x) matches
            2'b0?: o = 4'd1;
            default: o = 4'd0;
        endcase
    end
endmodule
