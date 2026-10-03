// IEEE 1800-2009 6.5: a continuously assigned row overlaps a procedurally
// written cell of that row.
module tb;
    logic [3:0] m[0:1][0:2];
    logic [3:0] x = 4'h5;
    assign m[0] = '{x, x, x};
    initial m[0][1] = 4'h1;
endmodule
