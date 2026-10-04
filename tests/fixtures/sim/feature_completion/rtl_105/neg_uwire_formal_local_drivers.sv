// RTL-105: two drivers of an `inout uwire` formal inside one module
// (IEEE 1800-2009 6.6.2).
module drv(inout uwire p);
    assign p = 1'b0;
    assign p = 1'b1;
endmodule
module tb;
    wire w;
    drv u(w);
    initial begin #1 $display("%b", w); $finish(0); end
endmodule
