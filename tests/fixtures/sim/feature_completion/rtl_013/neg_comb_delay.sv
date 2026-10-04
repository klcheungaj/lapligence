// IEEE 1800-2009 9.2.2.2: always_comb shall not contain blocking timing controls.
module tb;
  logic a, b;
  always_comb begin #1 b = a; end
  initial $finish(0);
endmodule
