`timescale 1ns/1ns
// An unpacked union clockvar rejects (decision K1-D1).
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  typedef union { int i; bit [31:0] u; } un_t;
  un_t un;
  clocking cb @(posedge clk);
    input un;
  endclocking
  initial begin
    un.i = 5;
    @(cb) $display("%0d", cb.un.i);
    $finish;
  end
endmodule
