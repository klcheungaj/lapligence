// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/fork_group_order.sv
module tb;
    integer started = 0;
    integer completed = 0;
    event release_children;
    initial begin
        for (int i = 0; i < 4096; i++) begin
            automatic int child_id = i;
            fork
                begin
                    if (child_id != started) $fatal(1, "fork start order");
                    started++;
                    @release_children;
                    completed++;
                end
            join_none
        end
        if (started != 0) $fatal(1, "early fork start");
        #1;
        if (started != 4096 || completed != 0) $fatal(1, "fork parking");
        ->release_children;
        wait fork;
        if (completed != 4096) $fatal(1, "incomplete wait fork");
        for (int i = 0; i < 4096; i++) begin
            fork
                begin
                    @release_children;
                    $fatal(1, "disabled child resumed");
                end
            join_none
        end
        #1;
        disable fork;
        wait fork;
        ->release_children;
        #1;
        $display("PASS fork_group_order started=%0d completed=%0d", started, completed);
        $finish(0);
    end
endmodule
