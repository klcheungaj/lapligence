// SV2009 6.24.2: $cast destination and source must be singular.
module tb;
  typedef struct { logic [3:0] a; bit [1:0] b; } rec_t;
  rec_t r, s;
  int ok;
  initial begin
    s = '{4'h1, 2'd1};
    ok = $cast(r, s);
    $finish(0);
  end
endmodule
