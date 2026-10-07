// SIM-017: try_get of a record in a loop condition is legal (SV 15.4.6); the
// copy-out to the record needs statements around each evaluation of the
// condition, which are not represented, so it is rejected explicitly.
module tb;
  typedef struct {
    int a;
    string s;
  } r_t;
  mailbox #(r_t) m = new();
  r_t r;
  initial begin
    while (m.try_get(r) > 0) $display("%0d", r.a);
    $finish;
  end
endmodule
