// RTL-011: a uwire net-array cell collapsed through an inout array port keeps
// the single-driver rule (IEEE 1800-2009 6.6.2).
module child(inout wire [3:0] c [0:1]); assign c[0] = 4'h3; endmodule
module tb;
    uwire [3:0] a [0:1];
    child u(a);
    assign a[0] = 4'h1;
    initial begin #1 $display("%h", a[0]); $finish(0); end
endmodule
