// RTL-012 negative: IEEE 1800-2009 10.3.4 excludes supply0/supply1 from
// continuous-assignment drive strengths (IEEE 1364-2001 6.1.4 omits them).
module tb;
  supply0 s;
  assign (weak1, weak0) s = 1'b1;
  initial begin #1 $display("%b", s); $finish(0); end
endmodule
