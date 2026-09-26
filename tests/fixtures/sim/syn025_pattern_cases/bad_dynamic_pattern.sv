// llg-test-fixture: tests/fixtures/sim/syn025_pattern_cases/bad_dynamic_pattern.sv
module tb;
    string value;
    logic result;
    initial begin
        value = "hello";
        case (value) matches
            .bound: result = 1;
        endcase
        $display("%b", result);
    end
endmodule
