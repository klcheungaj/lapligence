// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h25_abort_variants.sv
// IEEE 1800-2009 16.13.14: synchronous accept_on resolves a bounded
// sequence attempt in the sampled assertion control domain.
module tb;
    logic clk;
    logic first;
    logic second;
    logic sync_accept;

    property delayed_check;
        @(posedge clk) first |-> ##1 second;
    endproperty

    sync_accept_assert: assert property (@(posedge clk) sync_accept_on(sync_accept) delayed_check)
        $display("H25_SYNC_ACCEPT");

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        sync_accept = 1'b0;
        #1 first = 1'b1;
        #1 clk = 1'b1;
        #1;
        #1 begin
            clk = 1'b0;
            sync_accept = 1'b1;
        end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            first = 1'b0;
            second = 1'b1;
            sync_accept = 1'b0;
        end
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
