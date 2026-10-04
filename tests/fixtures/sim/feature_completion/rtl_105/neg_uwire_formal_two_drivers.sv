// RTL-105: an `inout uwire` formal collapses with the parent's wire, and the
// collapsed net keeps the single-driver rule (IEEE 1800-2009 6.6.2): one
// driver inside the child and one in the parent is an error.
module drv(inout uwire p);
    assign p = 1'b0;
endmodule
module tb;
    wire w;
    drv u(w);
    assign w = 1'b1;
    initial begin #1 $display("%b", w); $finish(0); end
endmodule
