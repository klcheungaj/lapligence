// llg-test-fixture: R05 must not reject same-type aliases without port edges.
`timescale 1ns/1ns
module tb;
    uwire a, b;
    reg d;
    alias a = b;
    assign a = d;
    initial begin
        d = 1;
        #1 $display("alias=%b%b", a, b);
        d = 0;
        #1 $display("alias=%b%b", a, b);
        $finish(0);
    end
endmodule
