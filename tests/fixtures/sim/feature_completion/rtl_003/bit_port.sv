// SV2009 11.5, 23.2.2.3: a bit-selected reference retains a one-bit boundary.
module child(ref logic x);
 initial begin #1; x[0] = 1; x[-1] = 1; x[1] = 1; $display("bit %b %b", x[0], x[1]); end
endmodule
module tb;
 logic [3:0] value = 0;
 child c(value[2]);
 initial begin #2; $display("root %b", value); $finish(0); end
endmodule
