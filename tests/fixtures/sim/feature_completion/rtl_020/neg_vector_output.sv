// IEEE 1800-2009 29.3.1, 29.8: the output terminal of a single UDP instance
// is scalar; a vector net is not a selected scalar.
primitive p(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        1 ? : 1;
    endtable
endprimitive

module tb;
    logic a, b;
    wire [1:0] y;
    p u(y, a, b);
    initial begin
        a = 0; b = 0;
        #1 $display("%b", y);
        $finish(0);
    end
endmodule
