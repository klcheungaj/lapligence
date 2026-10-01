// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/scheduler_wake_order.sv
// Signal subscribers retain llg's reverse-registration order within Active;
// the resulting #0 controls retain FIFO issue order within Inactive.
module tb;
    bit pulse = 0;
    bit left = 0;
    bit right = 0;
    integer active_order = 0;
    integer inactive_order = 0;
    integer multi_wakes = 0;
    for (genvar i = 0; i < 8; i++) begin : subscribers
        initial begin
            @(posedge pulse);
            active_order = active_order * 10 + i;
            #0;
            inactive_order = inactive_order * 10 + i;
        end
    end
    initial begin
        @(left or right or left or pulse);
        multi_wakes++;
        @(left or right or pulse);
        multi_wakes++;
    end
    initial begin
        #1 pulse = 1;
        left = 1;
        right = 1;
        #1;
        $display("order active=%0d inactive=%0d multi=%0d", active_order, inactive_order, multi_wakes);
        left = 0;
        right = 0;
        #1;
        $display("multi final=%0d", multi_wakes);
        $finish(0);
    end
endmodule
