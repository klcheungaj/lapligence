// RTL-101b: a ref formal aliases its actual, so the subroutine is specialized
// for the record it binds and writes the string member in place.
module tb;
  typedef struct { bit [7:0] a [0:65536]; string s; } rec_t;
  rec_t r;
  task automatic t(ref rec_t v);
    v.s = "y";
  endtask
  initial begin
    t(r);
    $display("%s", r.s);
    $finish(0);
  end
endmodule
