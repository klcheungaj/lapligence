// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/nested_task_fork_static_event.sv
module tb;
    bit observed;

    task automatic sample_event_source(output bit seen);
        static bit static_source;
        seen = 1'b0;
        static_source = 1'b0;
        fork
            begin
                #1 static_source = 1'b1;
            end
            begin
                @(tb.sample_event_source.static_source);
                seen = 1'b1;
            end
        join
    endtask

    initial begin
        sample_event_source(observed);
        #1;
        if (!observed) $fatal(1, "static event not observed");
        $display("static-event=%b", observed);
        $finish(0);
    end
endmodule
