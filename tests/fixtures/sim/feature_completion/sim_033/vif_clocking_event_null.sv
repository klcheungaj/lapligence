`timescale 1ns/1ns
// Waiting on a clocking block event through a null virtual interface handle
// stops the run (decision S99-D8), as any access through it does.
interface bus(input bit clk);
  logic [7:0] data;
  clocking cb @(posedge clk);
    input data;
  endclocking
endinterface

module tb;
  bit clk = 1'b0;
  always #5 clk = ~clk;
  bus b(clk);
  virtual bus v;
  initial begin
    $display("%0t before", $time);
    @(v.cb);
    $display("%0t after", $time);
  end
  initial #20 $finish;
endmodule
