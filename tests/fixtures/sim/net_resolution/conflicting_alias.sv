// llg-test-fixture: tests/fixtures/sim/net_resolution/conflicting_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire a, b;
    alias a = b;
    assign a = 1'b0;
    assign b = 1'b1;

    initial begin
        #1 $display("CHECK: conflict=%b%b", a, b);
        $finish(0);
    end
endmodule
