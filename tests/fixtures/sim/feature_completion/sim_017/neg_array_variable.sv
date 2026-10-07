// SIM-017: a whole unpacked-array variable as a message is legal (SV 15.4);
// only assignment patterns and record members transfer, so this is
// rejected explicitly.
module tb;
  typedef int quad_t[4];
  mailbox #(quad_t) m = new();
  quad_t a;
  initial begin
    a = '{1, 2, 3, 4};
    m.put(a);
    $finish;
  end
endmodule
