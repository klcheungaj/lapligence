// llg-test-fixture: alias observations share the forced value and preserve the
// winning driver strength after release. LRM: IEEE 1800-2009 10.11, 6.6.6.
module tb;
    wire p;
    wire q;

    alias p = q;

    assign (strong0, strong1) p = 1'b1;
    assign (weak0, weak1) p = 1'b0;

    initial begin
        #1;
        $display("CHECK: base=%b%b", p, q);
        force q = 1'b0;
        #1;
        $display("CHECK: forced=%b%b", p, q);
        release q;
        #1;
        $display("CHECK: released=%b%b", p, q);
        $finish(0);
    end
endmodule
