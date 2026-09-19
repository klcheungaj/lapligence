// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_21/case_four_state.sv
// G1-21 case_four_state_checks: case, casez, casex and case inside keep their
// distinct X/Z wildcard rules on one selector. IEEE 1800-2009 12.5.1.
module tb;
    logic [3:0] sel;
    logic [3:0] o_exact;
    logic [3:0] o_casez;
    logic [3:0] o_casex;
    logic [3:0] o_inside;

    always_comb
        case (sel)
            4'b10xz: o_exact = 4'd1;
            default: o_exact = 4'd0;
        endcase

    always_comb
        casez (sel)
            4'b10??: o_casez = 4'd2;
            default: o_casez = 4'd0;
        endcase

    always_comb
        casex (sel)
            4'b1x0x: o_casex = 4'd3;
            default: o_casex = 4'd0;
        endcase

    always_comb
        case (sel) inside
            [4'b1000 : 4'b1011]: o_inside = 4'd4;
            4'b11??: o_inside = 4'd5;
            default: o_inside = 4'd0;
        endcase

    initial begin
        sel = 4'b10xz; #1 $display("a %0d %0d %0d %0d", o_exact, o_casez, o_casex, o_inside);
        sel = 4'b1101; #1 $display("b %0d %0d %0d %0d", o_exact, o_casez, o_casex, o_inside);
        sel = 4'b1010; #1 $display("c %0d %0d %0d %0d", o_exact, o_casez, o_casex, o_inside);
        $finish(0);
    end
endmodule
