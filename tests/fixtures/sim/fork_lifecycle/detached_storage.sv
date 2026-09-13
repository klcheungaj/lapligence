// llg-test-fixture: tests/fixtures/sim/fork_lifecycle/detached_storage.sv
// IEEE 1800-2009 sections 6.21, 9.3.2, and 9.6.1: detached packed captures
// retain their values, while a delayed NBA to persistent module storage is
// committed after the issuing child has completed.
module tb;
    logic [7:0] target;

    initial begin
        automatic integer captured;
        captured = 7;
        fork
            begin
                #1 $display("capture=%0d t=%0t", captured, $time);
            end
        join_none
        captured = 9;
        #0;
        wait fork;
        $display("capture parent=%0d t=%0t", captured, $time);
    end

    initial begin
        fork
            begin
                target <= #2 8'h5a;
            end
        join_none
    end

    initial begin
        #3;
        $display("queued target=%h t=%0t", target, $time);
        $finish(0);
    end
endmodule
