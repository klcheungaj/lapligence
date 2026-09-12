// llg-test-fixture: tests/fixtures/sim/program_blocks/program_exit_child.sv
// IEEE 1800-2009 §§24.3 and 24.7: `$exit` from a detached program child
// terminates its parent and every other program process without recursion.
program child_exit;
    initial begin
        fork
            begin
                $display("exit child start");
                #0;
                $exit;
            end
        join_none
        #10 $display("exit parent after");
    end
    final $display("exit child final");
endprogram

program survivor;
    initial begin
        $display("survivor start");
        #20 $display("survivor after");
    end
    final $display("survivor final");
endprogram

module tb;
    child_exit child0();
    survivor survivor0();
    final $display("module final");
endmodule
