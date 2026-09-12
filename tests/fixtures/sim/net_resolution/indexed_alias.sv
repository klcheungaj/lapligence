// llg-test-fixture: tests/fixtures/sim/net_resolution/indexed_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire [7:0] a, b;
    alias a[7 -: 4] = b[3:0];
    assign a[7:4] = 4'b1010;

    initial begin
        #1 $display("CHECK: indexed=%b/%b", a[7:4], b[3:0]);
        $finish(0);
    end
endmodule
