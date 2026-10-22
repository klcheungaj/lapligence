// SIM-043 A03: vpi_control from cbStartOfSimulation (decision S43-D10).
`timescale 1ns / 1ns
module tb;
  initial begin
    $display("[%0t] hdl: initial", $time);
    #2 $display("[%0t] hdl: later", $time);
  end
  final $display("[%0t] final", $time);
endmodule
