// RTL-101 (lifted by RTL-101b): a function result beyond packed capacity
// compares column by column from a lexical temporary.
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
