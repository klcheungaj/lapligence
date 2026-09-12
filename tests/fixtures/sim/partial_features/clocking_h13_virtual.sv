interface vif_if(input logic clk);
  logic data = 0;
  clocking cb @(posedge clk);
    input #1step data;
  endclocking
endinterface

module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  vif_if bus(clk);
  virtual vif_if vif = bus;

  initial begin
    #5 bus.data = 1;
    #10 $finish;
  end

  always #5 clk = ~clk;

  always @(vif.cb) begin
    #1 $display("t=%0t data=%0d sampled=%0d", $time, bus.data, vif.cb.data);
  end
endmodule
