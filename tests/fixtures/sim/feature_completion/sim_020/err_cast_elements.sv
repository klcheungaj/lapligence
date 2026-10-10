// SIM-020 A02: a bit-stream cast into a dynamic array needs whole elements
// (SV 6.24.3); 24 source bits do not make whole shortint elements.
module tb;
  typedef shortint sd_t[];
  byte q[$];
  sd_t d;
  initial begin
    q = {8'h01, 8'h02, 8'h03};
    $display("before %0d", q.size());
    d = sd_t'(q);
    $display("after %0d", d.size());
    $finish;
  end
endmodule
