// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h25_reject_controls.sv
// IEEE 1800-2009 16.13.14: reject_on and sync_reject_on fail a pending
// assertion attempt at their respective asynchronous/sampled control point.
module tb;
    logic clk;
    logic first;
    logic second;
    logic async_reject;
    logic sync_reject;

    property delayed_check;
        @(posedge clk) first |-> ##1 second;
    endproperty

    async_reject_cover: cover property (reject_on(async_reject) delayed_check);
    sync_reject_cover: cover property (
        @(posedge clk) sync_reject_on(sync_reject) delayed_check
    );

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        async_reject = 1'b0;
        sync_reject = 1'b0;
        #1 first = 1'b1;
        #1 clk = 1'b1;
        #1 async_reject = 1'b1;
        #1 begin
            clk = 1'b0;
            async_reject = 1'b0;
            sync_reject = 1'b1;
        end
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
