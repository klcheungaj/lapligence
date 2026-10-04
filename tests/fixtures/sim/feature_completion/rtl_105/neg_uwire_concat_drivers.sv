// RTL-105: a concatenated uwire actual is not itself a driver, but the
// collapsed bit still allows one driver (IEEE 1800-2009 6.6.2): here the
// child drives the bit of `c` and the parent drives `c` too.
module hi(inout wire [1:0] p);
    assign p[1] = 1'b1;
endmodule
module tb;
    uwire c, f;
    hi u({c, f});
    assign c = 1'b0;
    initial begin #1 $display("%b%b", c, f); $finish(0); end
endmodule
