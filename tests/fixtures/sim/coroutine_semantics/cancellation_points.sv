// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/cancellation_points.sv
// A running process observes cancellation where it can change: after a resume
// point, after its own `disable`, and after a call that may disable. Nothing
// after that point in a cancelled block runs, on every control-flow path.
module tb;
    integer loop_steps = 0;
    integer after_loop = 0;
    integer after_inner = 0;
    integer after_outer = 0;
    integer other_branch = 0;
    integer branch_after = 0;
    integer copied = 5;
    integer after_call = 0;
    integer after_caller = 0;

    task automatic stop_caller(output integer result);
        result = 9;
        disable caller;
    endtask

    // Another process disables `outer` while this one waits inside a loop in
    // the nested block `inner`: the loop's break path, the tail of `inner`
    // and the tail of `outer` must all be skipped.
    initial begin
        begin : outer
            begin : inner
                forever begin
                    #2;
                    loop_steps++;
                    if (loop_steps > 100) break;
                end
                after_loop = 1;
            end
            after_inner = 1;
        end
        after_outer = 1;
    end
    initial #3 disable outer;

    // A disable in one branch leaves the block at once; the other branch
    // continues normally.
    initial begin
        begin : branchy
            for (int i = 0; i < 2; i++) begin
                if (i == 1) begin
                    disable branchy;
                    branch_after = 1;
                end else begin
                    other_branch++;
                end
            end
            branch_after = 2;
        end
    end

    // A task without timing controls disables its caller's block: its output
    // is not copied back and the rest of the block is skipped.
    initial begin
        begin : caller
            stop_caller(copied);
            after_call = 1;
        end
        after_caller = 1;
    end

    initial begin
        #10;
        if (loop_steps != 1 || after_loop != 0 || after_inner != 0 || after_outer != 1)
            $fatal(1, "nested block cancellation mismatch");
        if (other_branch != 1 || branch_after != 0)
            $fatal(1, "branch disable mismatch");
        if (copied != 5 || after_call != 0 || after_caller != 1)
            $fatal(1, "callee disable mismatch");
        $display("PASS cancellation_points steps=%0d outer=%0d branch=%0d copied=%0d caller=%0d",
            loop_steps, after_outer, other_branch, copied, after_caller);
        $finish(0);
    end
endmodule
