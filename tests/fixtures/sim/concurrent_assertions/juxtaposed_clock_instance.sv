// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/juxtaposed_clock_instance.sv
// IEEE 1800-2009 16.14.3: a named property's own clock nullifies the
// juxtaposed call-site clock (formerly rejected as conflicting), so the
// assertion samples value at the negedges 10 (0) and 20 (1), not at the
// posedges 5 (1) and 15 (0).
module tb;
    logic clk = 1'b0;
    logic value = 1'b1;

    property negedge_property(x);
        @(negedge clk) x;
    endproperty

    juxtaposed: assert property (@(posedge clk) negedge_property(value))
        $display("PASS"); else $display("FAIL");

    initial begin
        #5 clk = 1'b1;
        #2 value = 1'b0;
        #3 clk = 1'b0;
        #5 clk = 1'b1;
        #2 value = 1'b1;
        #3 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
