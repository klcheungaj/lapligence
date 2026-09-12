// llg-test-fixture: tests/fixtures/sim/net_resolution/true_net_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire a, b;
    alias a = b;
    assign a = 1'b1;

    initial begin
        #1 $display("CHECK: initial=%b%b", a, b);
        force b = 1'b0;
        #1 $display("CHECK: forced=%b%b", a, b);
        release b;
        #1 $display("CHECK: released=%b%b", a, b);
        $finish(0);
    end
endmodule
