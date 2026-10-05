// RTL-101b limit: a ref formal aliases its actual, but the string member
// of a column-layout record travels in a native value copy.
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
