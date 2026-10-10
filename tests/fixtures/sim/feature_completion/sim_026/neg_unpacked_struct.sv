// SIM-026 A03 negative: a conversion into an unpacked structure is illegal (IEEE 1800-2009 21.3.4.3).
module tb;
  typedef struct { int a; int b; } s_t;
  integer c; s_t s;
  initial begin
    c = $sscanf("1", "%d", s);
    $finish;
  end
endmodule
