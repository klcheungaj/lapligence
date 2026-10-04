// IEEE 1800-2009 29.3.1, 29.8: a single UDP instance has scalar terminals.
// Selecting one bit of a vector is legal; a two-bit part-select is not.
primitive p(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        1 ? : 1;
    endtable
endprimitive

module tb;
    logic [3:0] v;
    logic b;
    wire y;
    p u(y, v[2:1], b);
    initial begin
        v = 0; b = 0;
        #1 $display("%b", y);
        $finish(0);
    end
endmodule
