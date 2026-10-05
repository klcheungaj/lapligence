// RTL-101b limit: a string member of a record call result has no string
// expression form; read it from a record variable holding the result.
module tb;
  typedef struct { logic [1023:0] w [0:2047]; string n; } big_t;
  big_t r;
  function automatic big_t f(input big_t x);
    return x;
  endfunction
  initial begin
    r.n = "z";
    $display("%s", f(r).n);
    $finish(0);
  end
endmodule
