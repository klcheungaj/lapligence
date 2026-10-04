// RTL-012 negative: a supply net declaration assignment cannot carry a drive
// strength (IEEE 1364-2001 6.1.4).
module tb;
  supply1 (weak0, weak1) s = 1'b0;
  initial begin #1 $display("%b", s); $finish(0); end
endmodule
