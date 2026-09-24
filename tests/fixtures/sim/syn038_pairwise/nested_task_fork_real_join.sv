// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/nested_task_fork_real_join.sv
module tb;
    real observed;

    task automatic sample(output real seen);
        real shared;
        shared = 1.5;
        fork
            begin #1 shared = 2.5; end
            begin #2 seen = shared; end
        join
    endtask

    initial begin
        sample(observed);
        if (observed != 2.5) $fatal(1, "real joined capture mismatch");
        $display("real=%0.1f", observed);
        $finish(0);
    end
endmodule
