// SIM-017: a message is a singular expression (SV 15.4.3 L20519 "The
// message is any singular expression"; 6.4: an unpacked array is not
// singular), and industry-standard behaviour rejects an unpacked-array
// message. llg sends unpacked-array messages only from assignment patterns
// and record members, so a whole array variable source is rejected
// explicitly.
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
