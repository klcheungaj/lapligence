// SIM-017: a message is a singular expression (SV 15.4.5 "task get( ref
// singular message )"; 6.4: an unpacked array is not singular), and
// industry-standard behaviour rejects an unpacked-array destination. llg
// receives unpacked-array messages only into record members, so a whole
// array variable destination is rejected explicitly.
module tb;
  typedef int quad_t[4];
  mailbox #(quad_t) m = new();
  quad_t b;
  initial begin
    m.put('{1, 2, 3, 4});
    m.get(b);
    $finish;
  end
endmodule
