// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_21/pattern_case.sv
// G1-21 boundary: pattern-binding case forms are deferred to G3-01 and must be
// rejected rather than partially executed as an ordinary case.
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

    initial begin
        x = 2'b01;
        #1 $display("o=%0d", o);
        $finish(0);
    end
endmodule
