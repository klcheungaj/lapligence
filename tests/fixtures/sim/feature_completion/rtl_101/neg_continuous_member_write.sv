// RTL-101 nearest illegal: a continuously assigned record variable has one
// writer (SV 6.5), so a procedural write to one of its columns is illegal.
module tb;
  typedef struct { logic [7:0] a [0:65536]; logic [3:0] tag; } rec_t;
  rec_t r, q;
  assign q = r;
  initial begin
    q.a[3] = 8'h1;
    $finish(0);
  end
endmodule
