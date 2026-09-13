// llg-test-fixture: tests/fixtures/sim/partial_features/event_activation_capture.sv
// IEEE 1800-2009 §§9.4.2, 9.4.2.3, 13.3 and 13.5: evaluated event controls
// retain each automatic task activation's locals and formals independently.
module tb;
    logic gate = 0;
    logic qualifier = 0;
    int wakes = 0;

    task automatic wait_for_gate(input logic expected);
        logic local_value;
        local_value = expected;
        @(posedge (local_value && gate) iff qualifier);
        $display("task=%0d", local_value);
        wakes = wakes + 1;
    endtask

    initial begin
        fork
            wait_for_gate(0);
            wait_for_gate(1);
        join_none
        #1 gate = 1;
        #1 qualifier = 1;
        #1 gate = 0;
        #1 gate = 1;
        #1 $display("wakes=%0d", wakes);
        $finish(0);
    end
endmodule
