// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_02/coverage_reachable_pattern.sv
// A reached tagged pattern-matching case must be rejected with its location
// until tagged pattern lowering is implemented.
module tb;
    typedef union tagged {
        void invalid;
        logic [1:0] valid;
    } choice_t;
    choice_t x;
    logic [3:0] o;

    always_comb begin
        o = 4'd0;
        case (x) matches
            tagged invalid: o = 4'd1;
            default: o = 4'd0;
        endcase
    end
endmodule
