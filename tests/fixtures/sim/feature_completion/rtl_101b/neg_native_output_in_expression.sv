// RTL-101b limit: an output record argument with a string member needs a
// copy back after the call, which only a statement call provides.
module tb;
  typedef struct { bit [7:0] a [0:65536]; string s; } rec_t;
  rec_t r, q;
  function automatic int h(input rec_t i, output rec_t o);
    o = i;
    return 1;
  endfunction
  int n;
  initial begin
    r.s = "x";
    n = h(r, q);
    $display("%0d %s", n, q.s);
    $finish(0);
  end
endmodule
