// SV2009 6.24.3: fixed-size bit-stream casts with an unpacked type require
// equal sizes.
module tb;
  typedef logic [2:0] l3_t [0:1];
  logic [6:0] v;
  l3_t l;
  initial begin
    l = l3_t'(v);
    $finish(0);
  end
endmodule
