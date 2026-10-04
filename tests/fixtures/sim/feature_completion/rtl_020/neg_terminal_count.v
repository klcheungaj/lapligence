// IEEE 1364-2001 8.6; IEEE 1800-2009 29.8: an instance connects exactly one
// output and one terminal per declared input.
primitive p(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        1 ? : 1;
    endtable
endprimitive

module tb;
    reg a, b, c;
    wire y;
    p u(y, a, b, c);
    initial begin
        a = 0; b = 0; c = 0;
        #1 $display("%b", y);
        $finish(0);
    end
endmodule
