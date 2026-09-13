// llg-test-fixture: tests/fixtures/sim/fork_lifecycle/nested_disable.sv
// IEEE 1800-2009 sections 9.3.2 and 9.6.3: disabling a process cancels its
// active child and a nested child that was retained by join_none.
module tb;
    integer grandchild;

    initial begin
        grandchild = 0;
        fork
            begin
                fork
                    begin
                        #1 grandchild = 1;
                    end
                join_none
                #10 grandchild = 2;
            end
        join_none
        #0;
        disable fork;
        #2;
        $display("nested disable value=%0d t=%0t", grandchild, $time);
        $finish(0);
    end
endmodule
