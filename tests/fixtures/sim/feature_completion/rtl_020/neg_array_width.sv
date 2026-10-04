// IEEE 1364-2001 7.1.5; IEEE 1800-2009 28.3.6: an instance-array connection
// must be one terminal wide or exactly one terminal per instance wide.
primitive p(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        1 ? : 1;
    endtable
endprimitive

module tb;
    logic [1:0] a, b;
    wire [2:0] y;
    p u[1:0] (y, a, b);
    initial begin
        a = 0; b = 0;
        #1 $display("%b", y);
        $finish(0);
    end
endmodule
