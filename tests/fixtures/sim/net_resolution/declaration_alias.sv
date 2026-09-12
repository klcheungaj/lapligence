// llg-test-fixture: tests/fixtures/sim/net_resolution/declaration_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire a = 1'b1;
    wire b;
    alias a = b;

    initial begin
        #1 $display("CHECK: declaration=%b%b", a, b);
        $finish(0);
    end
endmodule
