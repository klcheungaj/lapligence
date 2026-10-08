// A legal unpacked structure argument whose sampled history is not
// implemented; it is rejected explicitly.
module tb;
  typedef struct {
    logic [3:0] a;
    int b;
  } rec_t;
  logic clk = 1'b0;
  rec_t rec = '{4'h1, 7};
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $changed(rec));
  initial #10 $finish;
endmodule
