// RTL-101: generated code scales with the declaration, not its extent.
module tb;
  typedef struct { logic [7:0] a [0:1048575]; logic [3:0] tag; } rec_t;
  rec_t r, s;
  integer i;
  initial begin
    i = 1048575;
    r.a[i] = 8'h5a;
    r.tag = 4'h3;
    s = r;
    $display("%h %h %0d", s.a[i], s.tag, s == r);
    $finish(0);
  end
endmodule
