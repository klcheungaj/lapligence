// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/termination_depth.sv
// The default stop policy resumes the exact continuation two timing-task
// levels deep. A later finish in a plain function called by a timing task
// abandons every caller and runs finals exactly once.
module tb;
    integer stop_after = 0;
    integer finish_after = 0;
    integer final_count = 0;

    task automatic stop_l2;
        $display("stop before");
        $stop(0);
        stop_after = 1;
        $display("stop resumed");
    endtask
    task automatic stop_l1;
        stop_l2();
    endtask

    function automatic void finish_function;
        $display("finish before");
        $finish(0);
        finish_after = 1;
    endfunction
    task automatic timed_finish;
        #1;
        finish_function();
        finish_after = 2;
    endtask

    initial begin
        stop_l1();
        timed_finish();
        finish_after = 3;
    end

    final begin
        final_count++;
        $display("final count=%0d stop_after=%0d finish_after=%0d",
            final_count, stop_after, finish_after);
    end
endmodule
