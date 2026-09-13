// llg-test-fixture: tests/fixtures/sim/fork_lifecycle/join_lifecycle.sv
// IEEE 1800-2009 sections 9.3.2 and 9.6.1: join_none children start when the
// parent first blocks, join_any resumes at the first child completion, and
// wait fork observes the live groups owned by the calling process.
module tb;
    integer value;
    integer any_value;

    initial begin
        value = 0;
        fork
            begin
                $display("none child sees=%0d t=%0t", value, $time);
                #1 value = 1;
            end
            begin
                #2 value = 2;
            end
        join_none
        value = 5;
        $display("none parent before block=%0d t=%0t", value, $time);
        #0;
        wait fork;
        $display("none complete=%0d t=%0t", value, $time);

        fork
            begin
                $display("terminate child t=%0t", $time);
            end
        join_none
        $display("terminate parent t=%0t", $time);
    end

    initial begin
        #3;
        fork
            begin #1 any_value = 1; end
            begin #2 any_value = 2; end
        join_any
        $display("any first=%0d t=%0t", any_value, $time);
        wait fork;
        $display("any complete=%0d t=%0t", any_value, $time);
    end

    initial begin
        #6 $finish(0);
    end
endmodule
