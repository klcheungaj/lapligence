// SIM-017: an associative array message is legal (SV 15.4) but the runtime
// value has no nested associative form; it is rejected explicitly.
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
