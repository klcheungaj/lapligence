// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h25_controls.sv
// IEEE 1800-2009 16.13/16.13.14: bounded accept_on and conditional properties
// retain sampled values while an asynchronous abort resolves active attempts.
module tb;
    logic clk;
    logic first;
    logic second;
    logic abort_now;
    logic choose_first;

    property delayed_check;
        @(posedge clk) first |-> ##1 second;
    endproperty

    aborted: assert property (accept_on(abort_now) delayed_check)
        $display("H25_ABORT_ACCEPT");

    conditional: assert property (
        @(posedge clk) if (choose_first) first else second
    ) $display("H25_CONDITIONAL_PASS");

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        abort_now = 1'b0;
        choose_first = 1'b1;
        #1 first = 1'b1;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            abort_now = 1'b1;
        end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            abort_now = 1'b0;
            choose_first = 1'b0;
            first = 1'b0;
            second = 1'b1;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
