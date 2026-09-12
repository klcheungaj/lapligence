// llg-test-fixture: tests/fixtures/sim/net_resolution/force_alias_rhs.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire a, b;
    logic force_value;
    alias a = b;
    assign a = 1'b0;

    initial begin
        force_value = 1'b1;
        force b = force_value;
        #1 $display("CHECK: force_rhs=%b%b", a, b);
        force_value = 1'b0;
        #1 $display("CHECK: force_rhs=%b%b", a, b);
        release b;
        #1 $display("CHECK: force_rhs=%b%b", a, b);
        $finish(0);
    end
endmodule
