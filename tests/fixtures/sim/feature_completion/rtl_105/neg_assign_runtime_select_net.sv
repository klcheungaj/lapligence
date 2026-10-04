// RTL-105: a net_lvalue takes only constant selects (IEEE 1800-2009 10.3,
// A.8.5); the runtime-select admission is for variables only.
module tb;
    wire [7:0] w;
    logic x;
    int i;
    assign w[i] = x;
    initial begin #1 $display("%b", w); $finish(0); end
endmodule
