// A column keeps one element default and starts with the cells a member
// initializer gives other values, however large the array (SV 7.2.2).
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
