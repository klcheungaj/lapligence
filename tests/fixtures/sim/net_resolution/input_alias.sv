// llg-test-fixture: tests/fixtures/sim/net_resolution/input_alias.sv
// LRM: IEEE 1800-2009 10.11.
module child(input wire p, output wire out);
    wire q;
    alias p = q;
    assign out = q;
endmodule

module tb;
    wire bus, observed;
    assign bus = 1'b1;
    child c(bus, observed);

    initial begin
        #1 $display("CHECK: input=%b%b", bus, observed);
        $finish(0);
    end
endmodule
