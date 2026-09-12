// llg-test-fixture: tests/fixtures/sim/net_resolution/inout_alias.sv
// LRM: IEEE 1800-2009 10.11.
module child(inout wire p);
    wire q;
    alias p = q;
    assign q = 1'b1;
endmodule

module tb;
    wire bus;
    child c(bus);

    initial begin
        #1 $display("CHECK: inout=%b%b", bus, c.p);
        $finish(0);
    end
endmodule
