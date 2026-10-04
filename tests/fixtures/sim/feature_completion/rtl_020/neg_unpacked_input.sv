// IEEE 1800-2009 29.3.1, 29.8: an unpacked array cannot connect to a scalar
// UDP terminal of a single instance.
primitive p(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        1 ? : 1;
    endtable
endprimitive

module tb;
    logic cells [0:1];
    logic b;
    wire y;
    p u(y, cells, b);
    initial begin
        b = 0;
        #1 $display("%b", y);
        $finish(0);
    end
endmodule
