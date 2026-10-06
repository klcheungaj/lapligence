// RTL-101b limit: a whole member array of a record call result has no
// lexical owner outside its expression; select one element or assign the
// result to a record variable first.
module tb;
  typedef struct { logic [1023:0] w [0:2047]; bit [7:0] t; } big_t;
  big_t r;
  logic [1023:0] arr [0:2047];
  function automatic big_t f(input big_t x);
    return x;
  endfunction
  initial begin
    arr = f(r).w;
    $display("%0d", arr[0][7:0]);
    $finish(0);
  end
endmodule
