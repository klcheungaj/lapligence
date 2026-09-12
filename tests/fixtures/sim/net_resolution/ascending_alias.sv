// llg-test-fixture: tests/fixtures/sim/net_resolution/ascending_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire [0:3] a, b;
    alias a = b;
    assign a = 4'b1010;

    initial begin
        #1 $display("CHECK: ascending=%b/%b", a, b);
        $finish(0);
    end
endmodule
