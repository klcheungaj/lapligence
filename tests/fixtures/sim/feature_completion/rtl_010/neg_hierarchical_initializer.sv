// IEEE 1800-2009 6.5: a hierarchical continuous assignment to a variable that
// its own module also writes procedurally.
module child;
    logic [3:0] v;
    initial v = 4'h1;
endmodule
module tb;
    child c();
    logic [3:0] x = 4'h3;
    assign c.v = x;
endmodule
