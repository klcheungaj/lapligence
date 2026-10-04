// IEEE 1800-2009 9.2.2.2: always_comb shall not contain fork-join blocks.
module tb;
  logic a, b;
  always_comb begin fork b = a; join_none end
  initial $finish(0);
endmodule
