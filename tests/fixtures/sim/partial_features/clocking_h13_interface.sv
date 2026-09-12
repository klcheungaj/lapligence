interface bus_if(input logic clk);
  logic data = 0;
  logic alias_data = 0;

  default clocking cb_default @(posedge clk);
    default input #1step;
    input sample = alias_data;
  endclocking

  clocking cb_zero @(posedge clk);
    input #0 zero_sample = data;
  endclocking
endinterface

module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  bus_if bus(clk);

  global clocking cb_global @(posedge clk);
  endclocking

  initial begin
    #5 bus.alias_data = 1;
    #10 bus.data = 1;
    #3 $finish;
  end

  always #5 clk = ~clk;

  always @(bus.cb_default) begin
    #1 $display("default t=%0t raw=%0d sample=%0d", $time, bus.alias_data,
                bus.cb_default.sample);
  end

  always @(bus.cb_zero) begin
    #1 $display("zero t=%0t raw=%0d sample=%0d", $time, bus.data,
                bus.cb_zero.zero_sample);
  end
endmodule
