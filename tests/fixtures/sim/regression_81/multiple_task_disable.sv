// llg-test-fixture: tests/fixtures/sim/regression_81/multiple_task_disable.sv
// IEEE 1364-2001 section 11 / IEEE 1800-2009 section 9.6.2: a named task
// disable selects all active declaration/instance-keyed invocations.
module tb;
    integer completed, body;

    task work(input integer value);
        #5 body = body + value;
    endtask

    initial begin
        completed = 0;
        body = 0;
        fork
            begin work(1); completed = completed + 1; end
            begin work(2); completed = completed + 1; end
        join_none
        #1 disable work;
        #1 $display("completed=%0d body=%0d", completed, body);
        #10 $display("late=%0d body=%0d", completed, body);
        $finish(0);
    end
endmodule
