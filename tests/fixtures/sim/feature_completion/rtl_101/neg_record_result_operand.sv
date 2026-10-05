// RTL-101 limit: a function result beyond packed capacity has no packed
// value, so equality takes record storage operands only.
module tb;
  typedef struct { logic [1023:0] w [0:2047]; bit [7:0] t; } big_t;
  big_t r;
  function automatic big_t f(input big_t x);
    return x;
  endfunction
  initial begin
    r.t = 8'd1;
    $display("%0d", f(r) == r);
    $finish(0);
  end
endmodule
