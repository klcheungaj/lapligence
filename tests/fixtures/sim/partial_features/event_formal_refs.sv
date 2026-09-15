// llg-test-fixture: tests/fixtures/sim/partial_features/event_formal_refs.sv
// IEEE 1800-2009 Sections 9.4.2, 13.4 and 13.5: live ref formals and legal
// const-ref function reads keep their caller dependencies across suspension.
module tb;
    logic first = 0;
    logic second = 0;
    int ref_wakes = 0;
    int function_wakes = 0;

    function automatic logic level(const ref logic source);
        level = source;
    endfunction

    task automatic wait_ref(ref logic source, input int tag);
        @(posedge source);
        ref_wakes = ref_wakes + tag;
    endtask

    task automatic wait_function(ref logic source, input int tag);
        @(posedge level(source));
        function_wakes = function_wakes + tag;
    endtask

    initial begin
        fork
            wait_ref(first, 1);
            wait_ref(second, 10);
            wait_function(first, 100);
            wait_function(second, 1000);
        join_none
        #1 first = 1;
        #1 second = 1;
        #1 $display("ref=%0d function=%0d", ref_wakes, function_wakes);
        $finish(0);
    end
endmodule
