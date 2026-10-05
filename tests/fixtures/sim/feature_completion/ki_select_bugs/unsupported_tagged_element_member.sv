// A tagged-union member of a packed-array element needs the element's tag
// check (IEEE 1800-2009 7.3.2); llg rejects it explicitly instead of
// reading the member without that check.
module tb;
  typedef union tagged packed { logic [3:0] a; logic [3:0] b; } t_t;
  t_t [1:0] tp;
  integer i;
  initial begin
    i = 1;
    tp[1] = tagged a 4'h5;
    $display("%h", tp[i].a);
  end
endmodule
