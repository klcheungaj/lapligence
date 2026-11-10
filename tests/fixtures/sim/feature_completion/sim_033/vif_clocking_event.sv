`timescale 1ns/1ns
// Waiting on a clocking block through a virtual interface view (SV 14.13,
// 25.9) resumes at the block event of the bound instance.
interface bus(input bit clk);
  logic [7:0] data;
  clocking cb @(posedge clk);
    input data;
  endclocking
  modport tb_mp(clocking cb);
endinterface

module tb;
  bit clk = 1'b0;
  always #5 clk = ~clk;
  bus b(clk);
  virtual bus.tb_mp vp;
  initial begin
    b.data = 8'h11;
    vp = b.tb_mp;
    @(vp.cb);
    $display("%0t %h", $time, vp.cb.data);
    $finish;
  end
endmodule
