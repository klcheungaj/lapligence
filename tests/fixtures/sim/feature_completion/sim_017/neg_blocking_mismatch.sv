// SIM-017: a blocking get whose destination does not match the message type
// is a run-time error (SV 15.4.5); the simulation ends with exit 1 before the
// next statement.
module tb;
  typedef struct {
    int a;
    string s;
  } r_t;
  mailbox m = new();
  r_t r;
  initial begin
    m.put(5);
    $display("before");
    m.get(r);
    $display("after");
    $finish;
  end
endmodule
