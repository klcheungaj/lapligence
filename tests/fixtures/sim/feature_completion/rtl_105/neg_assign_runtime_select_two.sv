// RTL-105: two continuous assignments whose longest static prefixes overlap
// (IEEE 1800-2009 6.5, 11.5.3); `a[i]` covers every element of `a`.
module tb;
    logic [7:0] a [0:3];
    logic [7:0] x, y;
    int i;
    assign a[i] = x;
    assign a[0] = y;
    initial begin #1 $display("%h", a[0]); $finish(0); end
endmodule
