// RTL-105: a uwire formal still cannot be a bidirectional pass-switch
// terminal (IEEE 1800-2009 6.6.2).
module sw(inout uwire p);
    wire q;
    tran t(p, q);
endmodule
module tb;
    wire w;
    sw u(w);
    initial begin #1 $display("%b", w); $finish(0); end
endmodule
