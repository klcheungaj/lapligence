// llg-test-fixture: tests/fixtures/sim/net_resolution/output_alias.sv
// LRM: IEEE 1800-2009 10.11.
module child(output wire p);
    wire q;
    alias p = q;
    assign q = 1'b1;
endmodule

module tb;
    wire observed;
    child c(observed);

    initial begin
        #1 $display("CHECK: output=%b%b", observed, c.p);
        $finish(0);
    end
endmodule
