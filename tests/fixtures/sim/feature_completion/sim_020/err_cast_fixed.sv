// SIM-020 A02: a dynamically sized source cast to a fixed-size type must
// match its size (SV 6.24.3); the mismatch is found at run time.
module tb;
  typedef struct { byte a; byte q[$]; } d_t;
  d_t v;
  int i;
  initial begin
    i = 7;
    v.q.push_back(8'h01);
    $display("before %0d", i);
    i = int'(v);
    $display("after %h", i);
    $finish;
  end
endmodule
