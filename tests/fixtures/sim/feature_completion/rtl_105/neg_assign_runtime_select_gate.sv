// RTL-105: a gate output terminal is a net_lvalue with constant selects even
// when it names a variable (IEEE 1800-2009 28.3, A.3.3, A.8.5).
module tb;
    logic [1:0] g;
    logic x;
    int i;
    buf b(g[i], x);
    initial begin #1 $display("%b", g); $finish(0); end
endmodule
