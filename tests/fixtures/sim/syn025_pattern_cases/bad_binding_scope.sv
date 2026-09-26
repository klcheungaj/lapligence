// llg-test-fixture: tests/fixtures/sim/syn025_pattern_cases/bad_binding_scope.sv
module tb;
    logic [7:0] value;
    initial begin
        value = 8'h5a;
        case (value) matches
            .local_value: value = local_value;
        endcase
        $display("%h", local_value);
    end
endmodule
