// llg-test-fixture: tests/fixtures/sim/function/timed_task_fork_joins.sv
module tb;
    logic first_done;
    logic second_done;

    task automatic run_forks(output logic first, output logic second);
        fork
            begin
                #1;
                first_done = 1'b1;
            end
            begin
                #2;
                second_done = 1'b1;
            end
        join_any
        first = first_done;
        wait fork;
        second = second_done;
    endtask

    initial begin
        logic first;
        logic second;
        first_done = 1'b0;
        second_done = 1'b0;
        run_forks(first, second);
        $display("fork result=%b%b t=%0t", first, second, $time);
        $finish;
    end
endmodule
