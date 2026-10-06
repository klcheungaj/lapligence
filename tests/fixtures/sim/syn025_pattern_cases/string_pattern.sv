// llg-test-fixture: tests/fixtures/sim/syn025_pattern_cases/string_pattern.sv
// IEEE 1800-2009 12.6.1: a whole-value binding of a string selector matches
// unconditionally and binds a copy of the selector within its item only.
module tb;
    string value;
    logic result;
    initial begin
        value = "hello";
        case (value) matches
            .bound &&& bound == "bye": result = 0;
            .bound: begin
                result = 1;
                $display("%s %0d", bound, bound.len());
            end
        endcase
        $display("%b", result);
        $finish(0);
    end
endmodule
