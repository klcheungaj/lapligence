// SV 10.6.2, 6.4: an unpacked array variable is not singular.
module tb;
  logic [3:0] ua [0:1];
  initial force ua = '{4'h1, 4'h2};
endmodule
