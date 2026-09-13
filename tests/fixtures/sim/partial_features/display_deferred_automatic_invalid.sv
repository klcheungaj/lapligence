// llg-test-fixture: tests/fixtures/sim/partial_features/display_deferred_automatic_invalid.sv
module tb;
    task automatic emit;
        integer local_value;
        begin
            local_value = 1;
            $strobe("%0d", local_value);
        end
    endtask

    initial begin
        emit();
        #1 $finish;
    end
endmodule
