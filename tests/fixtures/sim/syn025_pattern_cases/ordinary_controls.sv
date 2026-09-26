// llg-test-fixture: tests/fixtures/sim/syn025_pattern_cases/ordinary_controls.sv
// IEEE 1800-2009 12.5.1, 12.5.4, 12.6.1: distinct case match rules.
module tb;
    logic [3:0] result;
    initial begin
        result = 0;
        case (4'b10x1)
            4'b10x1: result = 1;
            default: result = 2;
        endcase
        if (result !== 1) $fatal(1, "ordinary exact case");

        casez (4'b10z1)
            4'b1001: result = 3;
            default: result = 4;
        endcase
        if (result !== 3) $fatal(1, "ordinary selector Z");

        case (4'b1010) inside
            4'b1x1x: result = 5;
            default: result = 6;
        endcase
        if (result !== 5) $fatal(1, "inside item wildcard");

        case (4'b10x1) matches
            4'b10x1: result = 7;
            default: result = 8;
        endcase
        if (result !== 7) $fatal(1, "exact pattern case");

        casez (4'b10z1) matches
            4'b1001: result = 9;
            default: result = 10;
        endcase
        if (result !== 9) $fatal(1, "pattern casez selector Z");

        casez (4'b10x1) matches
            4'b1001: result = 11;
            default: result = 12;
        endcase
        if (result !== 12) $fatal(1, "pattern casez retains X");

        casex (4'b10x1) matches
            4'b1001: result = 13;
            default: result = 14;
        endcase
        if (result !== 13) $fatal(1, "pattern casex selector X");

        $display("ordinary_controls=pass");
        $finish(0);
    end
endmodule
