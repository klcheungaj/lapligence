// RTL-101b limit: a column keeps one element default, so a member
// initializer that gives its cells different values is rejected rather
// than expanded per cell.
module tb;
  typedef struct {
    logic [7:0] a [0:65536] = '{0: 8'h1, default: 8'h5};
    logic [3:0] tag;
  } rec_t;
  rec_t r;
  initial begin
    $display("%h", r.a[0]);
    $finish(0);
  end
endmodule
