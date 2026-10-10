// SIM-020 A03: a struct with a real member is not a bit-stream type.
module tb;
  typedef struct { byte a; real r; } s_t;
  s_t s;
  logic [71:0] v;
  initial begin
    v = {>>{s}};
    $finish;
  end
endmodule
