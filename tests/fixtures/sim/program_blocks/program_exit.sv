// llg-test-fixture: tests/fixtures/sim/program_blocks/program_exit.sv
// IEEE 1800-2009 24.3 and 24.7: $exit terminates only its originating
// program. The other program's initial remains live until time 1.
program exit_program;
    initial begin
        $display("exit start");
        #0;
        $display("exit before");
        $exit;
        $display("exit after");
    end
    final $display("exit final");
endprogram

program other_program;
    initial begin
        $display("other start");
        fork
            begin
                $display("other child start");
                #10 $display("other child after");
            end
        join_none
        #1 $display("other parent after");
    end
    final $display("other final");
endprogram

module tb;
    exit_program exit0();
    other_program other0();
    final $display("module final");
endmodule
