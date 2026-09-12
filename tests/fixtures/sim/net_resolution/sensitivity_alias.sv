// llg-test-fixture: tests/fixtures/sim/net_resolution/sensitivity_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire a, b;
    logic drive;
    alias a = b;
    assign b = drive;

    always @(a) $display("CHECK: alias_event=%b", a);

    initial begin
        drive = 1'b0;
        #1 drive = 1'b1;
        #1 $finish(0);
    end
endmodule
