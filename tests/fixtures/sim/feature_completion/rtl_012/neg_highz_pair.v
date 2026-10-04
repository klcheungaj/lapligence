// RTL-012 negative: (highz0, highz1) is illegal (IEEE 1364-2001 6.1.4, 7.1.2).
module tb;
  wire w;
  reg a;
  buf (highz1, highz0) b(w, a);
  initial begin #1 $display("%b", w); $finish(0); end
endmodule
