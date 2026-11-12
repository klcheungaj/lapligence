// SIM-017: a message is a singular expression (SV 15.4.3 L20519 "The
// message is any singular expression"; 6.4: an unpacked array is not
// singular), and industry-standard behaviour rejects an associative-array
// message. The runtime message value has no nested associative form, so
// llg rejects it explicitly.
module tb;
  typedef int table_t[string];
  mailbox #(table_t) m = new();
  table_t a;
  initial begin
    a["k"] = 1;
    m.put(a);
    $finish;
  end
endmodule
