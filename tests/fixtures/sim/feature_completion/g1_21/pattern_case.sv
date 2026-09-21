// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_21/pattern_case.sv
// G1-21 control: tagged pattern-case forms use the owned tag guard and keep
// ordinary case-item selection when the active arm does not match.
module tb;
    typedef union tagged packed {
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

    initial begin
        x = 2'b01;
        #1 $display("o=%0d", o);
        $finish(0);
    end
endmodule
