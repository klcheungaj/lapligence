// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/nested_task_fork_formal_event.sv
module tb;
    bit formal_seen;

    task sample_formal_event(input bit formal_source, output bit observed);
        observed = 1'b0;
        formal_source = 1'b0;
        fork
            begin
                #1;
                tb.sample_formal_event.formal_source = 1'b1;
            end
            begin
                @(tb.sample_formal_event.formal_source);
                observed = 1'b1;
            end
        join
    endtask

    initial begin
        sample_formal_event(1'b0, formal_seen);
        #1;
        if (!formal_seen) $fatal(1, "hierarchical formal event not observed");
        $display("formal-hier-event=%b", formal_seen);
        $finish(0);
    end
endmodule
