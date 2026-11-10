`timescale 1ns/1ns
// An unpacked union clockvar samples its whole value (SV 14.13). Each read
// names the member last written (SV 7.3: "Only one of the data types in the
// union can be used at a time").
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
    #7 un.u = 32'hff;
    #10 un.i = -1;
  end
  always @(cb)
    if ($time == 15) $display("%0t u %h", $time, cb.un.u);
    else $display("%0t i %0d", $time, cb.un.i);
  initial #26 $finish;
endmodule
