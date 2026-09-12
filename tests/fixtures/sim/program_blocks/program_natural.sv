// llg-test-fixture: tests/fixtures/sim/program_blocks/program_natural.sv
// IEEE 1800-2009 §24.3: multiple program instances complete naturally and
// their final procedures run after the implicit simulation finish.
program first(output logic result);
    initial begin
        $display("first program");
        result <= 1'b1;
    end
    final $display("first final");
endprogram

program second;
    initial begin
        $display("second program");
        fork
            begin
                $display("second child");
                #1 $display("second child done");
            end
        join_none
    end
    final $display("second final");
endprogram

module tb;
    logic result;
    first first0(.result(result));
    second second0();
    always @(result) $display("module saw natural nba=%0d", result);
    final $display("module final");
endmodule
