// RTL-101 limit: a whole value beyond packed capacity cannot be bound to a
// pattern variable.
module tb;
  typedef struct { logic [7:0] a [0:65536]; logic [3:0] tag; } rec_t;
  rec_t r;
  initial begin
    r.tag = 4'h3;
    if (r matches .x) $display("%h", x.tag);
    $finish(0);
  end
endmodule
