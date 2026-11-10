`timescale 1ns/1ns
// Unpacked clockvars read through a virtual interface follow the bound
// instance (SV 14.13, 25.9): elements, a multidimensional element, an
// expression-bound #0 input and a whole-array copy.
interface bus(input bit clk);
  int arr [2];
  logic [3:0] g [0:1][0:1];
  clocking cb @(posedge clk);
    input arr, g;
    input #0 z = arr;
  endclocking
endinterface

module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  bus b1(clk);
  bus b2(clk);
  virtual bus v;
  int t [2];
  initial begin
    v = b1;
    b1.arr = '{1, 2};
    b1.g = '{'{4'h1, 4'h2}, '{4'h3, 4'h4}};
    b2.arr = '{5, 6};
    b2.g = '{'{4'h5, 4'h6}, '{4'h7, 4'h8}};
    #7 b1.arr[1] = 20; b2.g[1][0] = 4'hc;
    #10 v = b2;
  end
  always @(b1.cb) begin
    t = v.cb.arr;
    $display("%0t v %0d %0d %h %h z %0d t %0d %0d concrete %h", $time, v.cb.arr[0],
             v.cb.arr[1], v.cb.g[1][0], v.cb.g[0][1], v.cb.z[1], t[0], t[1], b2.cb.g[1][0]);
  end
  initial #36 $finish;
endmodule
