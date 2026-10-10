// SIM-020 llg limit: a bit-stream cast into a struct with resizable members
// is rejected.
module tb;
  typedef struct { byte a; byte q[$]; byte b; } d_t;
  d_t v;
  byte q[$];
  initial begin
    q = {8'h01, 8'h02, 8'h05};
    v = d_t'(q);
    $finish;
  end
endmodule
